# A local Mac release session

An explicitly approved release can be validated on its owner's Mac without a
permanent public worker or a dedicated cloud Mac. `release_session.py` runs as the
normal user, listens only on **127.0.0.1:4314**, and stops after a bounded lease
(one hour by default, at most two hours). It does not install a service, use root,
reboot the Mac, or change the public target configuration.

The operator must supply the app ID, native Mac target, exact local archive and
an independently reviewed archive SHA-256. Every request is checked against all
three identifiers and the actual archive bytes **before extraction or execution**.
Other apps, targets, archives and malformed requests are refused. The ordinary
runner then rechecks the manifest and executes only its three standard discovery
commands inside the existing `sandbox-exec` policy. It supplies a clean temporary
home with no inherited credentials and denies host files, network and forks.

Create a random session token in a regular file owned by the current user with
mode **0600**, or provide it through `APPS_RELEASE_SESSION_TOKEN`. Do not place the
token in command arguments, logs or a shared source file. Use the existing reviewed
runner virtual environment:

```sh
.dev/runner-venv/bin/python runner/release_session.py \
  --app-id write --target macos-aarch64 \
  --package /absolute/path/write-0.2.0-macos-aarch64.tar.gz \
  --package-sha256 REVIEWED_ARCHIVE_SHA256 \
  --token-file /private/path/session-token \
  --port 4314 --lease-seconds 3600
```

The startup output contains the loopback address, allowed archive hash, expiry
and process ID, never the token. Requests are serialized and body size is limited
to the approved package envelope. The final 90 seconds of a lease reject new
validations so bounded commands can finish before expiry. Ctrl-C closes the
listener and removes the temporary jobs directory.

For a production release, an operator may bridge the gateway's loopback port to
this local listener through an authenticated encrypted SSH/SSM tunnel. Restrict
both ends to loopback, keep the worker token separate from tunnel authentication,
and configure only a temporary gateway route. Prove the exact package, rejection
paths and existing sandbox controls before uploading. Remove that temporary route
and disconnect the tunnel when the release finishes or the lease closes. This
session does not claim general public Mac uploads are available.

Run the non-executing gate/HTTP tests with:

```sh
.dev/runner-venv/bin/python -m unittest discover -s runner -p test_release_session.py -v
```
