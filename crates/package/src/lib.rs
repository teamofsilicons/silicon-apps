//! Portable, deterministic Silicon Apps archives. No function executes package content.
use anyhow::{Context, Result, bail, ensure};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read},
    path::{Component, Path},
};

pub const TARGETS: [&str; 9] = [
    "linux-x86_64",
    "linux-i686",
    "linux-aarch64",
    "linux-armv7hf",
    "windows-x86_64",
    "windows-i686",
    "windows-aarch64",
    "macos-x86_64",
    "macos-aarch64",
];
pub const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_EXTRACTED_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    pub app_id: String,
    pub version: String,
    pub command: String,
    pub targets: BTreeMap<String, Target>,
}
fn schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub binary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_script: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub errors: Vec<String>,
    pub manifest: Option<Manifest>,
}

pub fn valid_app_id(value: &str) -> bool {
    value.len() >= 3 && valid_existing_app_id(value)
}

/// Historical Accounts apps (for example `dm`) retain their existing identifiers.
/// New app creation still uses [`valid_app_id`]; manifests reference an existing app.
pub fn valid_existing_app_id(value: &str) -> bool {
    (1..=30).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub fn strict_version(value: &str) -> bool {
    semver::Version::parse(value)
        .is_ok_and(|v| v.pre.is_empty() && v.build.is_empty() && v.to_string() == value)
}

pub fn safe_path(value: &Path) -> bool {
    !value.as_os_str().is_empty()
        && value.components().all(|c| match c {
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
                !part.ends_with(['.', ' '])
                    && !part.chars().any(|c| c.is_control())
                    && !matches!(
                        base.as_str(),
                        "CON"
                            | "PRN"
                            | "AUX"
                            | "NUL"
                            | "COM1"
                            | "COM2"
                            | "COM3"
                            | "COM4"
                            | "COM5"
                            | "COM6"
                            | "COM7"
                            | "COM8"
                            | "COM9"
                            | "LPT1"
                            | "LPT2"
                            | "LPT3"
                            | "LPT4"
                            | "LPT5"
                            | "LPT6"
                            | "LPT7"
                            | "LPT8"
                            | "LPT9"
                    )
            }
            _ => false,
        })
        && !value
            .to_string_lossy()
            .contains(['\\', ':', '\0', '<', '>', '"', '|', '?', '*'])
}

/// Filesystem paths use native separators; archive and manifest paths always use `/`.
fn portable_filesystem_path(path: &Path) -> Result<String> {
    path.components()
        .map(|component| match component {
            Component::Normal(part) => part
                .to_str()
                .map(str::to_owned)
                .context("package filenames must be valid UTF-8"),
            _ => bail!("package file path must contain only relative normal components"),
        })
        .collect::<Result<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

impl Manifest {
    pub fn errors(&self) -> Vec<String> {
        let mut errors = vec![];
        if self.schema_version != 1 {
            errors.push("schema_version: expected 1".into());
        }
        if !valid_existing_app_id(&self.app_id) {
            errors.push(
                "app_id: expected an existing 1–30 character lowercase app identifier; new IDs require at least 3 characters".into(),
            );
        }
        if !strict_version(&self.version) {
            errors.push("version: expected x.y.z without prerelease or build metadata".into());
        }
        if self.command.is_empty()
            || self.command.len() > 80
            || !safe_path(Path::new(&self.command))
            || !self
                .command
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            errors.push("command: expected 1–80 letters, digits, hyphens or underscores, without a path or extension".into());
        }
        if self.targets.is_empty() {
            errors.push("targets: at least one supported target is required".into());
        }
        for (name, target) in &self.targets {
            if !TARGETS.contains(&name.as_str()) {
                errors.push(format!(
                    "targets.{name}: unsupported target; choose from {}",
                    TARGETS.join(", ")
                ));
            }
            for (field, path) in [
                ("binary", Some(&target.binary)),
                ("install_script", target.install_script.as_ref()),
            ] {
                if let Some(path) = path
                    && !safe_path(Path::new(path))
                {
                    errors.push(format!("targets.{name}.{field}: `{path}` must be a relative path without parent segments, drive prefixes or backslashes"));
                }
            }
        }
        errors
    }
}

// Parse fields independently so a missing/incorrect field does not hide other errors.
fn parse_manifest(bytes: &[u8]) -> (Option<Manifest>, Vec<String>) {
    use serde_yaml::Value;
    let value: Value = match serde_yaml::from_slice(bytes) {
        Ok(value) => value,
        Err(error) => return (None, vec![format!("apps.yaml: invalid YAML: {error}")]),
    };
    let Some(root) = value.as_mapping() else {
        return (
            None,
            vec!["apps.yaml: expected a mapping of manifest fields".into()],
        );
    };
    fn unknown(map: &serde_yaml::Mapping, fields: &[&str], prefix: &str, errors: &mut Vec<String>) {
        for key in map.keys() {
            if !key.as_str().is_some_and(|key| fields.contains(&key)) {
                errors.push(format!(
                    "{prefix}: unknown field {}",
                    key.as_str().unwrap_or("<non-string key>")
                ));
            }
        }
    }
    fn string(
        map: &serde_yaml::Mapping,
        key: &str,
        prefix: &str,
        errors: &mut Vec<String>,
    ) -> String {
        match map.get(Value::String(key.into())).and_then(Value::as_str) {
            Some(value) => value.to_owned(),
            None => {
                errors.push(format!(
                    "{prefix}{key}: required string is missing or has the wrong type"
                ));
                String::new()
            }
        }
    }
    let mut errors = Vec::new();
    unknown(
        root,
        &["schema_version", "app_id", "version", "command", "targets"],
        "apps.yaml",
        &mut errors,
    );
    let schema_version = match root.get(Value::String("schema_version".into())) {
        None => 1,
        Some(value) => match value.as_u64().and_then(|n| u32::try_from(n).ok()) {
            Some(value) => value,
            None => {
                errors.push("schema_version: expected a positive integer".into());
                1
            }
        },
    };
    let app_id = string(root, "app_id", "", &mut errors);
    let version = string(root, "version", "", &mut errors);
    let command = string(root, "command", "", &mut errors);
    let mut targets = BTreeMap::new();
    match root
        .get(Value::String("targets".into()))
        .and_then(Value::as_mapping)
    {
        Some(map) => {
            for (name, value) in map {
                let Some(name) = name.as_str() else {
                    errors.push("targets: every target name must be a string".into());
                    continue;
                };
                let Some(fields) = value.as_mapping() else {
                    errors.push(format!(
                        "targets.{name}: expected binary and optional install_script fields"
                    ));
                    continue;
                };
                unknown(
                    fields,
                    &["binary", "install_script"],
                    &format!("targets.{name}"),
                    &mut errors,
                );
                let binary = string(fields, "binary", &format!("targets.{name}."), &mut errors);
                let install_script = match fields.get(Value::String("install_script".into())) {
                    None | Some(Value::Null) => None,
                    Some(value) => match value.as_str() {
                        Some(value) => Some(value.into()),
                        None => {
                            errors
                                .push(format!("targets.{name}.install_script: expected a string"));
                            None
                        }
                    },
                };
                targets.insert(
                    name.into(),
                    Target {
                        binary,
                        install_script,
                    },
                );
            }
        }
        None => {
            errors.push("targets: required target mapping is missing or has the wrong type".into())
        }
    }
    let manifest = Manifest {
        schema_version,
        app_id,
        version,
        command,
        targets,
    };
    // Do not repeat a semantic error for a field already rejected structurally.
    for error in manifest.errors() {
        let field = error.split(':').next().unwrap_or("");
        if !errors
            .iter()
            .any(|existing| existing.starts_with(&format!("{field}:")))
        {
            errors.push(error);
        }
    }
    (Some(manifest), errors)
}

pub fn validate_directory(root: &Path) -> ValidationReport {
    let mut report = ValidationReport::default();
    if !root.is_dir() {
        report
            .errors
            .push(format!("{}: not a directory", root.display()));
        return report;
    }
    let (manifest, errors) = match fs::read(root.join("apps.yaml")) {
        Ok(bytes) => parse_manifest(&bytes),
        Err(error) => (
            None,
            vec![format!("apps.yaml: cannot read required manifest: {error}")],
        ),
    };
    report.errors.extend(errors);
    if let Some(manifest) = &manifest {
        for (name, target) in &manifest.targets {
            for (field, path) in [
                ("binary", Some(&target.binary)),
                ("install_script", target.install_script.as_ref()),
            ] {
                if let Some(path) = path
                    && safe_path(Path::new(path))
                {
                    match fs::symlink_metadata(root.join(path)) {
                        Ok(meta) if meta.file_type().is_file() => {}
                        _ => report.errors.push(format!(
                            "targets.{name}.{field}: `{path}` must be an existing regular file"
                        )),
                    }
                }
            }
        }
    }
    let mut size = 0;
    let mut count = 0;
    for entry in walkdir::WalkDir::new(root).follow_links(false).min_depth(1) {
        match entry {
            Ok(e) => {
                count += 1;
                let portable =
                    portable_filesystem_path(e.path().strip_prefix(root).unwrap_or(e.path()));
                if !portable
                    .as_ref()
                    .is_ok_and(|path| safe_path(Path::new(path)))
                {
                    report.errors.push(format!(
                        "{}: unsafe cross-platform package path",
                        e.path().display()
                    ));
                }
                if e.file_type().is_symlink()
                    || !(e.file_type().is_file() || e.file_type().is_dir())
                {
                    report.errors.push(format!(
                        "{}: links and special files are not allowed",
                        e.path().display()
                    ));
                }
                if let Ok(m) = e.metadata() {
                    size += m.len();
                }
            }
            Err(e) => report.errors.push(e.to_string()),
        }
    }
    if count > MAX_ENTRIES {
        report.errors.push(format!(
            "archive has {count} entries; maximum is {MAX_ENTRIES}"
        ));
    }
    if size > MAX_EXTRACTED_BYTES {
        report.errors.push(format!(
            "uncompressed package exceeds {MAX_EXTRACTED_BYTES} bytes"
        ));
    }
    report.valid = report.errors.is_empty();
    report.manifest = manifest;
    report
}

/// Build identical bytes for identical file content and executable modes, independent of timestamps.
pub fn pack_directory(root: &Path) -> Result<Vec<u8>> {
    let report = validate_directory(root);
    ensure!(
        report.valid,
        "package validation failed:\n{}",
        report.errors.join("\n")
    );
    let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let mut paths = walkdir::WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    paths.sort_by(|a, b| a.path().cmp(b.path()));
    for entry in paths {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = portable_filesystem_path(entry.path().strip_prefix(root)?)?;
        ensure!(
            safe_path(Path::new(&relative)),
            "unsafe archive path {}",
            relative
        );
        let mut file = fs::File::open(entry.path())?;
        let metadata = file.metadata()?;
        let mut header = tar::Header::new_gnu();
        header.set_size(metadata.len());
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            header.set_mode(if metadata.permissions().mode() & 0o111 != 0 {
                0o755
            } else {
                0o644
            });
        }
        #[cfg(not(unix))]
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, &relative, &mut file)?;
    }
    let bytes = archive.into_inner()?.finish()?;
    ensure!(
        bytes.len() as u64 <= MAX_ARCHIVE_BYTES,
        "compressed package exceeds {MAX_ARCHIVE_BYTES} bytes"
    );
    Ok(bytes)
}

pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Inspect without executing any content. Reject path traversal, duplicate entries, hard/symlinks,
/// device files, bombs and missing target artifacts before returning the manifest.
pub fn inspect_archive(bytes: &[u8]) -> Result<Manifest> {
    let temp = tempfile::tempdir()?;
    extract_archive(bytes, temp.path())
}

/// Extract into a new, empty directory owned by the caller. Never follows archive links.
pub fn extract_archive(bytes: &[u8], destination: &Path) -> Result<Manifest> {
    ensure!(
        bytes.len() as u64 <= MAX_ARCHIVE_BYTES,
        "compressed package exceeds {MAX_ARCHIVE_BYTES} bytes"
    );
    ensure!(
        destination.is_dir(),
        "{}: extraction destination is not a directory",
        destination.display()
    );
    ensure!(
        fs::read_dir(destination)?.next().is_none(),
        "extraction destination must be empty"
    );
    ensure!(
        !fs::symlink_metadata(destination)?.file_type().is_symlink(),
        "extraction destination must not be a symlink"
    );
    let reader = GzDecoder::new(Cursor::new(bytes))
        .take(MAX_EXTRACTED_BYTES + (MAX_ENTRIES as u64 * 1024) + 1);
    let mut archive = tar::Archive::new(reader);
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for entry in archive
        .entries()
        .context("package must be a valid .tar.gz")?
    {
        let mut entry = entry.context("invalid tar entry")?;
        let path = entry.path()?.into_owned();
        ensure!(safe_path(&path), "unsafe archive path {}", path.display());
        ensure!(
            seen.insert(path.clone()),
            "duplicate archive path {}",
            path.display()
        );
        ensure!(
            seen.len() <= MAX_ENTRIES,
            "archive contains too many entries"
        );
        let kind = entry.header().entry_type();
        ensure!(
            kind.is_file() || kind.is_dir(),
            "{}: links and special archive files are not allowed",
            path.display()
        );
        total = total
            .checked_add(entry.size())
            .context("archive size overflow")?;
        ensure!(
            total <= MAX_EXTRACTED_BYTES,
            "uncompressed archive is too large"
        );
        let output = destination.join(&path);
        if kind.is_dir() {
            fs::create_dir_all(&output)?;
            continue;
        }
        fs::create_dir_all(output.parent().context("file has no parent")?)?;
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)?;
        std::io::copy(&mut entry, &mut out)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                &output,
                fs::Permissions::from_mode(if entry.header().mode()? & 0o111 != 0 {
                    0o755
                } else {
                    0o644
                }),
            )?;
        }
    }
    let report = validate_directory(destination);
    ensure!(
        report.valid,
        "package validation failed:\n{}",
        report.errors.join("\n")
    );
    match report.manifest {
        Some(m) => Ok(m),
        None => bail!("apps.yaml: missing manifest"),
    }
}

/// An install script as packaged for one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallScript {
    /// The script's path inside the package, as written in apps.yaml.
    pub path: String,
    /// SHA-256 of the script's exact bytes, lowercase hex.
    pub sha256: String,
    pub size: u64,
}
/// Install scripts larger than this are refused when read for display or signing.
pub const MAX_INSTALL_SCRIPT_BYTES: u64 = 16 * 1024 * 1024;

/// Read one regular file from a `.tar.gz` without extracting anything else.
/// Returns `None` when the archive has no regular file at `path`. Leading `./`
/// segments are ignored on both sides.
pub fn read_archive_file(bytes: &[u8], path: &str, limit: u64) -> Result<Option<Vec<u8>>> {
    fn normal(path: &str) -> String {
        path.split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect::<Vec<_>>()
            .join("/")
    }
    let wanted = normal(path);
    let reader = GzDecoder::new(Cursor::new(bytes))
        .take(MAX_EXTRACTED_BYTES + (MAX_ENTRIES as u64 * 1024) + 1);
    let mut archive = tar::Archive::new(reader);
    for entry in archive
        .entries()
        .context("package must be a valid .tar.gz")?
    {
        let mut entry = entry.context("invalid tar entry")?;
        let entry_path = entry.path()?.to_string_lossy().replace('\\', "/");
        if normal(&entry_path) != wanted || !entry.header().entry_type().is_file() {
            continue;
        }
        ensure!(
            entry.size() <= limit,
            "{path} is {} bytes, more than the {limit} byte limit",
            entry.size()
        );
        let mut content = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut content)?;
        return Ok(Some(content));
    }
    Ok(None)
}

/// The install script `target` runs, with its digest and bytes, or `None`
/// when the target has no install script. Check the archive with
/// [`inspect_archive`] first; this reads only the script.
pub fn install_script(
    bytes: &[u8],
    manifest: &Manifest,
    target: &str,
) -> Result<Option<(InstallScript, Vec<u8>)>> {
    let Some(path) = manifest
        .targets
        .get(target)
        .with_context(|| format!("the package does not describe target {target}"))?
        .install_script
        .clone()
    else {
        return Ok(None);
    };
    let content =
        read_archive_file(bytes, &path, MAX_INSTALL_SCRIPT_BYTES)?.with_context(|| {
            format!("apps.yaml names install script {path}, but the archive has no such file")
        })?;
    Ok(Some((
        InstallScript {
            path,
            sha256: sha256(&content),
            size: content.len() as u64,
        },
        content,
    )))
}

pub fn current_target() -> Result<&'static str> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match (os, arch) {
        ("linux", "x86_64") => Ok("linux-x86_64"),
        ("linux", "x86") => Ok("linux-i686"),
        ("linux", "aarch64") => Ok("linux-aarch64"),
        ("linux", "arm") => Ok("linux-armv7hf"),
        ("windows", "x86_64") => Ok("windows-x86_64"),
        ("windows", "x86") => Ok("windows-i686"),
        ("windows", "aarch64") => Ok("windows-aarch64"),
        ("macos", "x86_64") => Ok("macos-x86_64"),
        ("macos", "aarch64") => Ok("macos-aarch64"),
        _ => bail!(
            "unsupported OS/architecture {os}-{arch}; supported targets: {}",
            TARGETS.join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_reports_all_fields() {
        let m = Manifest {
            schema_version: 8,
            app_id: "X".into(),
            version: "1".into(),
            command: "../../bad".into(),
            targets: BTreeMap::new(),
        };
        assert_eq!(m.errors().len(), 5);
    }
    #[test]
    fn validates_every_target_and_reports_unreferenced_unsafe_files() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = Manifest {
            schema_version: 1,
            app_id: "portable".into(),
            version: "1.2.3".into(),
            command: "portable".into(),
            targets: TARGETS
                .iter()
                .map(|target| {
                    (
                        target.to_string(),
                        Target {
                            binary: format!(
                                "bin/{target}/portable{}",
                                if target.starts_with("windows") {
                                    ".exe"
                                } else {
                                    ""
                                }
                            ),
                            install_script: None,
                        },
                    )
                })
                .collect(),
        };
        fs::write(
            dir.path().join("apps.yaml"),
            serde_yaml::to_string(&manifest).unwrap(),
        )
        .unwrap();
        for target in manifest.targets.values() {
            let binary = dir.path().join(&target.binary);
            fs::create_dir_all(binary.parent().unwrap()).unwrap();
            fs::write(binary, b"compiled target fixture").unwrap();
        }
        let archive = pack_directory(dir.path()).unwrap();
        assert_eq!(inspect_archive(&archive).unwrap().targets.len(), 9);
        #[cfg(unix)]
        {
            fs::write(dir.path().join("NUL.txt"), b"not portable").unwrap();
            let report = validate_directory(dir.path());
            assert!(!report.valid);
            assert!(
                report
                    .errors
                    .iter()
                    .any(|error| error.contains("unsafe cross-platform package path"))
            );
        }
    }

    #[test]
    fn paths_are_cross_platform_safe() {
        assert_eq!(
            portable_filesystem_path(&Path::new("bin").join("native").join("apps")).unwrap(),
            "bin/native/apps"
        );
        for p in ["../evil", "/evil", "bin/../evil", "C:evil", "bin\\evil", ""] {
            assert!(!safe_path(Path::new(p)), "{p}");
        }
        for p in [
            "bin/NUL.exe",
            "bin/COM1",
            "bin/trailing.",
            "bin/trailing ",
            "bin/has?mark",
        ] {
            assert!(!safe_path(Path::new(p)), "{p}");
        }
        assert!(safe_path(Path::new("bin/cli")));
    }
    #[test]
    fn deterministic_roundtrip_and_missing_binary() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("apps.yaml"), "schema_version: 1\napp_id: example\nversion: 1.2.3\ncommand: example\ntargets:\n  linux-x86_64:\n    binary: example\n").unwrap();
        assert!(!validate_directory(dir.path()).valid);
        fs::write(dir.path().join("example"), "hello").unwrap();
        let first = pack_directory(dir.path()).unwrap();
        assert_eq!(first, pack_directory(dir.path()).unwrap());
        assert_eq!(inspect_archive(&first).unwrap().app_id, "example");
    }
    #[test]
    fn reads_the_install_script_of_a_target_without_extracting() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("apps.yaml"), "schema_version: 1\napp_id: example\nversion: 1.2.3\ncommand: example\ntargets:\n  linux-x86_64:\n    binary: example\n    install_script: scripts/setup.sh\n  macos-aarch64:\n    binary: example\n").unwrap();
        fs::write(dir.path().join("example"), "hello").unwrap();
        fs::create_dir_all(dir.path().join("scripts")).unwrap();
        fs::write(
            dir.path().join("scripts/setup.sh"),
            "#!/bin/sh\necho setup\n",
        )
        .unwrap();
        let bytes = pack_directory(dir.path()).unwrap();
        let manifest = inspect_archive(&bytes).unwrap();
        let (script, content) = install_script(&bytes, &manifest, "linux-x86_64")
            .unwrap()
            .unwrap();
        assert_eq!(content, b"#!/bin/sh\necho setup\n");
        assert_eq!(script.path, "scripts/setup.sh");
        assert_eq!(script.sha256, sha256(b"#!/bin/sh\necho setup\n"));
        assert_eq!(script.size, 21);
        assert!(
            install_script(&bytes, &manifest, "macos-aarch64")
                .unwrap()
                .is_none()
        );
        assert!(install_script(&bytes, &manifest, "windows-x86_64").is_err());
        assert_eq!(
            read_archive_file(&bytes, "./scripts/setup.sh", 1024)
                .unwrap()
                .unwrap(),
            content
        );
        assert!(
            read_archive_file(&bytes, "missing", 1024)
                .unwrap()
                .is_none()
        );
        assert!(read_archive_file(&bytes, "scripts/setup.sh", 4).is_err());
    }
    #[test]
    fn rejects_symlink_archive() {
        let mut a = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
        let mut h = tar::Header::new_gnu();
        h.set_size(0);
        h.set_mode(0o777);
        h.set_entry_type(tar::EntryType::Symlink);
        h.set_link_name("/tmp").unwrap();
        h.set_cksum();
        a.append_data(&mut h, "escape", Cursor::new([])).unwrap();
        let bytes = a.into_inner().unwrap().finish().unwrap();
        assert!(
            inspect_archive(&bytes)
                .unwrap_err()
                .to_string()
                .contains("links")
        );
    }
}
