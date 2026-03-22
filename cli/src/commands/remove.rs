use std::path::Path;

use anyhow::{bail, Result};
use rusqlite;

use crate::state::{RegistryFile, ServiceEntry, ServicesFile};

pub fn run(name: &str) -> Result<()> {
    run_with_paths(
        name,
        &ServicesFile::default_path()?,
        &observatory_db_path()?,
    )
}

fn observatory_db_path() -> Result<std::path::PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?
        .join(".epc/observatory.db"))
}

pub fn run_with_paths(name: &str, state_path: &Path, db_path: &Path) -> Result<()> {
    let mut services = ServicesFile::load_from(state_path)?;

    let entry = match services.services.get(name) {
        Some(e) => e.clone(),
        None => bail!("no service named '{name}' is registered"),
    };

    // Kill by PGID then by port (both best-effort — process may already be dead)
    std::process::Command::new("kill")
        .args(["--", &format!("-{}", entry.pid)])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok();

    for pid in ServicesFile::pids_on_port(entry.port) {
        std::process::Command::new("kill")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok();
    }

    // Remove from state file and persistent registry
    services.remove(name);
    services.save()?;

    if let Ok(mut registry) = RegistryFile::load() {
        registry.remove(name);
        registry.save().ok();
    }

    // Delete log file (best-effort)
    let log = std::path::Path::new(&entry.log_file);
    if log.exists() {
        std::fs::remove_file(log).ok();
    }

    // Remove from Observatory DB (best-effort — Observatory may not be installed)
    remove_from_observatory(name, db_path).ok();

    println!("\x1b[31m✕\x1b[0m \x1b[1m{name}\x1b[0m \x1b[2mremoved\x1b[0m");

    Ok(())
}

fn remove_from_observatory(name: &str, db_path: &Path) -> Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute(
        "DELETE FROM service_state WHERE service = ?1",
        rusqlite::params![name],
    )?;
    conn.execute(
        "DELETE FROM health_checks WHERE service = ?1",
        rusqlite::params![name],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::{NamedTempFile, TempDir};

    fn sample_entry(log_file: &str) -> ServiceEntry {
        ServiceEntry {
            dir: "/tmp/some_pkg".to_string(),
            port: 19000,
            pid: 2_147_483_647, // bogus — won't be running
            started: "2026-01-01T00:00:00Z".to_string(),
            log_file: log_file.to_string(),
        }
    }

    fn seed_state(state_path: &Path, name: &str, entry: ServiceEntry) {
        let mut sf = ServicesFile::load_from(state_path).unwrap();
        sf.insert(name.to_string(), entry);
        sf.save().unwrap();
    }

    fn seed_observatory(path: &Path, name: &str) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS service_state (
                service TEXT PRIMARY KEY, last_status TEXT, last_checked TEXT, repo_url TEXT
            );
            CREATE TABLE IF NOT EXISTS health_checks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service TEXT, checked_at TEXT, status TEXT,
                response_ms INTEGER, status_code INTEGER
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO service_state VALUES (?1, 'degraded', '2026-01-01', NULL)",
            rusqlite::params![name],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO health_checks (service, checked_at, status) VALUES (?1, '2026-01-01', 'degraded')",
            rusqlite::params![name],
        )
        .unwrap();
    }

    fn count(path: &Path, table: &str, name: &str) -> usize {
        let conn = Connection::open(path).unwrap();
        conn.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE service = ?1"),
            rusqlite::params![name],
            |row| row.get::<_, usize>(0),
        )
        .unwrap()
    }

    #[test]
    fn remove_nonexistent_service_errors() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");
        let err = run_with_paths("ghost", &state_path, &db_path).unwrap_err();
        assert!(err.to_string().contains("no service named"));
    }

    #[test]
    fn remove_removes_from_state() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");
        seed_state(&state_path, "my_svc", sample_entry("/tmp/my_svc.log"));
        run_with_paths("my_svc", &state_path, &db_path).unwrap();
        let sf = ServicesFile::load_from(&state_path).unwrap();
        assert!(!sf.services.contains_key("my_svc"));
    }

    #[test]
    fn remove_deletes_log_file() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");
        let log = NamedTempFile::new().unwrap();
        let log_path = log.path().to_str().unwrap().to_string();
        seed_state(&state_path, "my_svc", sample_entry(&log_path));
        run_with_paths("my_svc", &state_path, &db_path).unwrap();
        assert!(!std::path::Path::new(&log_path).exists());
    }

    #[test]
    fn remove_works_without_log_file() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");
        seed_state(
            &state_path,
            "my_svc",
            sample_entry("/tmp/does_not_exist_ever.log"),
        );
        // Should not error even if log is already gone
        assert!(run_with_paths("my_svc", &state_path, &db_path).is_ok());
    }

    #[test]
    fn remove_works_without_observatory_db() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("no_observatory.db"); // doesn't exist
        seed_state(&state_path, "my_svc", sample_entry("/tmp/x.log"));
        // Should not error if observatory isn't installed
        assert!(run_with_paths("my_svc", &state_path, &db_path).is_ok());
    }

    #[test]
    fn remove_purges_observatory_db() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");
        seed_state(&state_path, "my_svc", sample_entry("/tmp/x.log"));
        seed_observatory(&db_path, "my_svc");
        run_with_paths("my_svc", &state_path, &db_path).unwrap();
        assert_eq!(count(&db_path, "service_state", "my_svc"), 0);
        assert_eq!(count(&db_path, "health_checks", "my_svc"), 0);
    }

    #[test]
    fn remove_leaves_other_observatory_entries_untouched() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");
        seed_state(&state_path, "target", sample_entry("/tmp/target.log"));
        seed_observatory(&db_path, "target");
        seed_observatory(&db_path, "keeper");
        run_with_paths("target", &state_path, &db_path).unwrap();
        assert_eq!(count(&db_path, "service_state", "keeper"), 1);
    }
}
