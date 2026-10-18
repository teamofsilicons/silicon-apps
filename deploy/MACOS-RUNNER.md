# Dedicated EC2 Mac bootstrap

`macos-bootstrap.sh` installs the native validation service only on the exact named
EC2 Mac instance. It requires a matching IMDSv2 instance ID and a `mac*.metal`
instance type before changing the host. Run it as root through SSM on an AWS macOS
AMI, which supplies AWS CLI v2 and SSM. Do not run it on a developer workstation.

The bootstrap pins Python **3.14.8** to the official macOS universal installer:

- URL: <https://www.python.org/ftp/python/3.14.8/python-3.14.8-macos11.pkg>
- SHA-256: `507fc086c5c006ff875d344a75b4e67b8fb3c401f1bc4908c6250adb673d4907`
- Developer ID Installer: `Python Software Foundation (BMM5U3QVKW)`
- Apple notarization must be trusted by `pkgutil --check-signature`.

The hash is published on the [Python release page](https://www.python.org/downloads/release/python-3148/).
Both hash and signature are checked on the target before installation. The
bootstrap never installs Python locally as part of preparing a deployment.

The instance role must be restricted to this host's worker secret and the reviewed
`releases/native-runners/` source prefix. The secret's JSON fields are `TLS_CERT`,
`TLS_KEY`, and `APPS_RUNNER_TOKEN`. The certificate must match this instance's
private IPv4 address. Keep the CA private key off the instance. Inbound TCP 4313
must be restricted to the Linux gateway security group; no SSH or public validation
listener is needed.

```sh
/bin/bash deploy/macos-bootstrap.sh \
  --instance-id i-REPLACE_WITH_DEDICATED_MAC \
  --region us-east-2 \
  --source-uri s3://BUCKET/releases/native-runners/REVIEWED_SOURCE.tar.gz \
  --source-sha256 REPLACE_WITH_VERIFIED_SOURCE_SHA256 \
  --secret-arn arn:aws:secretsmanager:us-east-2:ACCOUNT:secret:WORKER_SECRET
```

Run the bootstrap from a reviewed copy delivered by SSM, not unverified downloaded
shell text. The source archive is hashed before extraction. Secret values are
written only into a root-private temporary directory, never into SSM command
arguments or output, and temporary copies are removed on exit. The installed
service uses the locked `_siliconapps` account; its code and configuration are
root-owned. `bootstrap-receipt.json` records non-secret provenance and deliberately
states that target acceptance is still pending.

After installation, run the actual service's isolation checks and exact package
through private TLS, then reboot and repeat a package validation. The installed
tests can additionally run as the service account:

```sh
sudo -u _siliconapps env PYTHONPATH=/opt/silicon-apps-native \
  TMPDIR=/var/lib/silicon-apps-native/jobs \
  /opt/silicon-apps-native/venv/bin/python -m unittest discover \
  -s /opt/silicon-apps-native/acceptance -p test_runner.py -v
```

Deliver a reviewed `macos-acceptance.py`, the public CA certificate, and the exact
release archive to the dedicated host. Run it with the installed Python runtime:

```sh
sudo /opt/silicon-apps-native/venv/bin/python deploy/macos-acceptance.py \
  --ca-file /private/bootstrap/worker-ca.crt \
  --package /private/bootstrap/write-0.2.0-macos-aarch64.tar.gz \
  --package-sha256 REPLACE_WITH_VERIFIED_PACKAGE_SHA256 \
  --output /private/bootstrap/mac-acceptance.json
```

This sends all test packages through the running authenticated TLS service. It
checks host read/write denial against a harmless file the service account could
otherwise access, network denial with permission errors, fork denial, writable
job scratch, output and wall-time limits, and cleanup. Fork can fail with the
process-control `EAGAIN` errno as well as permission denial; it must never succeed.
TLS must reject both an untrusted CA and a mismatched hostname. The final package
must pass all three commands with the expected app ID and signed-out identity.
The script does not enable a target or treat installation as a passing receipt.

Record bounded command results and package SHA-256, host-file and network denial,
fork denial, timeout/output cleanup, authenticated TLS and rejection paths, and
restart recovery. The gateway route and API target remain disabled until these
receipts pass. See [native-worker.md](native-worker.md) for the isolation contract.
