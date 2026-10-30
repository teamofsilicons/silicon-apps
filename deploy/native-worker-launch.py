#!/usr/bin/env python3
"""Launch the reviewed runner with a small environment, never an interactive user's home."""
import argparse
import json
import os
from pathlib import Path
import runpy


def environment(config, inherited):
    required = {"APPS_RUNNER_TOKEN", "APPS_RUNNER_HOST", "APPS_RUNNER_PORT", "APPS_RUNNER_TLS_CERT", "APPS_RUNNER_TLS_KEY", "APPS_RUNNER_IMAGES", "TMPDIR"}
    if not isinstance(config, dict) or set(config) != required or any(not isinstance(value, str) for value in config.values()):
        raise ValueError("Native worker configuration must contain exactly the documented string fields.")
    if not config["APPS_RUNNER_TLS_CERT"] or not config["APPS_RUNNER_TLS_KEY"]:
        raise ValueError("Native workers require TLS certificate and key paths.")
    for name in ("APPS_RUNNER_TLS_CERT", "APPS_RUNNER_TLS_KEY", "TMPDIR"):
        if not Path(config[name]).is_absolute():
            raise ValueError(f"{name} must be an absolute path.")
    # Docker's Windows client needs OS discovery paths. No AWS, Accounts, proxy,
    # interactive-home or other inherited credentials enter the service environment.
    result = {key: inherited[key] for key in ("PATH", "SystemRoot", "SYSTEMROOT", "WINDIR", "COMSPEC", "ProgramData", "PROGRAMDATA") if key in inherited}
    result.update(config)
    result.update({"HOME": config["TMPDIR"], "TEMP": config["TMPDIR"], "TMP": config["TMPDIR"], "SILICON_HOME": config["TMPDIR"], "PYTHONUNBUFFERED": "1"})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("--server", required=True, type=Path)
    args = parser.parse_args()
    config = json.loads(args.config.read_text(encoding="utf-8-sig"))
    clean = environment(config, os.environ)
    os.environ.clear()
    os.environ.update(clean)
    os.umask(0o077)
    runpy.run_path(str(args.server.resolve()), run_name="__main__")


if __name__ == "__main__":
    main()
