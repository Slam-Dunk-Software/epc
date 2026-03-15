#![allow(dead_code)]

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
