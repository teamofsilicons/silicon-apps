#!/usr/bin/env python3
"""Install a TLS validation worker on a dedicated Apple Silicon/Intel Mac. Run as root."""
import argparse
import json
import ipaddress
import os
from pathlib import Path
import platform
import plistlib
import pwd
import grp
import shutil
import subprocess

SERVICE = "com.teamofsilicons.apps-native-runner"
ACCOUNT = "_siliconapps"
INSTALL = Path("/opt/silicon-apps-native")
STATE = Path("/var/lib/silicon-apps-native")


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def account():
    try:
        user = pwd.getpwnam(ACCOUNT)
    except KeyError:
        user = None
    try:
        group = grp.getgrnam(ACCOUNT)
    except KeyError:
        group = None
    if user is not None or group is not None:
        if user is None or group is None or user.pw_uid == 0 or user.pw_gid != group.gr_gid or user.pw_shell != "/usr/bin/false" or user.pw_dir != "/var/empty":
            raise ValueError("Existing runner account is not the expected locked service account.")
        return user.pw_uid, group.gr_gid
    else:
        used = {row.pw_uid for row in pwd.getpwall()} | {row.gr_gid for row in grp.getgrall()}
        uid = next(value for value in range(350, 500) if value not in used)
        run("/usr/bin/dscl", ".", "-create", f"/Groups/{ACCOUNT}")
        run("/usr/bin/dscl", ".", "-create", f"/Groups/{ACCOUNT}", "PrimaryGroupID", str(uid))
        for field, value in {"UniqueID": str(uid), "PrimaryGroupID": str(uid), "UserShell": "/usr/bin/false", "NFSHomeDirectory": "/var/empty", "IsHidden": "1", "Password": "*"}.items():
            run("/usr/bin/dscl", ".", "-create", f"/Users/{ACCOUNT}", field, value)
        return uid, uid


def launch_configuration(python):
    return {"Label": SERVICE, "UserName": ACCOUNT, "GroupName": ACCOUNT,
            "ProgramArguments": [str(python), str(INSTALL / "launch.py"), "--config", str(INSTALL / "worker.json"), "--server", str(INSTALL / "server.py")],
            "WorkingDirectory": str(STATE), "EnvironmentVariables": {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
            "RunAtLoad": True, "KeepAlive": True, "ThrottleInterval": 10, "ExitTimeOut": 20,
            "ProcessType": "Background", "Umask": 0o077,
            "HardResourceLimits": {"NumberOfFiles": 256, "NumberOfProcesses": 64, "Core": 0},
            "StandardOutPath": str(STATE / "runner.log"), "StandardErrorPath": str(STATE / "runner-error.log")}


def readable_runtime(root, gid):
    """A caller's umask must not make the root-owned venv unreadable to launchd."""
    for path in [root, *root.rglob("*")]:
        if path.is_symlink():
            # Do not change the global Python installation through venv links.
            os.chown(path, 0, gid, follow_symlinks=False)
            continue
        executable = path.is_dir() or bool(path.stat().st_mode & 0o111)
        os.chown(path, 0, gid)
        path.chmod(0o750 if executable else 0o640)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help="Reviewed silicon-apps source checkout/bundle")
    parser.add_argument("--python", type=Path, required=True, help="Installed native Python 3.11+ executable")
    parser.add_argument("--token-file", type=Path, required=True)
    parser.add_argument("--tls-cert", type=Path, required=True, help="PEM certificate chain with this host's private IP SAN")
    parser.add_argument("--tls-key", type=Path, required=True)
    parser.add_argument("--host", required=True, help="Private interface IP; firewall must allow only gateway")
    parser.add_argument("--port", type=int, default=4313)
    args = parser.parse_args()
    if platform.system() != "Darwin" or os.geteuid() != 0 or not Path("/usr/bin/sandbox-exec").is_file():
        parser.error("A root installation on a dedicated Mac with sandbox-exec is required.")
    token = args.token_file.read_text().strip()
    if len(token) < 32 or not token.isascii() or any(char.isspace() for char in token):
        parser.error("The worker token file must contain at least 32 ASCII non-space characters.")
    if not args.python.is_absolute() or not args.python.is_file() or not 1 <= args.port <= 65535:
        parser.error("Provide an absolute installed Python executable and valid port.")
    address = ipaddress.ip_address(args.host)
    if not address.is_private or address.is_loopback or address.is_unspecified or address.is_multicast:
        parser.error("The listener must bind to a literal private interface IP.")
    uid, gid = account()
    INSTALL.mkdir(parents=True, exist_ok=True, mode=0o750)
    os.chown(INSTALL, 0, gid)
    INSTALL.chmod(0o750)
    for path in (STATE, STATE / "jobs"):
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
        os.chown(path, uid, gid)
        path.chmod(0o700)
    # Stop before replacing code/config. A failed setup remains unavailable, never
    # executing uploads with a half-updated sandbox or previous credentials.
    subprocess.run(["/bin/launchctl", "bootout", f"system/{SERVICE}"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
    files = {args.source / "runner/server.py": ("server.py", 0o640), args.source / "deploy/native-worker-launch.py": ("launch.py", 0o640),
             args.tls_cert: ("server.crt", 0o640), args.tls_key: ("server.key", 0o640)}
    for source, (name, mode) in files.items():
        destination = INSTALL / name
        shutil.copyfile(source, destination)
        os.chown(destination, 0, gid)
        destination.chmod(mode)
    python = INSTALL / "venv/bin/python"
    if not python.is_file():
        run(str(args.python), "-m", "venv", str(INSTALL / "venv"))
    run(str(python), "-m", "pip", "install", "--disable-pip-version-check", "-r", str(args.source / "runner/requirements.txt"))
    readable_runtime(INSTALL / "venv", gid)
    config = {"APPS_RUNNER_TOKEN": token, "APPS_RUNNER_HOST": args.host, "APPS_RUNNER_PORT": str(args.port),
              "APPS_RUNNER_TLS_CERT": str(INSTALL / "server.crt"), "APPS_RUNNER_TLS_KEY": str(INSTALL / "server.key"),
              "APPS_RUNNER_IMAGES": "{}", "TMPDIR": str(STATE / "jobs")}
    config_path = INSTALL / "worker.json"
    config_path.write_text(json.dumps(config))
    os.chown(config_path, 0, gid)
    config_path.chmod(0o640)
    plist_path = Path("/Library/LaunchDaemons") / f"{SERVICE}.plist"
    plist_path.write_bytes(plistlib.dumps(launch_configuration(python)))
    os.chown(plist_path, 0, 0)
    plist_path.chmod(0o644)
    run("/bin/launchctl", "bootstrap", "system", str(plist_path))
    run("/bin/launchctl", "kickstart", "-k", f"system/{SERVICE}")
    print("Native Mac worker installed. Verify TLS, authenticated sandbox probes, reboot recovery, and package receipts before enabling this target.")


if __name__ == "__main__":
    main()
