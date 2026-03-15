use std::path::Path;

use anyhow::{bail, Result};

use crate::state::ServicesFile;

pub fn run(name: &str) -> Result<()> {
    run_with_state(name, &ServicesFile::default_path()?)
}

pub fn run_with_state(name: &str, state_path: &Path) -> Result<()> {
    let mut services = ServicesFile::load_from(state_path)?;

    let entry = match services.services.get(name) {
        Some(e) => e.clone(),
        None => bail!("no service named '{name}' is registered"),
    };

    // Kill the entire process group first (PGID = entry.pid, set at deploy time
    // via process_group(0)). This catches the bash wrapper and any children that
    // haven't bound to the port yet.
    std::process::Command::new("kill")
        .args(["--", &format!("-{}", entry.pid)])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok();

    // Also kill anything still on the port — handles processes whose PGID drifted
    // or services deployed before this fix.
    for pid in ServicesFile::pids_on_port(entry.port) {
        std::process::Command::new("kill")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok();
    }

    services.remove(name);
    services.save()?;

    println!("Stopped {name}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ServiceEntry, ServicesFile};
    use tempfile::TempDir;

    fn sample_entry(pid: u32) -> ServiceEntry {
        ServiceEntry {
            dir: "/tmp/pkg".to_string(),
            port: 9000,
            pid,
            started: "2026-02-28T00:00:00Z".to_string(),
            log_file: "/tmp/.epc/logs/pkg.log".to_string(),
        }
    }

    #[test]
    fn stop_nonexistent_service_errors() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let err = run_with_state("no_such_service", &state_path).unwrap_err();
        assert!(err.to_string().contains("no service named"));
    }

    #[test]
    fn stop_removes_entry_from_state_file() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");

        // Spawn a real process that we can safely SIGTERM
        let child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("failed to spawn sleep");
        let pid = child.id();

        let mut sf = ServicesFile::load_from(&state_path).unwrap();
        sf.insert("test_svc".to_string(), sample_entry(pid));
        sf.save().unwrap();

        run_with_state("test_svc", &state_path).unwrap();

        let loaded = ServicesFile::load_from(&state_path).unwrap();
        assert!(!loaded.services.contains_key("test_svc"));
    }

    #[test]
    fn stop_after_process_already_dead() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");

        let mut sf = ServicesFile::load_from(&state_path).unwrap();
        // PID that doesn't exist — kill will fail but we still remove the entry
        sf.insert("dead_svc".to_string(), sample_entry(2_147_483_647));
        sf.save().unwrap();

        // This may or may not error depending on kill exit code — but the entry is removed
        let _ = run_with_state("dead_svc", &state_path);
        let loaded = ServicesFile::load_from(&state_path).unwrap();
        assert!(!loaded.services.contains_key("dead_svc"));
    }
}
