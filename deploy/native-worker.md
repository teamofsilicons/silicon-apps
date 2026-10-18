# Native validation workers

These installers configure **dedicated** macOS and Windows hosts. They do not allocate cloud infrastructure, issue certificates, install a container engine, enable API targets, or establish that uploaded packages passed. Never install them on a developer laptop or a host carrying API, Accounts, database, or unrelated cloud credentials.

## Private transport

Give each host its own random bearer token (at least 32 ASCII non-space characters) and TLS key. Issue a certificate with the worker's literal private IP in its Subject Alternative Name. Keep the CA private key off all workers. Install only the CA certificate in the Linux gateway's system trust store, then run the operating system trust updater (`update-ca-trust` on Amazon Linux). `APPS_RUNNER_WORKERS` remains an object of `{url, token}` routes; use `https://PRIVATE_IP:4313/validate`. Native workers default to port 4313, separate from the existing Linux gateway on 4312. Certificate verification remains enabled, and no public DNS or inbound public route is needed.

The API continues talking to the existing Linux gateway. Allow TCP 4313 on the new worker security group **only from the gateway security group**. Host management uses SSM; do not expose SSH, RDP, or the worker listener publicly. The runtime's IAM role should have only scoped management/bootstrap access, never the API role. Upload processes cannot access the network or host credentials.

`APPS_RUNNER_TLS_CERT` and `APPS_RUNNER_TLS_KEY` are a paired listener setting. Missing, relative, unreadable, or invalid configured paths fail startup rather than downgrading to HTTP. TLS requires 1.2 or newer. Authentication remains mandatory after TLS, and handshakes have a ten-second deadline.

## Mac

Use an Apple Silicon Mac for `macos-aarch64`; an Intel Mac is a separate target. EC2 Mac requires a Dedicated Host, sufficient host quota, and a minimum 24-hour allocation. Install native Python 3.11 or newer from a reviewed source first. Provide the reviewed repository/bundle, private-IP certificate/key and a root-readable token file:

```sh
sudo /absolute/path/to/python3 deploy/native-worker-macos.py \
  --source /absolute/path/to/silicon-apps --python /absolute/path/to/python3 \
  --token-file /private/bootstrap/worker-token \
  --tls-cert /private/bootstrap/worker.crt --tls-key /private/bootstrap/worker.key \
  --host 10.0.0.10
```

The installer creates a hidden locked `_siliconapps` account, root-owned code/configuration, private jobs directory, and a launchd service with restart/backoff. The sandbox denies network, process forks, home-directory reads, and writes outside the job. Execution retains CPU, process, output and wall-clock limits. This sandbox is not a VM or a reliable hard memory limit; use a dedicated disposable host, bound its storage, monitor pressure, and keep host secrets minimal. A package that needs fork or network during discovery fails validation; do not weaken the policy to accommodate it.

## Windows

Use Windows Server with the Containers and Hyper-V features enabled and a working Windows Docker engine. If hosted in EC2, use a type that advertises `nested-virtualization` and explicitly enable it at launch. Run the reviewed `windows-container-bootstrap.ps1` first; a Linux cross-compiler or a Windows artifact download does not establish that Hyper-V containers work. Pull and verify a compatible Microsoft Server Core image, recording its immutable digest.

```powershell
.\deploy\native-worker-windows.ps1 `
  -Source C:\bootstrap\silicon-apps -PythonPath C:\Python313\python.exe `
  -Image 'mcr.microsoft.com/windows/servercore@sha256:REPLACE_WITH_VERIFIED_DIGEST' `
  -TokenFile C:\bootstrap\worker-token -TlsCert C:\bootstrap\worker.crt `
  -TlsKey C:\bootstrap\worker.key -BindAddress 10.0.0.11 -GatewayAddress 10.0.0.5
```

The installer creates a dedicated task account with a generated password held by Windows Task Scheduler, protected service files, a boot task and a supervisor restart loop. That account is an administrator because it controls the host Docker engine; the machine must be dedicated. Package execution is inside a fresh `--isolation=hyperv` VM as `ContainerUser`, with no network, a read-only package mount, one CPU, 1 GiB memory, bounded output and a 25-second deadline. Do not substitute process isolation or a host `CreateProcess` call. Restrict and monitor Docker scratch storage on a bounded volume. Windows ARM64 and x86 are not enabled by this installer.

## Acceptance before advertising a target

1. From the actual Linux gateway, verify trusted private TLS, wrong-token rejection, and wrong-certificate/hostname rejection. A failed worker must return unavailable, with no fallback execution.
2. On the actual target host, execute the exact package archive's `--help`, `accounts --json`, and `login status --json`; record archive SHA-256, OS/architecture, isolation and complete bounded results. Verify signed-out output and matching app ID.
3. Exercise host-file read/write, outbound/metadata network, fork/process and output/timeout probes, and verify cleanup. Reboot the host and verify TLS/service recovery and a real package again. Monitor bounded disk usage and ensure no stale containers remain after crashes.
4. Only after those receipts pass, configure the gateway route and API enabled target. Publish validated packages for the required `linux-x86_64`, `windows-x86_64`, and `macos-aarch64` before promotion. Unverified targets stay unavailable.

Installer/config tests can run without changing host state. Platform installation, Windows engine isolation, resource controls, and restart recovery still require the actual dedicated hosts.
