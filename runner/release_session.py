#!/usr/bin/env python3
"""Temporary loopback-only native validation for one explicitly approved release archive."""
import argparse
import base64
import datetime
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import stat
import tempfile
import time

import server as runner

EXECUTION_RESERVE = 90
MAX_LEASE = 7200


class NotApproved(ValueError):
    pass


def read_token(path, environment):
    if path is not None:
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(descriptor, "r") as file:
            info = os.fstat(file.fileno())
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o600:
                raise ValueError("The token file must be a regular file owned by this user with mode 0600.")
            token = file.read(1025).strip()
    else:
        token = environment.get("APPS_RELEASE_SESSION_TOKEN", "")
    if not 32 <= len(token) <= 1024 or not token.isascii() or any(char.isspace() for char in token):
        raise ValueError("Provide a token of 32–1024 ASCII non-space characters using a 0600 file or APPS_RELEASE_SESSION_TOKEN.")
    return token


class ReleaseSession:
    def __init__(self, app_id, target, package_sha256, package_size, lease_seconds, executor=None, clock=time.monotonic):
        if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", app_id):
            raise ValueError("The allowed app ID must be explicit and valid.")
        if target not in ("macos-aarch64", "macos-x86_64") or not re.fullmatch(r"[a-f0-9]{64}", package_sha256):
            raise ValueError("Provide an exact native Mac target and lowercase archive SHA-256.")
        if not 1 <= package_size <= runner.MAX_BYTES or not EXECUTION_RESERVE + 1 <= lease_seconds <= MAX_LEASE:
            raise ValueError("Invalid package size or lease; the lease must be 91–7200 seconds.")
        self.app_id, self.target, self.package_sha256 = app_id, target, package_sha256
        self.package_size, self.clock = package_size, clock
        self.expires = clock() + lease_seconds
        self.executor = executor or runner.validate
        self.encoded_size = ((package_size + 2) // 3) * 4
        self.max_request_size = self.encoded_size + 65536

    def remaining(self):
        return self.expires - self.clock()

    def validate(self, request):
        if self.remaining() < EXECUTION_RESERVE:
            raise runner.WorkerUnavailable("The release session has expired or has insufficient time for another validation.")
        required = {"app_id", "target", "package_sha256", "package_base64", "manifest"}
        if not isinstance(request, dict) or set(request) != required:
            raise NotApproved("Only the explicitly approved release archive may be validated.")
        if (request["app_id"] != self.app_id or request["target"] != self.target or
                request["package_sha256"] != self.package_sha256 or not isinstance(request["manifest"], dict)):
            raise NotApproved("This app, target or archive SHA-256 is not approved for the release session.")
        encoded = request["package_base64"]
        if not isinstance(encoded, str) or len(encoded) != self.encoded_size:
            raise NotApproved("The release archive does not match the approved size.")
        try:
            data = base64.b64decode(encoded, validate=True)
        except (ValueError, TypeError):
            raise NotApproved("The release archive encoding is invalid.") from None
        if len(data) != self.package_size or not hmac.compare_digest(hashlib.sha256(data).hexdigest(), self.package_sha256):
            raise NotApproved("The release archive bytes are not approved.")
        # This boundary precedes all extraction and execution. The existing runner
        # rechecks the archive/manifest and supplies a clean sandbox environment.
        return self.executor(request, {}, {})


class ReleaseHandler(runner.Handler):
    def do_POST(self):
        if self.path != "/validate":
            return self.reply(404, {"error": "Unknown runner endpoint."})
        if not hmac.compare_digest(self.headers.get("Authorization", "").encode(), ("Bearer " + self.server.token).encode()):
            return self.reply(401, {"error": "Invalid release-session credential."})
        session = self.server.session
        if session.remaining() < EXECUTION_RESERVE:
            return self.reply(503, {"error": "The release session has expired."})
        try:
            size_text = self.headers.get("Content-Length", "")
            if self.headers.get("Transfer-Encoding") or not size_text.isdecimal():
                return self.reply(400, {"error": "A fixed Content-Length is required."})
            size = int(size_text)
            if not 0 < size <= session.max_request_size:
                return self.reply(413, {"error": "Request exceeds the approved release envelope."})
            deadline, received, chunks = min(session.clock() + 30, session.expires), 0, []
            while received < size:
                remaining = deadline - session.clock()
                if remaining <= 0:
                    raise ValueError("The release request exceeded its upload deadline.")
                self.connection.settimeout(remaining)
                chunk = self.rfile.read1(min(1024 * 1024, size - received))
                if not chunk:
                    raise ValueError("The release request ended before Content-Length.")
                chunks.append(chunk)
                received += len(chunk)
            self.reply(200, session.validate(json.loads(b"".join(chunks))))
        except NotApproved as error:
            self.reply(403, {"error": str(error)})
        except runner.WorkerUnavailable as error:
            self.reply(503, {"error": str(error)})
        except (ValueError, KeyError, TypeError, OSError, runner.tarfile.TarError, runner.subprocess.SubprocessError) as error:
            self.reply(422, {"error": str(error)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app-id", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--package-sha256", required=True)
    parser.add_argument("--token-file", type=Path)
    parser.add_argument("--port", type=int, default=4314)
    parser.add_argument("--lease-seconds", type=int, default=3600)
    args = parser.parse_args()
    if os.geteuid() == 0 or runner.native_target() != args.target or not Path("/usr/bin/sandbox-exec").is_file():
        parser.error("Run as your normal user on the matching native Mac with sandbox-exec; root is not supported.")
    if not 1024 <= args.port <= 65535:
        parser.error("Use an unprivileged loopback port between 1024 and 65535.")
    try:
        token = read_token(args.token_file, os.environ)
        if not 1 <= args.package.stat().st_size <= runner.MAX_BYTES:
            raise ValueError("The approved archive must be between 1 byte and 128 MiB.")
        data = args.package.read_bytes()
        if not hmac.compare_digest(hashlib.sha256(data).hexdigest(), args.package_sha256):
            raise ValueError("The local release archive does not match the explicitly approved SHA-256.")
        session = ReleaseSession(args.app_id, args.target, args.package_sha256, len(data), args.lease_seconds)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    del data
    os.umask(0o077)
    with tempfile.TemporaryDirectory(prefix="silicon-apps-release-session-") as jobs:
        # The service itself does not inherit cloud, Accounts, proxy or personal-home
        # settings. Each package command receives the runner's stricter clean env.
        os.environ.clear()
        os.environ.update({"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": jobs, "TMPDIR": jobs, "SILICON_HOME": jobs})
        tempfile.tempdir = jobs
        with runner.RunnerHTTPServer(("127.0.0.1", args.port), ReleaseHandler) as service:
            service.token, service.session, service.timeout = token, session, 0.5
            expires = datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(seconds=args.lease_seconds)
            print(json.dumps({"status": "ready", "url": f"http://127.0.0.1:{args.port}/validate",
                              "app_id": args.app_id, "target": args.target, "package_sha256": args.package_sha256,
                              "expires_at": expires.isoformat(), "pid": os.getpid(), "loopback_only": True}), flush=True)
            try:
                while session.remaining() > 0:
                    service.handle_request()
            except KeyboardInterrupt:
                pass
    print(json.dumps({"status": "closed", "package_sha256": args.package_sha256}), flush=True)


if __name__ == "__main__":
    main()
