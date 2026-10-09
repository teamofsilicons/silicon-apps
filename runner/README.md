# Run isolated package checks

Create a virtual environment, install `requirements.txt`, set `APPS_RUNNER_TOKEN` to a random
secret of at least 32 ASCII characters without spaces, then run `python runner/server.py`.
`APPS_RUNNER_HOST` defaults to `127.0.0.1`; `APPS_RUNNER_PORT` defaults to `4312`.
The Apps API sends authenticated `POST /validate` requests. Workers need no Accounts,
database, telemetry or cloud credentials. Run them on dedicated worker machines.

## Route targets to workers

A gateway can serve all nine targets using `APPS_RUNNER_WORKERS`:

```json
{
  "macos-aarch64": {"url": "https://mac-arm-worker.example/validate", "token": "REPLACE_WITH_RANDOM_WORKER_TOKEN_32_OR_MORE_CHARACTERS"},
  "macos-x86_64": {"url": "https://mac-intel-worker.example/validate", "token": "REPLACE_WITH_A_DIFFERENT_RANDOM_WORKER_TOKEN"},
  "windows-x86_64": {"url": "https://windows-worker.example/validate", "token": "REPLACE_WITH_A_DIFFERENT_RANDOM_WORKER_TOKEN"}
}
```

All nine API targets are supported as keys; route each enabled target to its provisioned
worker. Entries contain exactly `url` and `token`. Remote URLs require HTTPS with certificate
verification; plain HTTP is accepted only for literal loopback addresses or `localhost`.
URL credentials, queries, fragments and arbitrary paths are rejected. Tokens are kept in
server environment configuration and forwarded only to their configured worker. There are
no redirects, inherited HTTP proxies or fallback execution on the gateway host.

The gateway verifies package digests before forwarding, requires the exact requested target
and isolation attestation in the response, bounds upstream work to 100 seconds and response
bodies to 1 MiB, and requires all three bounded command results. Worker outages produce
503; package errors preserve the worker's exact 422 result. The API independently validates
command outputs and makes unavailable-worker responses retryable with the same key.

Set the API's `APPS_RUNNER_TARGETS` to the comma-separated targets actually provisioned.
The gateway's local worker configuration applies only to targets without a worker entry.
A failing configured worker never falls back to local execution.

## Native macOS workers

The matching macOS architecture uses a deny-by-default `sandbox-exec` policy. Only the
uploaded package and required system libraries are readable; only a disposable scratch
directory is writable. Host account files, network requests and external writes are denied.
Native validation denies all process forks; these three commands must complete in one
process (replacing that process with an executable is permitted). The runner process starts
a new session before entering the sandbox, so it cannot escape its process group by forking
and calling setsid. Commands have wall-clock, output, CPU, file and process limits, and
are terminated after completion or timeout. If an app requires subprocesses during these
checks, configure a separate VM-backed worker instead of weakening this native policy. macOS memory enforcement is not equivalent to a
VM memory cap, so run this worker on a disposable dedicated machine with host-level limits.

Native macOS command execution and filesystem/network denial have been tested on the local
Apple Silicon machine. Intel macOS needs its own matching worker before acceptance.

## Linux Docker workers

Set `APPS_RUNNER_IMAGES` to immutable image references, for example:

```json
{"linux-x86_64":"debian@sha256:REPLACE_WITH_A_VERIFIED_64_CHARACTER_HEX_DIGEST"}
```

The placeholder above must be replaced before startup; floating tags are rejected. Pull the
operator-approved image before starting the worker. The mapping also accepts `linux-i686`,
`linux-aarch64` and `linux-armv7hf`; the selected Docker platform must be available natively
or through an explicitly configured emulator. Containers run with `--platform` set to
`linux/amd64`, `linux/386`, `linux/arm64` or `linux/arm/v7` respectively, and with
`--pull=never`, so every image must already be on the host.

The production x86_64 worker serves all four. `deploy/install.py` pins one per-platform
`python:3.14-slim-trixie` manifest digest per target, pulls each with `--platform`, and writes
all four into `APPS_RUNNER_IMAGES`. `linux-i686` runs natively as a 32-bit process.
`linux-aarch64` and `linux-armv7hf` run under QEMU user-mode emulation: the installer
registers `qemu-aarch64` and `qemu-arm` (only those two) in binfmt_misc with the F flag
through a digest-pinned `tonistiigi/binfmt` image, and a oneshot unit,
`silicon-apps-binfmt.service`, registers them again at every boot before the worker starts.
With the F flag the kernel holds the emulator open, so nothing is added to the validation
containers. The binfmt entries also carry the C flag; the containers' `no-new-privileges`
setting means that never grants credentials to an emulated program. Emulation is slower than
native execution, but every command keeps the same 25 second wall-clock limit.

Before the worker is declared ready, the installer runs `python3` in each pinned image under
its platform, with the isolation flags below, and requires the expected `uname -m` and word
size: `x86_64` and 64-bit; `i686` (or the host's `x86_64`) and 32-bit; `aarch64` and 64-bit;
`armv7l` (or `armv8l`) and 32-bit. A target that does not run fails the install by name.
`deploy/PRODUCTION.md` describes the full worker install and rollback.

Containers have no network, read-only root and package mounts, dropped capabilities,
no-new-privileges, a non-root user, 256 MiB memory, one CPU, 32 processes, and a bounded
scratch tmpfs. Do not mount the Docker engine socket into the Apps API.

## Windows Hyper-V workers

On a Windows host with Hyper-V container isolation and a compatible Docker engine, configure
an immutable Windows image per target in `APPS_RUNNER_IMAGES`. The same Python worker then
launches a fresh `--isolation=hyperv` container, requires `ContainerUser`, mounts the package
read-only at `C:\package`, disables networking, and limits the utility VM to 1 GiB and one
CPU. The container is forcibly removed after completion or timeout.

`windows-x86_64` uses `windows/amd64`; `windows-i686` also requires an amd64 image with
WoW64 support. `windows-aarch64` requires a worker and container engine that actually support
`windows/arm64`; a manifest entry alone is insufficient. Container launch failures remain
visible and never cause execution on the host. The deployment must provide the matching
Windows runtime and image. Windows CPU/isolation options follow the
[Docker run reference](https://docs.docker.com/reference/cli/docker/container/run/) and
[Microsoft resource controls](https://learn.microsoft.com/en-us/virtualization/windowscontainers/manage-containers/resource-controls).

Linux and Windows command construction and routing are covered by tests. No native Linux
Docker or Windows Hyper-V engine was available in the implementation environment; live
validation on those targets remains an operator deployment check. For the four Linux
targets, the worker installer's per-platform self-check and `deploy/verify-worker.py
--target` are that check.

## Protocol and validation

Each request contains `app_id`, `target`, `package_sha256`, `package_base64`, and the parsed
`manifest`. The worker checks the digest, independently compares archived `apps.yaml`, and
rejects traversal, Windows device paths, links, special files, duplicate entries and
expansion-limit violations. It then executes:

1. `--help`: exit zero with nonempty help text.
2. `accounts --json`: exit zero and JSON `app_id` equal to the submitted app.
3. `login status --json`: exit zero and `authenticated:false` in the clean signed-out sandbox.

The response is `{isolated:true,target,validation:[{command,exit_code,stdout,stderr,passed,expected}]}`.
The worker never executes install scripts. Limits are 128 MiB compressed, 512 MiB expanded,
20,000 entries, and 64 KiB captured output per command stream. Uploads have a 30-second
deadline. Run verification with `python -m unittest discover -s runner -v`.
