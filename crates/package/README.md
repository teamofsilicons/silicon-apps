# silicon-apps-package

Safe Silicon Apps manifest validation, deterministic packing, checksums and extraction. This crate never executes package content.

Put `apps.yaml` at the package root:

```yaml
schema_version: 1
app_id: ring
version: 0.1.0
command: ring
targets:
  macos-aarch64:
    binary: bin/ring
    # install_script: scripts/install.sh
  windows-x86_64:
    binary: windows/ring.exe
    # install_script: scripts/install.cmd
```

All target paths are relative to the root. Supported targets: `linux-x86_64`, `linux-i686`, `linux-aarch64`, `linux-armv7hf`, `windows-x86_64`, `windows-i686`, `windows-aarch64`, `macos-x86_64`, `macos-aarch64`.

`validate_directory` returns every discoverable manifest, file and safety error together. `pack_directory` returns deterministic `.tar.gz` bytes with normalized timestamps, ownership and modes. `inspect_archive` and `extract_archive` reject absolute/parent paths, backslashes/drive prefixes, duplicate entries, links and special files; bounds are 512 MiB compressed, 1 GiB extracted and 20,000 entries. The extraction destination must be an empty directory. `sha256` returns the lowercase hex content digest.

New app IDs must contain 3–30 lowercase letters, digits, hyphens or underscores. Existing Accounts identifiers such as `dm` remain valid references and manifests; new app creation still enforces the minimum of three characters. Versions are strict `x.y.z` without prerelease or build metadata. Development and production are independent release channels, not semver prerelease suffixes.

Each binary must implement `--help`, `accounts --json` including its `app_id`, and `login status --json` including `authenticated` and its signed-in account when applicable. The server validates those commands only through configured isolated target runners. A structurally valid local package is not evidence that those runtime commands passed.
