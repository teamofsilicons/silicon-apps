# Windows validation host prerequisites

Use a dedicated Windows Server 2025 **Core AMD64** host with hardware-assisted
nested virtualization enabled and at least 80 GiB of disk. Manage it through AWS
Systems Manager; its security group must not allow public RDP, SSH or Docker
ports. These prerequisites do not enable an Apps target or publish a package.

`windows-container-bootstrap.ps1` installs the official Docker CE/Moby Windows
engine and Python, enables Containers and Hyper-V, pulls a pinned Microsoft
Server Core image, and proves a container can run under Hyper-V isolation. The
separate `native-worker-windows.ps1` installs the authenticated TLS worker only
after this prerequisite check passes.

## Pinned inputs

`windows-container-bootstrap.json` is the reviewed download receipt. It pins:

- Docker CE 29.9.1, from Docker's official stable Windows binary directory;
- Python 3.14.8 AMD64, with the SHA-256 published on its official release page;
- the `windows/amd64` manifest behind Microsoft's `servercore:ltsc2025`, including
  its exact digest and OS version.

The script checks downloaded archive hashes and both extracted Docker executable
hashes before execution. Python additionally needs a valid Windows Authenticode
signature issued to the Python Software Foundation. There are no floating image
tags or downloaded installation scripts executed at runtime. Updating these pins
requires fresh source receipts and another Hyper-V/worker validation run.

Docker's manual binary installation has no automatic engine security updates;
the operator must maintain this pinned runtime. The flow follows Microsoft's
[Windows container setup](https://learn.microsoft.com/en-us/virtualization/windowscontainers/quick-start/set-up-environment)
and Docker's [Windows binary installation](https://docs.docker.com/engine/install/binaries/).
Python's installer hash is available on the
[3.14.8 release page](https://www.python.org/downloads/release/python-3148/).

## Run and resume through SSM

Copy the `.ps1`, `.json`, and `test-windows-container-bootstrap.ps1` files together
to a trusted local directory on the worker. Run the tests first as a separate
SSM command; they validate process argument quoting, hash failure behavior,
command failure/timeout handling, and rejection of process isolation, networked
containers, elevated users, unbounded resources and writable mounts.

```powershell
& .\test-windows-container-bootstrap.ps1
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass `
    -File .\windows-container-bootstrap.ps1
exit $LASTEXITCODE
```

Use `AWS-RunPowerShellScript` directly with a 3600-second execution timeout.
Feature installation may return **3010**. SSM then reboots the host and reruns
the command, as documented in
[Handling reboots](https://docs.aws.amazon.com/systems-manager/latest/userguide/send-commands-reboot.html).
The script persists a one-reboot limit and fails if another reboot is required.
Do not replace that mechanism with an untracked `Restart-Computer` call or wrap
it in `aws:runDocument`. When run outside SSM, 3010 means an operator must reboot
once and rerun the same script.

State and the final verification receipt live in
`C:\ProgramData\SiliconAppsContainerBootstrap`, accessible only to Administrators
and SYSTEM. The disposable self-check mount grants read access to its public
sentinel file. Docker listens only on its local named pipe, starts automatically,
and defaults to Hyper-V isolation. The script blocks inbound RDP, SSH and Docker
TCP ports and leaves outbound SSM management available. It refuses to replace an
unrelated existing Docker installation or configuration.

## Required proof before installing the worker

Only `verification.json` with `status: ready` proves bootstrap completion. It
records the installed Python path, Docker version, exact image digest, download
pins, signature thumbprint and self-check results. The real container must finish
successfully using `ContainerUser`, `--isolation=hyperv`, `--network=none`, one CPU,
1 GiB memory and a read-only package mount. Inspection must confirm all those
settings; the in-container probe verifies the user and denies a mount write.
The container is removed after success or failure. There is no process-isolated
or host-execution fallback.

Pass the recorded `python_path` and `image` to `native-worker-windows.ps1`, then
run the native worker acceptance probes and all three package discovery commands.
A ready Docker engine alone does not establish that uploaded packages validate
within the runner's 25-second command deadline.
