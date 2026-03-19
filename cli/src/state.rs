use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// In-memory representation of ~/.epc/services.toml
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ServicesFile {
    #[serde(skip)]
    path: PathBuf,
    #[serde(default)]
    pub services: HashMap<String, ServiceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// Absolute path to the package directory
    pub dir: String,
    pub port: u16,
    pub pid: u32,
    /// ISO 8601 timestamp
    pub started: String,
    /// Absolute path to the log file
    pub log_file: String,
}

impl ServicesFile {
    pub fn default_path() -> Result<PathBuf> {
        Ok(dirs::home_dir()
            .context("could not determine home directory")?
            .join(".epc")
            .join("services.toml"))
    }

    /// Load from the default location (~/.epc/services.toml).
    pub fn load() -> Result<Self> {
        Self::load_from(&Self::default_path()?)
    }

    /// Load from an explicit path (useful for tests).
    pub fn load_from(path: &Path) -> Result<Self> {
        let mut sf = if path.exists() {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let mut parsed: Self = toml::from_str(&raw)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            parsed.path = path.to_path_buf();
            parsed
        } else {
            let mut sf = Self::default();
            sf.path = path.to_path_buf();
            sf
        };
        sf.path = path.to_path_buf();
        Ok(sf)
    }

    /// Save to the path this file was loaded from (or the default path if new).
    pub fn save(&self) -> Result<()> {
        let path = if self.path == PathBuf::default() {
            Self::default_path()?
        } else {
            self.path.clone()
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let raw = toml::to_string_pretty(self).context("failed to serialize services.toml")?;
        std::fs::write(&path, raw)
            .with_context(|| format!("failed to write {}", path.display()))
    }

    pub fn insert(&mut self, name: String, entry: ServiceEntry) {
        self.services.insert(name, entry);
    }

    pub fn remove(&mut self, name: &str) -> Option<ServiceEntry> {
        self.services.remove(name)
    }

    /// Returns true if any process is listening on `port` (any interface).
    ///
    /// This is the canonical liveness check — a PID stored at deploy time can't
    /// stay in sync if the process crashes, restarts, or is managed externally.
    /// The port is the real source of truth: if it answers, the service is up.
    ///
    /// Uses `lsof` rather than a TCP connect so it works regardless of which
    /// interface the service bound to (127.0.0.1, Tailscale IP, 0.0.0.0, etc).
    pub fn is_port_listening(port: u16) -> bool {
        std::process::Command::new("lsof")
            .args(["-i", &format!(":{port}"), "-sTCP:LISTEN"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Starting from `preferred`, return the first port that is not listening.
    /// Scans up to 100 ports above the preferred value before giving up.
    pub fn find_available_port(preferred: u16) -> Option<u16> {
        (preferred..preferred.saturating_add(100))
            .find(|&p| !Self::is_port_listening(p))
    }

    /// Returns all PIDs currently listening on `port` (any interface).
    ///
    /// Use this for kill operations instead of the cached PID in services.toml —
    /// the cached PID goes stale the moment anything touches the process outside EPC.
    pub fn pids_on_port(port: u16) -> Vec<u32> {
        let Ok(output) = std::process::Command::new("lsof")
            .args(["-t", "-i", &format!(":{port}"), "-sTCP:LISTEN"])
            .output() else { return vec![] };
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    }

    /// Returns true if the PID is still alive (`kill -0 <pid>` semantics).
    ///
    /// Only use this when you need to *signal* the process (e.g. `epc stop`).
    /// For display / health checks, prefer `is_port_listening(port)`.
    pub fn is_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn state_path(dir: &TempDir) -> PathBuf {
        dir.path().join("services.toml")
    }

    fn sample_entry() -> ServiceEntry {
        ServiceEntry {
            dir: "/tmp/my_pkg".to_string(),
            port: 9000,
            pid: 99999,
            started: "2026-02-28T00:00:00Z".to_string(),
            log_file: "/tmp/.epc/logs/my_pkg.log".to_string(),
        }
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let sf = ServicesFile::load_from(&state_path(&dir)).unwrap();
        assert!(sf.services.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = state_path(&dir);

        let mut sf = ServicesFile::load_from(&path).unwrap();
        sf.insert("my_pkg".to_string(), sample_entry());
        sf.save().unwrap();

        let loaded = ServicesFile::load_from(&path).unwrap();
        let entry = loaded.services.get("my_pkg").unwrap();
        assert_eq!(entry.port, 9000);
        assert_eq!(entry.pid, 99999);
        assert_eq!(entry.dir, "/tmp/my_pkg");
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a").join("b").join("services.toml");
        let sf = ServicesFile::load_from(&path).unwrap();
        sf.save().unwrap();
        assert!(path.exists());
    }

    #[test]
    fn insert_then_remove() {
        let dir = TempDir::new().unwrap();
        let path = state_path(&dir);
        let mut sf = ServicesFile::load_from(&path).unwrap();
        sf.insert("pkg".to_string(), sample_entry());
        assert!(sf.services.contains_key("pkg"));
        let removed = sf.remove("pkg");
        assert!(removed.is_some());
        assert!(sf.services.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let mut sf = ServicesFile::default();
        assert!(sf.remove("nothing").is_none());
    }

    #[test]
    fn multiple_services_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = state_path(&dir);

        let mut sf = ServicesFile::load_from(&path).unwrap();
        sf.insert("alpha".to_string(), ServiceEntry { port: 8001, ..sample_entry() });
        sf.insert("beta".to_string(), ServiceEntry { port: 8002, ..sample_entry() });
        sf.save().unwrap();

        let loaded = ServicesFile::load_from(&path).unwrap();
        assert_eq!(loaded.services.len(), 2);
        assert_eq!(loaded.services["alpha"].port, 8001);
        assert_eq!(loaded.services["beta"].port, 8002);
    }

    #[test]
    fn is_port_listening_closed_port_returns_false() {
        // Port 1 is reserved and almost certainly not listening
        assert!(!ServicesFile::is_port_listening(1));
    }

    #[test]
    fn is_alive_current_process() {
        let pid = std::process::id();
        assert!(ServicesFile::is_alive(pid));
    }

    #[test]
    fn is_alive_bogus_pid() {
        assert!(!ServicesFile::is_alive(2_147_483_647));
    }

    #[test]
    fn load_bad_toml_errors() {
        let dir = TempDir::new().unwrap();
        let path = state_path(&dir);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"not valid [[[ toml").unwrap();

        let err = ServicesFile::load_from(&path).unwrap_err();
        assert!(err.to_string().contains("failed to parse"));
    }
}
