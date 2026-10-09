//! Checksum-verified transactional local installs with rollback and one updater.
use crate::{
    Client, Config, LocalState, Package, Release, Resolution, package, state as persistence,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, path::Path, str::FromStr, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallSpec {
    pub app_id: String,
    pub channel: String,
    pub version: Option<String>,
}
impl FromStr for InstallSpec {
    type Err = anyhow::Error;
    fn from_str(input: &str) -> Result<Self> {
        let mut parts = input.split('@');
        let name = parts.next().unwrap_or("");
        let version = parts.next().map(str::to_owned);
        ensure!(
            parts.next().is_none(),
            "invalid install spec `{input}`; use app, 'app>dev', 'app@1.2.3' or 'app>dev@1.2.3'"
        );
        let (id, channel) = if let Some(id) = name.strip_suffix(">dev") {
            (id, "development")
        } else {
            (name, "production")
        };
        ensure!(
            package::valid_existing_app_id(id),
            "invalid app_id `{id}`; expected an existing 1–30 character lowercase app identifier"
        );
        if let Some(v) = &version {
            ensure!(
                package::strict_version(v),
                "invalid version `{v}`; expected x.y.z"
            );
        }
        Ok(Self {
            app_id: id.into(),
            channel: channel.into(),
            version,
        })
    }
}
impl std::fmt::Display for InstallSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.app_id,
            if self.channel == "development" {
                ">dev"
            } else {
                ""
            },
            self.version
                .as_ref()
                .map(|v| format!("@{v}"))
                .unwrap_or_default()
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    pub app_id: String,
    pub channel: String,
    pub version: String,
    pub target: String,
    pub command: String,
    pub release_id: String,
    pub package_id: String,
    pub sha256: String,
    pub installed_at: String,
    #[serde(default)]
    pub server: String,
    /// The API key that signed this release, when it came from a registry.
    #[serde(default)]
    pub signature_key_id: Option<String>,
    /// The author key that also signed the package, if any.
    #[serde(default)]
    pub author_key_id: Option<String>,
    /// The c:id or si:id that signed as author, if any.
    #[serde(default)]
    pub signed_by: Option<String>,
    /// The install script's SHA-256, or `none` when the target has none.
    /// Missing for installs made before this was recorded.
    #[serde(default)]
    pub install_script: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct InstallOutcome {
    pub installed: Installed,
    pub message: String,
    pub count_recorded: bool,
    pub warning: Option<String>,
    /// One line to show the person: the install script changed, or the
    /// author signature changed, since the installed version.
    pub notice: Option<String>,
}

pub fn requires_channel_switch(state: &LocalState, spec: &InstallSpec) -> Result<Option<String>> {
    Ok(state
        .installed()?
        .get(&spec.app_id)
        .filter(|i| i.channel != spec.channel)
        .map(|i| i.channel.clone()))
}
pub fn requires_source_switch(
    state: &LocalState,
    spec: &InstallSpec,
    server: &str,
) -> Result<Option<String>> {
    let source = crate::auth::service_scope(server)?;
    Ok(state
        .installed()?
        .get(&spec.app_id)
        .filter(|i| i.server != source)
        .map(|i| {
            if i.server.is_empty() {
                "an unscoped legacy registry".into()
            } else {
                i.server.clone()
            }
        }))
}

pub async fn install(
    client: &Client,
    state: &LocalState,
    config: &Config,
    spec: &InstallSpec,
    allow_switch: bool,
) -> Result<InstallOutcome> {
    install_source(
        PackageSource::Registry(client),
        state,
        config,
        spec,
        allow_switch,
    )
    .await
}

pub struct LocalArchive {
    pub bytes: Vec<u8>,
    pub sha256: String,
}
enum PackageSource<'a> {
    Registry(&'a Client),
    Archive(LocalArchive),
}

/// Bootstrap or intentionally install a local archive using an independently supplied checksum.
/// Register its app/channel for the same automatic updater; no registry count is invented.
pub async fn install_local(
    state: &LocalState,
    config: &Config,
    spec: &InstallSpec,
    archive: LocalArchive,
    allow_switch: bool,
) -> Result<InstallOutcome> {
    install_source(
        PackageSource::Archive(archive),
        state,
        config,
        spec,
        allow_switch,
    )
    .await
}

async fn install_source(
    source: PackageSource<'_>,
    state: &LocalState,
    config: &Config,
    spec: &InstallSpec,
    allow_switch: bool,
) -> Result<InstallOutcome> {
    let _lock = state.lock("install")?;
    let source_server = crate::auth::service_scope(match &source {
        PackageSource::Registry(client) => client.base_url(),
        PackageSource::Archive(_) => &config.server,
    })?;
    let mut installed = state.installed()?;
    // The official CLI used `apps` through 0.1.8. Adopt that installation only
    // when its registry and command identify this same service. Other registries
    // and unrelated commands retain the normal ownership checks.
    let legacy = spec.app_id == crate::APP_ID
        && source_server == crate::DEFAULT_URL
        && !installed.contains_key(crate::APP_ID)
        && installed.get("apps").is_some_and(|old| {
            old.server == crate::DEFAULT_URL
                && matches!(old.command.as_str(), "apps" | "silicon-apps")
        });
    let previous_id = if legacy { "apps" } else { &spec.app_id };
    if let Some(old) = installed.get(previous_id) {
        ensure!(
            old.server == source_server || allow_switch,
            "{} was installed from {}; switching its registry to {} requires explicit confirmation (--yes)",
            spec.app_id,
            if old.server.is_empty() {
                "an unscoped legacy registry"
            } else {
                &old.server
            },
            source_server
        );
        ensure!(
            old.channel == spec.channel || allow_switch,
            "{} is on {}; switching to {} requires confirmation (--yes)",
            spec.app_id,
            old.channel,
            spec.channel
        );
    }
    let target = package::current_target()?;
    let client = match &source {
        PackageSource::Registry(client) => Some(*client),
        PackageSource::Archive(_) => None,
    };
    let (resolution, bytes) = match source {
        PackageSource::Registry(client) => {
            let resolution = client.resolve(spec, target).await?;
            let bytes = client.download(&resolution).await?;
            (resolution, bytes)
        }
        PackageSource::Archive(archive) => {
            ensure!(
                archive.sha256.len() == 64 && archive.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "--sha256 must be the trusted 64-character SHA-256 digest"
            );
            let digest = package::sha256(&archive.bytes);
            ensure!(
                digest == archive.sha256.to_ascii_lowercase(),
                "SHA-256 checksum mismatch; local archive was not installed"
            );
            let manifest = package::inspect_archive(&archive.bytes)?;
            let app_id = if legacy_manifest(&manifest, spec, &source_server) {
                spec.app_id.clone()
            } else {
                manifest.app_id.clone()
            };
            let resolution = Resolution {
                app_id: app_id.clone(),
                release: Release {
                    id: format!("bootstrap:{digest}"),
                    app_id,
                    channel: spec.channel.clone(),
                    version: manifest.version,
                    package_ids: vec![],
                },
                package: Package {
                    id: format!("bootstrap:{digest}"),
                    target: target.into(),
                    sha256: digest,
                    size: archive.bytes.len() as u64,
                    command: manifest.command,
                },
                download_path: String::new(),
                signature: None,
                author_signature: None,
                install_script: None,
                withdrawn: vec![],
            };
            (resolution, archive.bytes)
        }
    };
    ensure!(
        resolution.app_id == spec.app_id && resolution.release.app_id == spec.app_id,
        "server returned a release for a different app"
    );
    ensure!(
        resolution.release.channel == spec.channel,
        "server returned a different release channel"
    );
    ensure!(
        resolution.package.target == target,
        "server returned a package for the wrong target"
    );
    if let Some(version) = &spec.version {
        ensure!(
            &resolution.release.version == version,
            "server returned a different exact version"
        );
    }
    let manifest = package::inspect_archive(&bytes)?;
    ensure!(
        manifest.app_id == spec.app_id || legacy_manifest(&manifest, spec, &source_server),
        "manifest belongs to another app"
    );
    // A registry package is installed only when its signature checks out. A
    // local archive is trusted through the --sha256 its caller supplied.
    let verified = match client {
        Some(client) => Some(
            crate::signing::verify_package(client, state, &resolution, &bytes, &manifest, target)
                .await?,
        ),
        None => None,
    };
    let script_digest = match &verified {
        Some(v) => v.install_script_sha256.clone(),
        None => crate::signing::install_script_sha256(&bytes, &manifest, target)?,
    };
    let package_target = manifest
        .targets
        .get(target)
        .context("package manifest does not support this target")?;
    ensure!(
        manifest.command == resolution.package.command,
        "manifest command differs from release metadata"
    );
    let destination = state.root.join("installed").join(&spec.app_id);
    let previous_destination = state.root.join("installed").join(previous_id);
    ensure!(
        !legacy || !destination.exists(),
        "Both old and new Silicon Apps directories exist; preserve them and resolve the conflict before upgrading"
    );
    let command_path = state
        .root
        .join("bin")
        .join(command_filename(&manifest.command));
    ensure!(
        installed
            .values()
            .all(|i| i.app_id == previous_id || i.command != manifest.command),
        "command `{}` belongs to another installed app",
        manifest.command
    );
    if command_path.exists() || fs::symlink_metadata(&command_path).is_ok() {
        ensure!(
            installed
                .get(previous_id)
                .is_some_and(|i| i.command == manifest.command),
            "{} already exists and is not owned by this app; refusing to overwrite it",
            command_path.display()
        );
    }
    let staging = tempfile::tempdir_in(state.root.join("installed"))?;
    package::extract_archive(&bytes, staging.path())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            staging.path().join(&package_target.binary),
            fs::Permissions::from_mode(0o755),
        )?;
    }
    let old = installed.get(previous_id).cloned();
    let backup = state.root.join("installed").join(format!(
        ".{}-backup-{}",
        spec.app_id,
        uuid::Uuid::new_v4()
    ));
    if previous_destination.exists() {
        rename_installed(&previous_destination, &backup).await?;
    }
    if let Err(e) = fs::rename(staging.path(), &destination) {
        if backup.exists() {
            let _ = fs::rename(&backup, &previous_destination);
        }
        return Err(e.into());
    }
    let event_id = uuid::Uuid::new_v4().to_string();
    let event_path = state
        .root
        .join("install-events")
        .join(format!("{event_id}.json"));
    let event = client.map(|client| InstallEvent {
        app_id: spec.app_id.clone(),
        release_id: resolution.release.id.clone(),
        package_id: resolution.package.id.clone(),
        idempotency_key: event_id,
        server: client.base_url().to_owned(),
    });
    let transaction = async {
        link_command(&destination.join(&package_target.binary), &command_path)?;
        if let Some(script) = &package_target.install_script {
            run_script(&destination, script, config.install_script_timeout_seconds).await?;
        }
        let item = Installed {
            app_id: spec.app_id.clone(),
            channel: spec.channel.clone(),
            version: resolution.release.version.clone(),
            target: target.into(),
            command: manifest.command.clone(),
            release_id: resolution.release.id.clone(),
            package_id: resolution.package.id.clone(),
            sha256: resolution.package.sha256.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            server: source_server.clone(),
            signature_key_id: verified.as_ref().map(|v| v.key_id.clone()),
            author_key_id: verified.as_ref().and_then(|v| v.author_key_id.clone()),
            signed_by: verified.as_ref().and_then(|v| v.author.clone()),
            install_script: Some(script_digest.clone().unwrap_or_else(|| "none".into())),
        };
        if legacy {
            installed.remove("apps");
        }
        installed.insert(spec.app_id.clone(), item.clone());
        if let Some(event) = &event {
            persistence::atomic_json(&event_path, event)?;
        }
        state.save_installed(&installed)?;
        Ok::<_, anyhow::Error>(item)
    }
    .await;
    let item = match transaction {
        Ok(i) => i,
        Err(error) => {
            let _ = fs::remove_file(&event_path);
            let _ = remove_command(&command_path);
            let _ = fs::remove_dir_all(&destination);
            if backup.exists() {
                fs::rename(&backup, &previous_destination)
                    .context("rollback failed; previous files remain in backup")?;
            }
            if let Some(old) = &old {
                let old_manifest = package::validate_directory(&previous_destination)
                    .manifest
                    .context("previous manifest missing during rollback")?;
                let old_binary = &old_manifest
                    .targets
                    .get(&old.target)
                    .context("previous target missing")?
                    .binary;
                link_command(
                    &previous_destination.join(old_binary),
                    &state.root.join("bin").join(command_filename(&old.command)),
                )?;
            }
            bail!("install failed and previous version was restored: {error:#}")
        }
    };
    if backup.exists() {
        fs::remove_dir_all(&backup)?;
    }
    let notice = old.as_ref().and_then(|old| change_notice(old, &item));
    if let Some(old) = old
        && old.command != item.command
    {
        remove_command(&state.root.join("bin").join(command_filename(&old.command)))?;
    }
    let count = match (client, event.as_ref()) {
        (Some(client), Some(event)) => record_install(client, event).await,
        _ => Ok(()),
    };
    if count.is_ok() {
        let _ = fs::remove_file(&event_path);
    }
    Ok(InstallOutcome {
        message: format!(
            "Installed {} {} ({}). Run `{} --help`. Add {} to PATH if needed.",
            item.app_id,
            item.version,
            item.channel,
            item.command,
            state.root.join("bin").display()
        ),
        installed: item,
        count_recorded: client.is_some() && count.is_ok(),
        notice,
        warning: if client.is_none() {
            Some("Local archive registered for automatic channel updates; registry install count will be recorded on its first registry installation.".into())
        } else {
            count.err().map(|e| {
            format!("Installed successfully; install count is queued for an idempotent retry by the updater: {e:#}")
        })
        },
    })
}

/// One line about what changed between two installed versions that the
/// person should know: a different install script, or a different (or
/// missing) author signature.
pub fn change_notice(old: &Installed, new: &Installed) -> Option<String> {
    let short = |digest: &str| -> String {
        if digest == "none" {
            "none".into()
        } else {
            digest.chars().take(12).collect()
        }
    };
    if let (Some(before), Some(after)) = (&old.install_script, &new.install_script)
        && before != after
    {
        return Some(if after == "none" {
            format!(
                "{} {} no longer runs an install script (it was sha256 {}).",
                new.app_id,
                new.version,
                short(before)
            )
        } else {
            format!(
                "{} {} changes its install script (sha256 {} -> {}). Read it with `silicon-apps show {} --install-script`.",
                new.app_id,
                new.version,
                short(before),
                short(after),
                new.app_id
            )
        });
    }
    match (&old.author_key_id, &new.author_key_id) {
        (Some(before), None) => Some(format!(
            "{} {} is not signed by an author, though {} was (key {before}).",
            new.app_id, new.version, old.version
        )),
        (Some(before), Some(after)) if before != after => Some(format!(
            "{} {} is signed by a different author key ({before} -> {after}, {}).",
            new.app_id,
            new.version,
            new.signed_by.as_deref().unwrap_or("unknown signer")
        )),
        _ => None,
    }
}

/// Read the install script a release runs on a target, after checking the
/// package's signatures, without installing anything.
pub async fn inspect_install_script(
    client: &Client,
    state: &LocalState,
    spec: &InstallSpec,
    target: &str,
) -> Result<serde_json::Value> {
    let resolution = client.resolve(spec, target).await?;
    let bytes = client.download(&resolution).await?;
    let manifest = package::inspect_archive(&bytes)?;
    let verified =
        crate::signing::verify_package(client, state, &resolution, &bytes, &manifest, target)
            .await?;
    let script = package::install_script(&bytes, &manifest, target)?;
    let label = format!(
        "{} {} ({}, {target})",
        resolution.app_id, resolution.release.version, resolution.release.channel
    );
    let mut value = json!({
        "app_id":resolution.app_id,
        "version":resolution.release.version,
        "channel":resolution.release.channel,
        "release_id":resolution.release.id,
        "target":target,
        "signature_key_id":verified.key_id,
        "signed_by":verified.author,
        "install_script":null,
    });
    match script {
        None => value["message"] = json!(format!("{label} has no install script.")),
        Some((info, content)) => {
            let text = String::from_utf8(content).ok();
            value["install_script"] = json!({
                "path":info.path,
                "sha256":info.sha256,
                "size":info.size,
                "content":text,
                "binary":text.is_none(),
            });
            value["message"] = json!(format!(
                "{label} runs {} on install and update.\nsha256 {}\n\n{}",
                info.path,
                info.sha256,
                text.as_deref()
                    .unwrap_or("(not UTF-8 text; use --json to see its digest and size)")
            ));
        }
    }
    Ok(value)
}

// Historical first-party archives are immutable. Their checksum still comes
// from the trusted catalog or the explicit local --sha256 argument.
fn legacy_manifest(manifest: &package::Manifest, spec: &InstallSpec, server: &str) -> bool {
    spec.app_id == crate::APP_ID
        && server == crate::DEFAULT_URL
        && manifest.app_id == "apps"
        && matches!(manifest.command.as_str(), "apps" | "silicon-apps")
        && matches!(
            manifest.version.as_str(),
            "0.1.0" | "0.1.1" | "0.1.2" | "0.1.3" | "0.1.4" | "0.1.5" | "0.1.6" | "0.1.7" | "0.1.8"
        )
}
#[derive(Serialize, Deserialize)]
struct InstallEvent {
    app_id: String,
    release_id: String,
    package_id: String,
    idempotency_key: String,
    server: String,
}
async fn record_install(client: &Client, event: &InstallEvent) -> Result<()> {
    client
        .action(
            "POST",
            &event.app_id,
            &["installs"],
            Some(json!({"release_id":event.release_id,"package_id":event.package_id})),
            Some(&event.idempotency_key),
        )
        .await?;
    Ok(())
}

pub(crate) async fn flush_install_events(client: &Client, state: &LocalState) -> Result<()> {
    let _lock = state.lock("install-events")?;
    let path = state.root.join("install-events");
    if !path.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let event: InstallEvent = serde_json::from_slice(&fs::read(entry.path())?)?;
        if event.server != client.base_url() {
            continue;
        }
        if record_install(client, &event).await.is_ok() {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

async fn rename_installed(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        for attempt in 0..30 {
            match fs::rename(source, destination) {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && attempt < 29 => {
                    tokio::time::sleep(Duration::from_millis(100)).await
                }
                Err(e) => return Err(e.into()),
            }
        }
        unreachable!()
    }
    #[cfg(not(windows))]
    {
        fs::rename(source, destination)?;
        Ok(())
    }
}

pub fn uninstall(state: &LocalState, id: &str) -> Result<String> {
    ensure!(package::valid_existing_app_id(id), "invalid app_id `{id}`");
    let _lock = state.lock("install")?;
    let mut items = state.installed()?;
    let item = items
        .remove(id)
        .with_context(|| format!("{id} is not installed"))?;
    let destination = state.root.join("installed").join(id);
    let trash = state
        .root
        .join("installed")
        .join(format!(".uninstall-{id}-{}", uuid::Uuid::new_v4()));
    if destination.exists() {
        #[cfg(windows)]
        {
            let mut result = fs::rename(&destination, &trash);
            for _ in 0..30 {
                if !result
                    .as_ref()
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied)
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
                result = fs::rename(&destination, &trash);
            }
            result?;
        }
        #[cfg(not(windows))]
        fs::rename(&destination, &trash)?;
    }
    if let Err(error) = state.save_installed(&items) {
        if trash.exists() {
            let _ = fs::rename(&trash, &destination);
        }
        return Err(error);
    }
    remove_command(&state.root.join("bin").join(command_filename(&item.command)))?;
    if trash.exists() {
        fs::remove_dir_all(trash)?;
    }
    Ok(format!(
        "Uninstalled {id}. You can leave a review with `apps review {id} --rating 5`."
    ))
}
fn command_filename(command: &str) -> String {
    if cfg!(windows) {
        format!("{command}.cmd")
    } else {
        command.to_owned()
    }
}
fn remove_command(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
fn link_command(binary: &Path, command: &Path) -> Result<()> {
    let temp = command.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    #[cfg(unix)]
    std::os::unix::fs::symlink(binary, &temp)?;
    #[cfg(windows)]
    {
        ensure!(
            !binary.to_string_lossy().contains(['\r', '\n', '"', '%']),
            "binary path cannot be represented safely in a Windows command shim"
        );
        fs::write(
            &temp,
            format!("@echo off\r\n\"{}\" %*\r\n", binary.display()),
        )?;
    }
    #[cfg(not(any(unix, windows)))]
    fs::copy(binary, &temp)?;
    #[cfg(windows)]
    remove_command(command)?;
    fs::rename(temp, command)?;
    Ok(())
}
async fn run_script(root: &Path, script: &str, timeout: u64) -> Result<()> {
    use std::process::Stdio;
    let path = root.join(script);
    let output = tempfile::tempfile()?;
    #[cfg(unix)]
    let mut command = {
        let mut c = tokio::process::Command::new("/bin/sh");
        c.arg(&path).process_group(0);
        c
    };
    #[cfg(windows)]
    let mut command = {
        // cmd.exe cannot execute the extended-length absolute paths produced by
        // canonicalize. Run the manifest path relative to the install directory.
        // Expand it once inside quotes, with delayed expansion disabled.
        let mut c = tokio::process::Command::new("cmd.exe");
        c.env(
            "APPS_INSTALL_SCRIPT",
            format!(".\\{}", script.replace('/', "\\")),
        )
        .args(["/D", "/V:OFF", "/S", "/C"])
        .raw_arg(r#"""%APPS_INSTALL_SCRIPT%"""#);
        c
    };
    command
        .current_dir(root)
        .env("APPS_INSTALL_DIR", root)
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(output.try_clone()?)
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .with_context(|| format!("cannot start install script {}", path.display()))?;
    let status =
        tokio::time::timeout(Duration::from_secs(timeout.clamp(1, 3600)), child.wait()).await;
    match status {
        Ok(status) => {
            let status = status?;
            ensure!(
                status.success(),
                "install script `{script}` failed with {status}; previous version will be restored\n{}",
                script_output_tail(&output)?
            );
        }
        Err(_) => {
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                let _ = tokio::process::Command::new("/bin/kill")
                    .args(["-KILL", "--", &format!("-{pid}")])
                    .status()
                    .await;
            }
            #[cfg(windows)]
            if let Some(pid) = child.id() {
                let _ = tokio::process::Command::new("taskkill.exe")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .status()
                    .await;
            }
            let _ = child.kill().await;
            bail!(
                "install script `{script}` exceeded {timeout} seconds; it was terminated and previous version will be restored\n{}",
                script_output_tail(&output)?
            );
        }
    }
    Ok(())
}

fn script_output_tail(file: &fs::File) -> Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = file.try_clone()?;
    let length = file.metadata()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(64 * 1024)))?;
    let mut bytes = Vec::new();
    file.take(64 * 1024).read_to_end(&mut bytes)?;
    if bytes.is_empty() {
        Ok("The script produced no output.".into())
    } else {
        Ok(format!(
            "Script output (last 64 KiB):\n{}",
            String::from_utf8_lossy(&bytes)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_channels_and_exact_versions() {
        for (input, channel, version) in [
            ("ring", "production", None),
            ("ring>dev", "development", None),
            ("ring@1.2.3", "production", Some("1.2.3")),
            ("ring>dev@1.2.3", "development", Some("1.2.3")),
        ] {
            let s: InstallSpec = input.parse().unwrap();
            assert_eq!(s.channel, channel);
            assert_eq!(s.version.as_deref(), version);
            assert_eq!(s.to_string(), input);
        }
        for bad in [
            "../oops",
            "Ring",
            "ring>prod",
            "ring@latest",
            "ring@1.2.3@4",
            "ring@1.2.3-beta",
        ] {
            assert!(bad.parse::<InstallSpec>().is_err(), "{bad}");
        }
    }
    #[test]
    fn home_must_be_a_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("file");
        fs::write(&file, "x").unwrap();
        assert!(
            LocalState::new(file)
                .unwrap_err()
                .to_string()
                .contains("not a directory")
        );
    }
}
