#!/usr/bin/env python3
"""Exercise the installed Mac worker over verified TLS; never execute uploaded code directly."""
import argparse
import base64
import datetime
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import pwd
import socket
import ssl
import tarfile
import time
import urllib.error
import urllib.request

TARGET = "macos-aarch64"
COMMANDS = ["--help", "accounts --json", "login status --json"]
INSTALL = Path("/opt/silicon-apps-native")
STATE = Path("/var/lib/silicon-apps-native")


def package_request(data, manifest):
    return {"app_id": manifest["app_id"], "target": TARGET, "manifest": manifest,
            "package_sha256": hashlib.sha256(data).hexdigest(),
            "package_base64": base64.b64encode(data).decode()}


def fixture(program):
    manifest = {"schema_version": 1, "app_id": "mac-validation-probe", "version": "0.0.0", "command": "probe",
                "targets": {TARGET: {"binary": "bin/probe"}}}
    script = ("#!/usr/bin/perl\nuse strict; use warnings;\n" + program + '''
if ($ARGV[0] eq 'accounts') { print '{"app_id":"mac-validation-probe"}'; }
elsif ($ARGV[0] eq 'login') { print '{"authenticated":false}'; }
''').encode()
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        for name, data in (("apps.yaml", json.dumps(manifest).encode()), ("bin/probe", script)):
            entry = tarfile.TarInfo(name)
            entry.size, entry.mode = len(data), 0o755
            archive.addfile(entry, io.BytesIO(data))
    return package_request(buffer.getvalue(), manifest)


def verify_result(result, expected_app, passing=True):
    if result.get("isolated") is not True or result.get("target") != TARGET:
        raise AssertionError("The worker did not confirm native Mac isolation.")
    rows = result.get("validation", [])
    if [row.get("command") for row in rows] != COMMANDS:
        raise AssertionError("The worker did not return all exact required commands.")
    if any(len(row.get(stream, "").encode()) > 65536 for row in rows for stream in ("stdout", "stderr")):
        raise AssertionError("Worker output exceeded its byte bound.")
    if passing and not all(row.get("passed") is True and row.get("exit_code") == 0 for row in rows):
        raise AssertionError("Required command failed: " + json.dumps(rows))
    if json.loads(rows[1]["stdout"]).get("app_id") != expected_app:
        raise AssertionError("Accounts output did not match the app.")
    if json.loads(rows[2]["stdout"]).get("authenticated") is not False:
        raise AssertionError("Validation did not start signed out.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ca-file", required=True, type=Path)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--package-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64" or os.geteuid() != 0:
        parser.error("Run as root on the dedicated Apple Silicon worker.")
    import yaml
    config = json.loads((INSTALL / "worker.json").read_text())
    host, port = config["APPS_RUNNER_HOST"], int(config["APPS_RUNNER_PORT"])
    context = ssl.create_default_context(cafile=str(args.ca_file))
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), urllib.request.HTTPSHandler(context=context))
    url = f"https://{host}:{port}/validate"
    checks = []

    def request(body, token=None):
        encoded = json.dumps(body).encode()
        req = urllib.request.Request(url, data=encoded, headers={"Content-Type": "application/json",
            "Authorization": "Bearer " + (token if token is not None else config["APPS_RUNNER_TOKEN"])})
        before = time.monotonic()
        with opener.open(req, timeout=75) as response:
            result = json.loads(response.read(1024 * 1024))
        return result, round(time.monotonic() - before, 3)

    try:
        request({}, "invalid-worker-credential")
        raise AssertionError("Wrong token was accepted.")
    except urllib.error.HTTPError as error:
        if error.code != 401:
            raise
    checks.append({"name": "wrong-token", "status": 401, "passed": True})
    for label, tls, hostname in (("untrusted-ca", ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT), host),
                                 ("wrong-hostname", context, "invalid-worker.example")):
        try:
            with socket.create_connection((host, port), timeout=10) as connection:
                with tls.wrap_socket(connection, server_hostname=hostname):
                    raise AssertionError(label + " was accepted.")
        except ssl.SSLCertVerificationError:
            checks.append({"name": label, "passed": True})

    owner = pwd.getpwnam("_siliconapps")
    sentinel = STATE / "acceptance-sentinel"
    sentinel_text = "Nonsecret host-only sandbox acceptance sentinel.\n"
    descriptor = os.open(sentinel, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w") as file:
        file.write(sentinel_text)
    os.chown(sentinel, owner.pw_uid, owner.pw_gid)
    jobs_before = {path.name for path in (STATE / "jobs").iterdir()}
    program = r'''
use Errno qw(EACCES EPERM EAGAIN);
use Fcntl qw(O_WRONLY O_APPEND);
use Socket qw(AF_INET SOCK_STREAM sockaddr_in inet_aton);
sub denied { my ($name) = @_; die "$name had an unexpected errno: $!\n" unless $! == EACCES || $! == EPERM; print "$name denied\n"; }
if ($ARGV[0] eq '--help') {
  if (open(my $read, '<', 'SENTINEL')) { close($read); die "Host read was allowed\n"; } denied('host read');
  if (sysopen(my $write, 'SENTINEL', O_WRONLY | O_APPEND)) { close($write); die "Host write was allowed\n"; } denied('host write');
  for my $address ('HOST', '169.254.169.254') {
    if (!socket(my $network, AF_INET, SOCK_STREAM, 0)) { denied('socket'); }
    else { if (connect($network, sockaddr_in(PORT, inet_aton($address)))) { close($network); die "Network was allowed\n"; } denied('network'); close($network); }
  }
  my $child = fork();
  if (defined($child)) { if ($child == 0) { exit 2; } kill 9, $child; waitpid($child, 0); die "Fork was allowed\n"; }
  if ($! == EAGAIN) { print "fork denied by process control\n"; } else { denied('fork'); }
  open(my $scratch, '>', "$ENV{HOME}/allowed-scratch") or die "Scratch was not writable: $!\n";
  print $scratch "scratch allowed\n"; close($scratch); print "scratch write allowed\n";
}
'''.replace("SENTINEL", str(sentinel)).replace("HOST", host).replace("PORT", str(port))
    try:
        isolation_request = fixture(program)
        result, elapsed = request(isolation_request)
        verify_result(result, "mac-validation-probe")
        if sentinel.read_text() != sentinel_text:
            raise AssertionError("Host sentinel changed.")
        checks.append({"name": "host-file-network-fork-denial", "passed": True, "elapsed_seconds": elapsed, "result": result})
        for label, body in (("wall-time", "if ($ARGV[0] eq '--help') { select undef, undef, undef, 30; print 'late'; }"),
                            ("output-bound", "if ($ARGV[0] eq '--help') { while (1) { print 'x' x 4096; } }")):
            result, elapsed = request(fixture(body))
            verify_result(result, "mac-validation-probe", passing=False)
            if result["validation"][0]["passed"] is not False or result["validation"][0]["exit_code"] == 0:
                raise AssertionError(label + " was not terminated.")
            if label == "wall-time" and not 14 <= elapsed < 25:
                raise AssertionError("Wall timeout did not respect the 15 second deadline.")
            if label == "output-bound" and (len(result["validation"][0]["stdout"].encode()) != 65536 or elapsed >= 5):
                raise AssertionError("Output probe did not hit the actual 64 KiB bound and terminate promptly.")
            checks.append({"name": label, "passed": True, "elapsed_seconds": elapsed, "result": result})
        data = args.package.read_bytes()
        if hashlib.sha256(data).hexdigest() != args.package_sha256:
            raise AssertionError("Exact release package hash did not match.")
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
            manifest = yaml.safe_load(archive.extractfile("apps.yaml").read())
        result, elapsed = request(package_request(data, manifest))
        verify_result(result, manifest["app_id"])
        checks.append({"name": "exact-release-package", "passed": True, "elapsed_seconds": elapsed,
                       "package_sha256": args.package_sha256, "version": manifest.get("version"), "result": result})
        if {path.name for path in (STATE / "jobs").iterdir()} != jobs_before:
            raise AssertionError("Temporary validation jobs were not cleaned up.")
        checks.append({"name": "job-cleanup", "passed": True})
    finally:
        sentinel.unlink(missing_ok=True)
    receipt = {"checked_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
               "os": platform.platform(), "machine": platform.machine(), "worker": url,
               "bootstrap": json.loads((INSTALL / "bootstrap-receipt.json").read_text()), "checks": checks}
    args.output.write_text(json.dumps(receipt, indent=2) + "\n")
    args.output.chmod(0o600)
    print(json.dumps({"receipt": str(args.output), "checks": len(checks), "passed": True,
                      "package_sha256": args.package_sha256, "reboot_recovery": "requires a separate post-reboot invocation"}))


if __name__ == "__main__":
    main()
