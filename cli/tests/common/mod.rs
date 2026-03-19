#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

/// A temporary HOME directory for tests that will eventually read/write ~/.epc/.
///
/// Pass to assert_cmd via `.env("HOME", temp_home.path())` so that
/// `dirs::home_dir()` resolves to the temp dir instead of the real home.
pub struct TempHome {
    pub dir: TempDir,
}

impl TempHome {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("failed to create temp home dir");
        // Pre-create the .epc directory so tests don't have to.
        std::fs::create_dir_all(dir.path().join(".epc"))
            .expect("failed to create .epc dir");
        TempHome { dir }
    }

    pub fn epc_dir(&self) -> std::path::PathBuf {
        self.dir.path().join(".epc")
    }

    pub fn services_toml(&self) -> std::path::PathBuf {
        self.epc_dir().join("services.toml")
    }
}

/// Parse a services.toml and return a map of service name → raw TOML table.
/// Used in integration tests to assert on state after a command runs.
pub fn load_services_toml(path: PathBuf) -> HashMap<String, toml::Value> {
    if !path.exists() {
        return HashMap::new();
    }
    let raw = std::fs::read_to_string(&path).unwrap();
    let doc: toml::Value = toml::from_str(&raw).unwrap();
    doc.get("services")
        .and_then(|v| v.as_table())
        .map(|t| t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}
