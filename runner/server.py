#!/usr/bin/env python3
"""Authenticated isolated package validator. Runs no package code outside a sandbox."""
import base64
import hashlib
import hmac
import http.client
import ipaddress
import socket
import ssl
import io
import json
import os
import platform
import re
try:
    import resource
except ImportError:  # Windows workers isolate execution in Hyper-V containers.
    resource = None
import signal
import subprocess
import tarfile
import tempfile
import time
import uuid
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit

import yaml

MAX_BYTES = 128 * 1024 * 1024
MAX_UNPACKED = 512 * 1024 * 1024
OUTPUT_BYTES = 64 * 1024
MAX_WORKER_RESPONSE = 1024 * 1024
MAX_WORKER_SECONDS = 100
TARGETS = {"linux-x86_64", "linux-i686", "linux-aarch64", "linux-armv7hf", "windows-x86_64", "windows-i686", "windows-aarch64", "macos-x86_64", "macos-aarch64"}
WINDOWS_PLATFORMS = {"windows-x86_64": "windows/amd64", "windows-i686": "windows/amd64", "windows-aarch64": "windows/arm64"}
WINDOWS_RESERVED = {"CON", "PRN", "AUX", "NUL", *(f"COM{i}" for i in range(1, 10)), *(f"LPT{i}" for i in range(1, 10))}
COMMANDS = [("--help", ["--help"]), ("accounts --json", ["accounts", "--json"]),
            ("login status --json", ["login", "status", "--json"])]
PLATFORMS = {"linux-x86_64": "linux/amd64", "linux-i686": "linux/386",
             "linux-aarch64": "linux/arm64", "linux-armv7hf": "linux/arm/v7"}


class WorkerValidationError(ValueError):
    def __init__(self, result):
        super().__init__("The target worker rejected the package.")
        self.result = result


class WorkerUnavailable(ValueError):
    """A configured execution environment is temporarily unavailable; retry is safe."""


def worker_config(value):
    if not isinstance(value, dict) or len(value) > len(TARGETS):
        raise ValueError("APPS_RUNNER_WORKERS must be an object keyed by supported target.")
    result = {}
    for target, spec in value.items():
        if target not in TARGETS or not isinstance(spec, dict) or set(spec) != {"url", "token"}:
            raise ValueError("Each worker needs a supported target and exactly url/token fields.")
        token, raw_url = spec["token"], spec["url"]
        if not isinstance(token, str) or len(token) < 32 or not token.isascii() or any(c.isspace() for c in token):
            raise ValueError("Worker credentials must contain at least 32 ASCII non-space characters.")
        if not isinstance(raw_url, str):
            raise ValueError("Worker URL must be a string.")
        url = urlsplit(raw_url)
        local = url.hostname == "localhost"
        try:
            local = local or ipaddress.ip_address(url.hostname).is_loopback
        except ValueError:
            pass
        if (url.scheme not in ("http", "https") or not url.hostname or
                url.username or url.password or url.query or url.fragment or
                url.path not in ("", "/", "/validate") or
                url.scheme == "http" and not local):
            raise ValueError("Workers require HTTPS, or literal loopback HTTP, without URL credentials, queries or fragments; only /validate is allowed.")
        try:
            port = url.port
        except ValueError as error:
            raise ValueError("Worker URL has an invalid port.") from error
        result[target] = {"url": raw_url.rstrip("/"), "token": token}
    return result


def image_config(value):
    if not isinstance(value, dict):
        raise ValueError("APPS_RUNNER_IMAGES must be an object keyed by supported target.")
    for target, image in value.items():
        if target not in {*PLATFORMS, *WINDOWS_PLATFORMS} or not isinstance(image, str) or not re.fullmatch(r"[A-Za-z0-9./:_-]+@sha256:[a-fA-F0-9]{64}", image):
            raise ValueError("Container images require a Linux/Windows target and an immutable image@sha256:<64 hex> reference.")
    return value


def forward_to_worker(request, spec):
    """Forward to a configured authenticated worker; no redirects, proxy, or fallback."""
    url = urlsplit(spec["url"])
    connection_type = http.client.HTTPSConnection if url.scheme == "https" else http.client.HTTPConnection
    options = {"timeout": MAX_WORKER_SECONDS}
    if url.scheme == "https":
        options["context"] = ssl.create_default_context()
    connection = connection_type(url.hostname, url.port, **options)
    deadline = time.monotonic() + MAX_WORKER_SECONDS
    try:
        payload = json.dumps(request).encode()
        connection.request("POST", "/validate", body=payload,
                           headers={"Authorization": "Bearer " + spec["token"], "Content-Type": "application/json", "Content-Length": str(len(payload))})
        transport = connection.sock
        transport.settimeout(max(0.1, deadline - time.monotonic()))
        response = connection.getresponse()
        if response.status not in (200, 422):
            raise WorkerUnavailable(f"The configured {request['target']} worker returned HTTP {response.status}; no local execution was attempted.")
        length = response.getheader("Content-Length")
        if length and (not length.isdecimal() or int(length) > MAX_WORKER_RESPONSE):
            raise WorkerUnavailable("Worker response exceeds the 1 MiB result limit.")
        chunks, size = [], 0
        while True:
            if response.isclosed():
                break
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise WorkerUnavailable("Worker exceeded the 100 second execution deadline.")
            transport.settimeout(remaining)
            chunk = response.read1(min(65536, MAX_WORKER_RESPONSE + 1 - size))
            if not chunk:
                break
            chunks.append(chunk)
            size += len(chunk)
            if size > MAX_WORKER_RESPONSE:
                raise WorkerUnavailable("Worker response exceeds the 1 MiB result limit.")
        result = json.loads(b"".join(chunks))
        if response.status == 422:
            raise WorkerValidationError(result)
        if not isinstance(result, dict) or result.get("isolated") is not True or result.get("target") != request["target"]:
            raise WorkerUnavailable("Worker did not attest isolation on the requested target.")
        checks = result.get("validation")
        if not isinstance(checks, list) or len(checks) != 3 or {c.get("command") for c in checks if isinstance(c, dict)} != {c for c, _ in COMMANDS}:
            raise WorkerUnavailable("Worker response must contain exactly the three required command results.")
        for check in checks:
            if any(not isinstance(check.get(key), str) or len(check[key].encode()) > OUTPUT_BYTES * 3 for key in ("stdout", "stderr")) or type(check.get("exit_code")) is not int:
                raise WorkerUnavailable("Worker command output or exit code is invalid or exceeds limits.")
        return result
    except (OSError, http.client.HTTPException, ValueError) as error:
        if isinstance(error, (WorkerUnavailable, WorkerValidationError)):
            raise
        raise WorkerUnavailable(f"Configured {request['target']} worker failed ({type(error).__name__}); no local execution was attempted.") from error
    finally:
        connection.close()


def native_target():
    arch = {"arm64": "aarch64", "aarch64": "aarch64", "x86_64": "x86_64", "AMD64": "x86_64"}.get(platform.machine())
    return f"macos-{arch}" if platform.system() == "Darwin" and arch else None


def regular_path(value):
    return isinstance(value, str) and bool(value) and not any(c in value for c in "\\:\x00") and not value.startswith("/") and all(p not in ("", ".", "..") and not p.endswith((" ", ".")) and p.split(".")[0].upper() not in WINDOWS_RESERVED for p in value.split("/"))


def unpack(data, destination, expected_manifest):
    """Reject link, traversal, device, duplicate, count and decompression attacks."""
    if len(data) > MAX_BYTES:
        raise ValueError("Compressed package exceeds the runner's 128 MiB limit.")
    seen, total = set(), 0
    with tarfile.open(fileobj=io.BytesIO(data), mode="r|gz") as archive:
        for entry in archive:
            if not regular_path(entry.name) or entry.name in seen:
                raise ValueError(f"Unsafe or duplicate archive path: {entry.name!r}")
            if not (entry.isfile() or entry.isdir()):
                raise ValueError(f"Links and special files are forbidden: {entry.name!r}")
            seen.add(entry.name)
            if entry.size < 0:
                raise ValueError("Negative archive entry sizes are forbidden.")
            total += entry.size
            if len(seen) > 20000 or total > MAX_UNPACKED:
                raise ValueError("The archive exceeds runner extraction limits.")
            path = destination.joinpath(*PurePosixPath(entry.name).parts)
            if entry.isdir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                source = archive.extractfile(entry)
                with path.open("xb") as target:
                    while chunk := source.read(1024 * 1024):
                        target.write(chunk)
                path.chmod(0o755 if entry.mode & 0o111 else 0o644)
    # The service runs with UMask=0077, while Linux guests run as uid 65534.
    # Normalize explicit and implicit directories so that the read-only package
    # mount remains traversable by that separate, unprivileged container user.
    for path in destination.rglob("*"):
        if path.is_dir():
            path.chmod(0o755)
    manifest_path = destination / "apps.yaml"
    if not manifest_path.is_file() or manifest_path.stat().st_size > 1024 * 1024:
        raise ValueError("apps.yaml is missing or larger than 1 MiB.")
    manifest = yaml.safe_load(manifest_path.read_text())
    if not isinstance(manifest, dict) or not isinstance(manifest.get("targets"), dict) or any(not isinstance(t, dict) for t in manifest["targets"].values()):
        raise ValueError("apps.yaml must contain an object with a targets object.")
    # Default version is interpreted consistently with the Rust package.
    if isinstance(manifest, dict):
        manifest.setdefault("schema_version", 1)
        for target in manifest.get("targets", {}).values():
            if isinstance(target, dict) and target.get("install_script") is None:
                target.pop("install_script", None)
    expected_manifest = json.loads(json.dumps(expected_manifest))
    for target in expected_manifest.get("targets", {}).values():
        if target.get("install_script") is None:
            target.pop("install_script", None)
    if manifest != expected_manifest:
        raise ValueError("The runner manifest differs from the uploaded apps.yaml.")
    return manifest


def limits():
    os.setsid()
    resource.setrlimit(resource.RLIMIT_CPU, (10, 10))
    resource.setrlimit(resource.RLIMIT_FSIZE, (OUTPUT_BYTES, OUTPUT_BYTES))
    resource.setrlimit(resource.RLIMIT_NOFILE, (64, 64))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_NPROC, (32, 32))


def bounded_run(argv, cwd, env, timeout=15, limit_process=False):
    """Bound wall-clock/output; native forks are denied and containers are removed by execute."""
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        proc = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                stdout=stdout, stderr=stderr,
                                **({"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == "nt" else {"preexec_fn": limits if limit_process else os.setsid}))
        timed_out = False
        deadline = time.monotonic() + timeout
        try:
            while proc.poll() is None:
                if time.monotonic() >= deadline or os.fstat(stdout.fileno()).st_size > OUTPUT_BYTES or os.fstat(stderr.fileno()).st_size > OUTPUT_BYTES:
                    timed_out = True
                    break
                time.sleep(0.03)
        finally:
            # Native processes cannot fork; Docker guests are removed separately by execute.
            try:
                if os.name == "nt":
                    if proc.poll() is None:
                        proc.kill()
                else:
                    os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            proc.wait(timeout=5)
        stdout.seek(0)
        stderr.seek(0)
        out, err = stdout.read(OUTPUT_BYTES).decode(errors="replace"), stderr.read(OUTPUT_BYTES).decode(errors="replace")
        if timed_out:
            err += "\nCommand exceeded the time or output limit; its process group was terminated."
        return (-1 if timed_out else proc.returncode), out, err


def mac_command(root, binary, args):
    # Deny network, home directories, service credentials and writes outside the job.
    # sandbox-exec's -D arguments avoid interpolating untrusted paths into policy syntax.
    policy = '''(version 1)
    (deny default)
    (allow process-exec)
    (allow sysctl-read)
    (allow file-read* (literal "/") (subpath "/System") (subpath "/usr/lib") (subpath "/bin") (subpath "/usr/bin")
      (literal "/dev/null") (literal "/dev/urandom") (literal "/dev/random") (subpath (param "PACKAGE")))
    (allow file-read-metadata)
    (allow file-write* (subpath (param "SCRATCH")) (literal "/dev/null"))'''
    return ["/usr/bin/sandbox-exec", "-D", f"PACKAGE={root}", "-D", f"SCRATCH={root / '.scratch'}", "-p", policy, str(root / binary), *args]


def execute(root, binary, args, target, images):
    scratch = root / ".scratch"
    scratch.mkdir(exist_ok=True)
    env = {"PATH": "/usr/bin:/bin", "HOME": str(scratch), "TMPDIR": str(scratch), "LANG": "C", "APPS_TELEMETRY": "0", "SILICON_HOME": str(scratch)}
    if target == native_target() and Path("/usr/bin/sandbox-exec").is_file():
        return bounded_run(mac_command(root, binary, args), root, env, limit_process=True)
    image = images.get(target)
    if not image or (target not in PLATFORMS and not target.startswith("windows-")):
        raise WorkerUnavailable(f"No isolated native or OCI runner is configured for {target}.")
    if target.startswith("windows-") and os.name != "nt":
        raise WorkerUnavailable("Windows package validation requires a Windows worker with Hyper-V container isolation.")
    name = "apps-validation-" + uuid.uuid4().hex
    command = ["docker", "run", "--rm", "--pull=never", "--name", name, "--platform", PLATFORMS.get(target, ""),
               "--network=none", "--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges", "--pids-limit=32",
               "--memory=256m", "--cpus=1", "--user=65534:65534", "--tmpfs=/tmp:rw,noexec,nosuid,size=16m",
               "--mount", f"type=bind,source={root},target=/package,readonly", "--workdir=/package",
               "--env=HOME=/tmp", "--env=SILICON_HOME=/tmp", "--env=APPS_TELEMETRY=0",
               "--entrypoint", "/package/" + binary, image, *args]
    if target.startswith("windows-"):
        # A fresh Hyper-V utility VM owns the container kernel. Package files are
        # mounted read-only; no host environment, network, sockets or credentials
        # enter the guest. ContainerUser is unprivileged inside that VM.
        command = ["docker", "run", "--rm", "--pull=never", "--name", name,
                   "--isolation=hyperv", "--platform", WINDOWS_PLATFORMS[target], "--network=none", "--memory=1g", "--cpu-count=1",
                   "--user=ContainerUser", "--mount", f"type=bind,source={root},target=C:\\package,readonly",
                   "--workdir=C:\\Users\\ContainerUser", "--env=HOME=C:\\Users\\ContainerUser",
                   "--env=SILICON_HOME=C:\\Users\\ContainerUser", "--env=APPS_TELEMETRY=0",
                   "--entrypoint", "C:\\package\\" + binary.replace("/", "\\"), image, *args]
    try:
        # Only the Docker client sees the host environment; it is not forwarded into containers.
        return bounded_run(command, root, os.environ.copy(), timeout=25)
    finally:
        subprocess.run(["docker", "rm", "-f", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)


def validate(request, images, workers=None):
    if not isinstance(request, dict) or any(not isinstance(request.get(key), str) for key in ("target", "app_id", "package_base64", "package_sha256")) or not isinstance(request.get("manifest"), dict):
        raise ValueError("Runner request requires target/app_id/package_base64/package_sha256 strings and a manifest object.")
    if len(request["package_base64"]) > ((MAX_BYTES + 2) // 3) * 4:
        raise ValueError("Compressed package exceeds the runner's 128 MiB limit.")
    target, app_id = request["target"], request["app_id"]
    data = base64.b64decode(request["package_base64"], validate=True)
    if not hmac.compare_digest(hashlib.sha256(data).hexdigest(), request["package_sha256"]):
        raise ValueError("Package SHA-256 does not match the submitted bytes.")
    if target not in TARGETS:
        raise ValueError("Unknown package target.")
    if workers and target in workers:
        return forward_to_worker(request, workers[target])
    if target != native_target() and target not in images:
        raise WorkerUnavailable(f"No isolated runner is configured for {target}.")
    with tempfile.TemporaryDirectory(prefix="silicon-apps-runner-") as folder:
        root = Path(folder).resolve()
        root.chmod(0o755)
        manifest = unpack(data, root, request["manifest"])
        if manifest.get("app_id") != app_id:
            raise ValueError("Manifest app_id does not match the upload.")
        binary = manifest.get("targets", {}).get(target, {}).get("binary")
        if not regular_path(binary) or not (root / binary).is_file():
            raise ValueError("The target binary must be a regular file in the package.")
        results = []
        for label, args in COMMANDS:
            code, stdout, stderr = execute(root, binary, args, target, images)
            passed = code == 0
            expected = "Exit 0 and nonempty help text."
            if label == "--help":
                passed = passed and bool((stdout + stderr).strip())
            else:
                expected = 'Exit 0 and JSON containing app_id matching the app.' if label == "accounts --json" else 'Exit 0 and JSON containing authenticated: false in a clean signed-out sandbox.'
                try:
                    body = json.loads(stdout)
                    if label == "accounts --json":
                        passed = passed and isinstance(body, dict) and body.get("app_id") == app_id
                    else:
                        passed = passed and isinstance(body, dict) and body.get("authenticated") is False
                except (ValueError, TypeError):
                    passed = False
            results.append({"command": label, "exit_code": code, "stdout": stdout, "stderr": stderr, "passed": bool(passed), "expected": expected})
    return {"isolated": True, "target": target, "validation": results}


class Handler(BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(10)

    def log_message(self, *_args):
        pass  # Never log credentials, uploads or program output.

    def reply(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        if self.path != "/validate":
            return self.reply(404, {"error": "Unknown runner endpoint."})
        if not hmac.compare_digest(self.headers.get("Authorization", "").encode(), ("Bearer " + self.server.token).encode()):
            return self.reply(401, {"error": "Invalid runner credential."})
        try:
            size = int(self.headers.get("Content-Length", "0"))
            if size <= 0 or size > MAX_BYTES * 2:
                return self.reply(413, {"error": "Runner request must be 1–256 MiB."})
            deadline, received, chunks = time.monotonic() + 30, 0, []
            while received < size:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise ValueError("Runner request exceeded the 30 second upload deadline.")
                self.connection.settimeout(remaining)
                chunk = self.rfile.read1(min(1024 * 1024, size - received))
                if not chunk:
                    raise ValueError("Runner request body ended before Content-Length.")
                chunks.append(chunk)
                received += len(chunk)
            result = validate(json.loads(b"".join(chunks)), self.server.images, self.server.workers)
            self.reply(200, result)
        except WorkerValidationError as error:
            self.reply(422, error.result)
        except WorkerUnavailable as error:
            self.reply(503, {"error": str(error)})
        except (ValueError, KeyError, TypeError, OSError, tarfile.TarError, subprocess.SubprocessError) as error:
            self.reply(422, {"error": str(error)})


def main():
    token = os.environ.get("APPS_RUNNER_TOKEN", "")
    if len(token) < 32 or not token.isascii() or any(c.isspace() for c in token):
        raise SystemExit("APPS_RUNNER_TOKEN must be at least 32 characters.")
    server = HTTPServer((os.environ.get("APPS_RUNNER_HOST", "127.0.0.1"), int(os.environ.get("APPS_RUNNER_PORT", "4312"))), Handler)
    server.token = token
    server.images = image_config(json.loads(os.environ.get("APPS_RUNNER_IMAGES", "{}")))
    server.workers = worker_config(json.loads(os.environ.get("APPS_RUNNER_WORKERS", "{}")))
    print(f"Silicon Apps isolated runner on {server.server_address}; native target={native_target()}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
