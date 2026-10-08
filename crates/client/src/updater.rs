//! The sole updater: one locked process checks every installed channel once per minute.
use crate::{
    Client, Config, LocalState, auth,
    install::{self, InstallSpec},
    state,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Stdio, time::Duration};

pub async fn update(
    client: &Client,
    state: &LocalState,
    config: &Config,
    app: Option<&str>,
    allow_script: bool,
) -> Result<Value> {
    install::flush_install_events(client, state).await?;
    let mut results = vec![];
    let items = state.installed()?;
    if let Some(app) = app {
        ensure!(items.contains_key(app), "{app} is not installed");
    }
    for item in items
        .values()
        .filter(|i| app.is_none_or(|app| app == i.app_id))
    {
        let spec = InstallSpec {
            app_id: item.app_id.clone(),
            channel: item.channel.clone(),
            version: None,
        };
        let result=async {
            ensure!(!item.server.is_empty(),"{} has an unscoped legacy installation. Explicitly reinstall from the intended server with --yes before automatic updates can resume.",item.app_id);
            ensure!(item.server==auth::service_scope(client.base_url())?,"{} is installed from {}; selected registry {} cannot update it. Use --server {} update {} or explicitly reinstall from another registry with --yes.",item.app_id,item.server,client.base_url(),item.server,item.app_id);
            let latest=client.resolve(&spec,&item.target).await?;
            if latest.release.id==item.release_id {return Ok(json!({"app_id":item.app_id,"status":"current","version":item.version}));}
            if let Some(receipt)=defer_self_install(state,config,&spec,false,allow_script||item.allow_install_script)?{return Ok(receipt);}
            let outcome=install::install(client,state,config,&spec,false,allow_script||item.allow_install_script).await?;
            Ok::<_,anyhow::Error>(json!({"app_id":item.app_id,"status":"updated","version":outcome.installed.version,"warning":outcome.warning}))
        }.await;
        results.push(match result {
            Ok(value) => value,
            Err(error) => {
                json!({"app_id":item.app_id,"status":"failed","error":format!("{error:#}")})
            }
        });
    }
    Ok(json!({"items":results}))
}
pub fn status(state: &LocalState) -> Result<Value> {
    let running = if state.root.join("locks/daemon").exists() {
        state.lock("daemon").is_err()
    } else {
        false
    };
    let last: Value = state::read_json(&state.root.join("updater.json"))?;
    Ok(
        json!({"running":running,"interval_seconds":state.config()?.update_interval_seconds,"last_run":last}),
    )
}
pub async fn run(state: &LocalState, once: bool) -> Result<Value> {
    run_with_overrides(state, once, None, None).await
}
pub async fn run_with_overrides(
    state: &LocalState,
    once: bool,
    server: Option<&str>,
    accounts_url: Option<&str>,
) -> Result<Value> {
    let _lock = state.lock("daemon")?;
    let stop = state.root.join("daemon-stop");
    if stop.exists() {
        fs::remove_file(&stop)?;
    }
    state::atomic_json(
        &state.root.join("daemon.json"),
        &json!({"pid":std::process::id(),"started_at":chrono::Utc::now().to_rfc3339()}),
    )?;
    loop {
        let mut config = state.config()?;
        if let Some(url) = server {
            config.server = url.to_owned();
        }
        if let Some(url) = accounts_url {
            config.accounts_url = url.to_owned();
        }
        let result = match auth::authenticated_client(
            state,
            &config,
            std::env::var("APPS_TOKEN").ok(),
        )
        .await
        {
            Ok(client) => update(&client, state, &config, None, false).await,
            Err(error) => Err(error),
        };
        let value = match result {
            Ok(value) => json!({"at":chrono::Utc::now().to_rfc3339(),"result":value}),
            Err(error) => {
                json!({"at":chrono::Utc::now().to_rfc3339(),"error":format!("{error:#}")})
            }
        };
        state::atomic_json(&state.root.join("updater.json"), &value)?;
        if !once
            && value["result"]["items"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["app_id"] == "apps" && item["status"] == "updated")
            })
        {
            // A successful self-update must replace the updater's own running code too.
            drop(_lock);
            let executable = preferred_executable(state, &std::env::current_exe()?)?;
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                // Preserve the process ID and supervisor relationship on launchd/systemd.
                let error = std::process::Command::new(executable)
                    .arg("--home")
                    .arg(&state.home)
                    .arg("--server")
                    .arg(&config.server)
                    .arg("--accounts-url")
                    .arg(&config.accounts_url)
                    .args(["daemon", "run"])
                    .exec();
                return Err(error).context("Apps updated, but its updater could not reload the new executable; restart it with apps daemon start");
            }
            #[cfg(not(unix))]
            return start_process_configured(state, &executable, &config).await;
        }
        if once {
            return Ok(value);
        }
        let delay = tokio::time::sleep(Duration::from_secs(config.update_interval_seconds.max(10)));
        tokio::pin!(delay);
        loop {
            tokio::select! { _=&mut delay=>break, _=tokio::time::sleep(Duration::from_secs(1))=>{if stop.exists(){fs::remove_file(&stop)?;return Ok(json!({"status":"stopped"}));}}, _=tokio::signal::ctrl_c()=>return Ok(json!({"status":"stopped"})) }
        }
    }
}
pub async fn start(state: &LocalState, executable: &Path) -> Result<Value> {
    start_configured(state, executable, &state.config()?).await
}
pub async fn start_configured(
    state: &LocalState,
    executable: &Path,
    config: &Config,
) -> Result<Value> {
    state.initialize()?;
    state.save_config(config)?;
    if status(state)?["running"] == true {
        return Ok(json!({"status":"already_running"}));
    }
    if state.root.join("service.json").is_file() {
        activate_registered_service()?;
        return Ok(
            json!({"status":"starting","message":"The registered updater service is starting with its saved configuration."}),
        );
    }
    start_process_configured(state, executable, config).await
}

/// Start a detached process directly. Used by startup launchers without recursively starting
/// their own registered service; Windows copies the latest installed executable on every run.
pub async fn start_process_configured(
    state: &LocalState,
    executable: &Path,
    config: &Config,
) -> Result<Value> {
    state.initialize()?;
    state.save_config(config)?;
    if status(state)?["running"] == true {
        return Ok(json!({"status":"already_running"}));
    }
    let executable = runtime_executable(state, &preferred_executable(state, executable)?)?;
    let log = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(state.root.join("updater.log"))?;
    let mut command = std::process::Command::new(&executable);
    command
        .arg("--home")
        .arg(&state.home)
        .args(["daemon", "run"])
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    detach_process(&mut command);
    command
        .spawn()
        .context("could not start Apps updater process")?;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if status(state)?["running"] == true {
            return Ok(json!({"status":"started"}));
        }
    }
    anyhow::bail!(
        "updater did not start; inspect {}",
        state.root.join("updater.log").display()
    )
}
pub async fn stop(state: &LocalState) -> Result<Value> {
    if status(state)?["running"] != true {
        return Ok(json!({"status":"not_running"}));
    }
    #[cfg(target_os = "macos")]
    if state.root.join("service.json").is_file() {
        ensure!(
            std::process::Command::new("launchctl")
                .arg("disable")
                .arg(launchd_label()?)
                .status()?
                .success(),
            "could not pause automatic launchd restarts; updater has not been stopped"
        );
    }
    state::atomic_write(&state.root.join("daemon-stop"), b"stop")?;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if status(state)?["running"] != true {
            return Ok(json!({"status":"stopped"}));
        }
    }
    Ok(
        json!({"status":"stop_requested","message":"Updater will stop after its current install finishes."}),
    )
}
/// Write a reviewed native service definition. Activating it is explicit CLI behavior.
pub fn service_definition(
    state: &LocalState,
    executable: &Path,
) -> Result<(std::path::PathBuf, String)> {
    #[cfg(target_os = "macos")]
    {
        let escaped = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
        };
        let path = dirs::home_dir()
            .context("home not found")?
            .join("Library/LaunchAgents/com.teamofsilicons.apps.plist");
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>com.teamofsilicons.apps</string><key>ProgramArguments</key><array><string>{}</string><string>--home</string><string>{}</string><string>daemon</string><string>run</string></array><key>RunAtLoad</key><true/><key>KeepAlive</key><true/><key>StandardOutPath</key><string>{}</string><key>StandardErrorPath</key><string>{}</string></dict></plist>\n",
            escaped(&executable.to_string_lossy()),
            escaped(&state.home.to_string_lossy()),
            escaped(&state.root.join("updater.log").to_string_lossy()),
            escaped(&state.root.join("updater.log").to_string_lossy())
        );
        Ok((path, body))
    }
    #[cfg(target_os = "linux")]
    {
        let quote = |p: &Path| -> Result<String> {
            let p = p.to_string_lossy();
            ensure!(
                !p.contains(['\r', '\n']),
                "service paths cannot contain line breaks"
            );
            Ok(format!(
                "\"{}\"",
                p.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('%', "%%")
            ))
        };
        let path = dirs::home_dir()
            .context("home not found")?
            .join(".config/systemd/user/silicon-apps.service");
        Ok((
            path,
            format!(
                "[Unit]\nDescription=Silicon Apps automatic updater\nAfter=network-online.target\n\n[Service]\nType=simple\nExecStart={} --home {} daemon run\nRestart=on-failure\nRestartSec=10\n\n[Install]\nWantedBy=default.target\n",
                quote(executable)?,
                quote(&state.home)?
            ),
        ))
    }
    #[cfg(windows)]
    {
        Ok((
            state.root.join("updater-task.xml"),
            windows_service_document(state, executable),
        ))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = (state, executable);
        anyhow::bail!(
            "This platform has no supported service manager; run `apps daemon run` under your session manager."
        )
    }
}
pub fn install_service(state: &LocalState, executable: &Path) -> Result<Value> {
    state.initialize()?;
    let executable = preferred_executable(state, executable)?;
    let (path, body) = service_definition(state, &executable)?;
    #[cfg(not(windows))]
    state::atomic_write(&path, body.as_bytes())?;
    #[cfg(windows)]
    {
        let mut bytes = vec![0xff, 0xfe];
        for unit in body.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        state::atomic_write(&path, &bytes)?;
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("id").arg("-u").output()?;
        let uid = String::from_utf8(output.stdout)?.trim().to_owned();
        let _ = std::process::Command::new("launchctl")
            .arg("enable")
            .arg(format!("gui/{uid}/com.teamofsilicons.apps"))
            .status();
        let status = std::process::Command::new("launchctl")
            .arg("bootstrap")
            .arg(format!("gui/{uid}"))
            .arg(&path)
            .status()?;
        ensure!(
            status.success(),
            "launchctl could not load {}; inspect it and run launchctl bootstrap manually",
            path.display()
        );
    }
    #[cfg(target_os = "linux")]
    {
        ensure!(
            std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .status()?
                .success(),
            "systemd user daemon-reload failed"
        );
        ensure!(
            std::process::Command::new("systemctl")
                .args(["--user", "enable", "--now", "silicon-apps.service"])
                .status()?
                .success(),
            "systemd user service activation failed"
        );
    }
    #[cfg(windows)]
    {
        ensure!(
            std::process::Command::new("schtasks.exe")
                .args(["/Create", "/TN", "SiliconAppsUpdater", "/XML"])
                .arg(&path)
                .arg("/F")
                .status()?
                .success(),
            "Windows could not register the Apps updater task"
        );
        ensure!(
            std::process::Command::new("schtasks.exe")
                .args(["/Run", "/TN", "SiliconAppsUpdater"])
                .status()?
                .success(),
            "Windows could not start the Apps updater task"
        );
    }
    state::atomic_json(&state.root.join("service.json"), &json!({"path":path}))?;
    Ok(json!({"status":"installed","service_file":path}))
}

#[cfg(any(windows, test))]
fn windows_service_document(state: &LocalState, executable: &Path) -> String {
    let escaped = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let shim = state.root.join("bin/apps.cmd");
    let launcher = if shim.is_file() {
        shim
    } else {
        executable.to_owned()
    };
    let shell = std::path::PathBuf::from(
        std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()),
    )
    .join("System32/cmd.exe");
    let arguments = format!(
        "/D /S /C \"\"{}\" --home \"{}\" daemon run --detached\"",
        launcher.display(),
        state.home.display()
    );
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?><Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\"><Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers><Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><StartWhenAvailable>true</StartWhenAvailable></Settings><Actions><Exec><Command>{}</Command><Arguments>{}</Arguments></Exec></Actions></Task>",
        escaped(&shell.to_string_lossy()),
        escaped(&arguments)
    )
}

#[cfg(target_os = "macos")]
fn launchd_label() -> Result<String> {
    let output = std::process::Command::new("id").arg("-u").output()?;
    ensure!(
        output.status.success(),
        "could not determine current user for launchd"
    );
    Ok(format!(
        "gui/{}/com.teamofsilicons.apps",
        String::from_utf8(output.stdout)?.trim()
    ))
}

fn activate_registered_service() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let label = launchd_label()?;
        ensure!(
            std::process::Command::new("launchctl")
                .arg("enable")
                .arg(&label)
                .status()?
                .success(),
            "launchctl enable failed"
        );
        ensure!(
            std::process::Command::new("launchctl")
                .arg("kickstart")
                .arg(&label)
                .status()?
                .success(),
            "launchctl kickstart failed; run apps daemon remove then install to repair its definition"
        );
    }
    #[cfg(target_os = "linux")]
    ensure!(
        std::process::Command::new("systemctl")
            .args(["--user", "start", "silicon-apps.service"])
            .status()?
            .success(),
        "could not start Apps systemd user service"
    );
    #[cfg(windows)]
    ensure!(
        std::process::Command::new("schtasks.exe")
            .args(["/Run", "/TN", "SiliconAppsUpdater"])
            .status()?
            .success(),
        "could not start Apps scheduled task"
    );
    Ok(())
}

pub async fn remove_service(state: &LocalState) -> Result<Value> {
    let receipt = stop(state).await?;
    ensure!(
        status(state)?["running"] != true,
        "updater is finishing its current install. Stop is requested; retry daemon remove after it finishes: {receipt}"
    );
    let marker: Value = state::read_json(&state.root.join("service.json"))?;
    if marker.is_null() {
        return Ok(json!({"status":"not_installed"}));
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("launchctl")
            .arg("bootout")
            .arg(launchd_label()?)
            .status()?;
    }
    #[cfg(target_os = "linux")]
    ensure!(
        std::process::Command::new("systemctl")
            .args(["--user", "disable", "--now", "silicon-apps.service"])
            .status()?
            .success(),
        "could not disable Apps service"
    );
    #[cfg(windows)]
    ensure!(
        std::process::Command::new("schtasks.exe")
            .args(["/Delete", "/TN", "SiliconAppsUpdater", "/F"])
            .status()?
            .success(),
        "could not remove Apps scheduled task"
    );
    if let Some(path) = marker["path"].as_str()
        && Path::new(path).exists()
    {
        fs::remove_file(path)?;
    }
    fs::remove_file(state.root.join("service.json"))?;
    Ok(json!({"status":"removed"}))
}

/// Windows does not permit replacing a running executable. The updater runs from its own
/// immutable copy so it can replace the user-facing `apps` package like any other package.
pub fn runtime_executable(state: &LocalState, executable: &Path) -> Result<std::path::PathBuf> {
    #[cfg(windows)]
    {
        let directory = state
            .root
            .join("runtime")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&directory)?;
        let copy = directory.join("apps.exe");
        fs::copy(executable, &copy)?;
        Ok(copy)
    }
    #[cfg(not(windows))]
    {
        let _ = state;
        Ok(executable.to_owned())
    }
}

fn preferred_executable(state: &LocalState, fallback: &Path) -> Result<std::path::PathBuf> {
    let root = state.root.join("installed/apps");
    if root.is_dir()
        && let Some(manifest) = crate::package::validate_directory(&root).manifest
        && let Some(target) = manifest.targets.get(crate::package::current_target()?)
    {
        let executable = root.join(&target.binary);
        if executable.is_file() {
            return Ok(executable);
        }
    }
    Ok(fallback.to_owned())
}

/// Return a detached helper receipt only when Windows currently has this app's binary open.
/// The normal public install command runs from a separate copy, with the same explicit options.
pub fn defer_self_install(
    state: &LocalState,
    config: &Config,
    spec: &InstallSpec,
    allow_switch: bool,
    allow_script: bool,
) -> Result<Option<Value>> {
    defer_self_install_archive(state, config, spec, allow_switch, allow_script, None)
}
pub fn defer_self_install_archive(
    state: &LocalState,
    config: &Config,
    spec: &InstallSpec,
    allow_switch: bool,
    allow_script: bool,
    archive: Option<(&Path, &str)>,
) -> Result<Option<Value>> {
    #[cfg(windows)]
    {
        let executable = std::env::current_exe()?;
        if spec.app_id != "apps" || !executable.starts_with(state.root.join("installed/apps")) {
            return Ok(None);
        }
        state.initialize()?;
        let helper = runtime_executable(state, &executable)?;
        let log = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(state.root.join("self-update.log"))?;
        let mut command = std::process::Command::new(helper);
        command
            .arg("--home")
            .arg(&state.home)
            .arg("--server")
            .arg(&config.server)
            .arg("--accounts-url")
            .arg(&config.accounts_url)
            .arg("install")
            .arg(spec.to_string());
        if allow_switch {
            command.arg("--yes");
        }
        if allow_script {
            command.arg("--allow-install-script");
        }
        if let Some((path, digest)) = archive {
            command
                .arg("--archive")
                .arg(path)
                .arg("--sha256")
                .arg(digest);
        }
        command
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        detach_process(&mut command);
        command.spawn()?;
        Ok(Some(
            json!({"status":"scheduled","message":"Apps self-update is continuing in a separate helper after this process exits. Inspect `apps installed` and the self-update log for its result.","log":state.root.join("self-update.log")}),
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = (state, config, spec, allow_switch, allow_script, archive);
        Ok(None)
    }
}

fn detach_process(command: &mut std::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000208);
    }
}

/// Windows needs a separate process to remove the executable currently running the CLI.
pub fn defer_self_uninstall(state: &LocalState, config: &Config) -> Result<Option<Value>> {
    #[cfg(windows)]
    {
        let executable = std::env::current_exe()?;
        if !executable.starts_with(state.root.join("installed/apps")) {
            return Ok(None);
        }
        let helper = runtime_executable(state, &executable)?;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(state.root.join("self-update.log"))?;
        let mut command = std::process::Command::new(helper);
        command
            .arg("--home")
            .arg(&state.home)
            .arg("--server")
            .arg(&config.server)
            .arg("--accounts-url")
            .arg(&config.accounts_url)
            .args(["uninstall", "apps"])
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        detach_process(&mut command);
        command.spawn()?;
        Ok(Some(
            json!({"status":"scheduled","message":"Apps is being removed by a helper after this process exits. Its updater service is stopped and removed first.","log":state.root.join("self-update.log")}),
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = (state, config);
        Ok(None)
    }
}

pub fn telemetry(
    state: &LocalState,
    config: &Config,
    event: &str,
    step: &str,
    progress: f64,
    context: Value,
) {
    if !config.telemetry {
        return;
    }
    let Ok(key) =
        std::env::var("APPS_TELEMETRY_TABLE_KEY").or_else(|_| std::env::var("APPS_TELEMETRY_KEY"))
    else {
        return;
    };
    // Accounts enables aws-lc while Space Station enables ring. Its WebSocket
    // transport requires a process provider when both are linked. Retain the
    // provider already selected by an embedding application.
    let _ = rustls::crypto::ring::default_provider().install_default();
    if let Ok(client) = space_station::SpaceClient::builder(&key)
        .home(telemetry_home(&state.root))
        .url(
            std::env::var("SPACE_STATION_URL")
                .unwrap_or_else(|_| space_station::DEFAULT_URL.into()),
        )
        .flush_timeout(Duration::from_millis(100))
        .on_error(|_| {})
        .build()
    {
        client.record(json!({"source":"silicon-apps-cli","version":crate::VERSION,"step":step,"progress":progress,"event":event,"context":context}));
        let _ = client.flush();
    }
}

fn telemetry_home(root: &Path) -> std::path::PathBuf {
    let home = root.join("telemetry");
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        // AF_UNIX expects a Win32 path, while canonicalize returns a verbatim
        // disk path. Both spellings address the same local storage directory.
        let mut components = home.components();
        if let Some(Component::Prefix(prefix)) = components.next()
            && let Prefix::VerbatimDisk(drive) = prefix.kind()
        {
            let mut normalized = std::path::PathBuf::from(format!("{}:\\", drive as char));
            normalized.extend(components.filter(|part| !matches!(part, Component::RootDir)));
            return normalized;
        }
    }
    home
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn telemetry_uses_win32_disk_path_without_changing_other_prefixes() {
        assert_eq!(
            telemetry_home(Path::new(r"\\?\C:\Users\Alice\.apps")),
            std::path::PathBuf::from(r"C:\Users\Alice\.apps\telemetry")
        );
        for root in [
            r"C:\Users\Alice\.apps",
            r"\\server\share\apps",
            r"\\?\UNC\server\share\apps",
        ] {
            assert_eq!(
                telemetry_home(Path::new(root)),
                Path::new(root).join("telemetry")
            );
        }
    }
    #[test]
    fn windows_startup_follows_current_command_instead_of_frozen_runtime() {
        let home = tempfile::tempdir().unwrap();
        let state = LocalState::new(home.path()).unwrap();
        state.initialize().unwrap();
        fs::write(state.root.join("bin/apps.cmd"), "current command").unwrap();
        let body = windows_service_document(&state, Path::new("runtime/old/apps.exe"));
        assert!(body.contains("bin/apps.cmd") || body.contains(r"bin\apps.cmd"));
        assert!(body.contains("daemon run --detached"));
        assert!(!body.contains("runtime/old"));
    }
}
