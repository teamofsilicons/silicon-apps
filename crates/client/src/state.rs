//! Explicit local persistence adapters. The HTTP client remains stateless.
use crate::install::Installed;
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// cmd.exe cannot launch a canonical Windows verbatim path. Keep filesystem
/// paths canonical, and convert only where a path is embedded in a shell command.
#[cfg(any(windows, test))]
pub(crate) fn windows_shell_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: String,
    pub accounts_url: String,
    pub telemetry: bool,
    pub install_script_timeout_seconds: u64,
    pub update_interval_seconds: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            server: crate::DEFAULT_URL.into(),
            accounts_url: "https://accounts.teamofsilicons.com".into(),
            telemetry: true,
            install_script_timeout_seconds: 120,
            update_interval_seconds: 60,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LocalState {
    pub home: PathBuf,
    pub root: PathBuf,
}
/// Releases the advisory lock even if a concurrently spawned child temporarily
/// retains a duplicate descriptor before exec closes it.
#[derive(Debug)]
pub struct LocalLock(fs::File);

impl Drop for LocalLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

impl LocalState {
    pub fn new(home: impl Into<PathBuf>) -> Result<Self> {
        let home = home.into();
        ensure!(home.is_dir(), "{}: not a directory", home.display());
        let home = home.canonicalize()?;
        let root = home.join(".apps");
        if root.exists() {
            ensure!(
                !fs::symlink_metadata(&root)?.file_type().is_symlink(),
                "{}: state directory must not be a symlink",
                root.display()
            );
        }
        Ok(Self { home, root })
    }
    /// Resolve explicit home, SILICON_HOME, saved `config home`, then normal home.
    pub fn discover(explicit: Option<&Path>) -> Result<Self> {
        if let Some(home) = explicit {
            return Self::new(home);
        }
        if let Some(home) = std::env::var_os("SILICON_HOME") {
            return Self::new(PathBuf::from(home));
        }
        let default = dirs::home_dir()
            .context("could not locate home; set SILICON_HOME to an existing directory")?;
        let pointer = default.join(".apps/home");
        if pointer.is_file() {
            return Self::new(PathBuf::from(fs::read_to_string(pointer)?.trim()));
        }
        Self::new(default)
    }
    pub fn set_home(location: &Path) -> Result<Self> {
        let state = Self::new(location)?;
        let default = dirs::home_dir().context("could not locate default home")?;
        let root = default.join(".apps");
        fs::create_dir_all(&root)?;
        atomic_write(&root.join("home"), state.home.to_string_lossy().as_bytes())?;
        Ok(state)
    }
    pub fn initialize(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        for p in ["bin", "installed", "locks"] {
            fs::create_dir_all(self.root.join(p))?;
        }
        Ok(())
    }
    pub fn config(&self) -> Result<Config> {
        read_json(&self.root.join("config.json"))
    }
    pub fn save_config(&self, config: &Config) -> Result<()> {
        self.initialize()?;
        atomic_json(&self.root.join("config.json"), config)
    }
    pub fn installed(&self) -> Result<BTreeMap<String, Installed>> {
        read_json(&self.root.join("installed.json"))
    }
    pub fn save_installed(&self, items: &BTreeMap<String, Installed>) -> Result<()> {
        self.initialize()?;
        atomic_json(&self.root.join("installed.json"), items)
    }
    pub fn lock(&self, name: &str) -> Result<LocalLock> {
        ensure!(
            name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "invalid lock name"
        );
        self.initialize()?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join("locks").join(name))?;
        lock.try_lock_exclusive().with_context(|| {
            format!("another Apps process holds the {name} lock; wait for it to finish")
        })?;
        Ok(LocalLock(lock))
    }
}
pub fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match fs::read(path) {
        Ok(b) => serde_json::from_slice(&b).with_context(|| {
            format!(
                "{} contains invalid JSON; preserve it and repair the reported syntax",
                path.display()
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e.into()),
    }
}
pub fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    atomic_write(path, &serde_json::to_vec_pretty(value)?)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().context("state path must have a parent")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn dropping_lock_releases_it_with_a_duplicate_descriptor_alive() {
        let home = tempfile::tempdir().unwrap();
        let state = LocalState::new(home.path()).unwrap();
        let guard = state.lock("install").unwrap();
        // A fork in another thread can retain the open-file description until
        // the child executes its new program, even with close-on-exec set.
        let duplicate = guard.0.try_clone().unwrap();
        assert!(state.lock("install").is_err());
        drop(guard);
        let next = state.lock("install").unwrap();
        assert!(state.lock("install").is_err());
        drop(duplicate);
        assert!(state.lock("install").is_err());
        drop(next);
        assert!(state.lock("install").is_ok());
    }
}
