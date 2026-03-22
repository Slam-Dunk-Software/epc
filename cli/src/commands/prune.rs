use std::path::Path;

use anyhow::Result;
use rusqlite;

use crate::state::{RegistryFile, ServicesFile};

pub fn run() -> Result<()> {
    run_internal(
        &ServicesFile::default_path()?,
        &observatory_db_path()?,
        None,
    )
}

fn observatory_db_path() -> Result<std::path::PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?
        .join(".epc/observatory.db"))
}

/// `confirm` overrides the interactive prompt:
///   Some(true)  = proceed without asking
///   Some(false) = abort without asking
///   None        = ask the user
pub fn run_internal(state_path: &Path, db_path: &Path, confirm: Option<bool>) -> Result<()> {
    let mut services = ServicesFile::load_from(state_path)?;

    // Collect services whose project directory no longer exists
    let mut stale: Vec<(String, crate::state::ServiceEntry)> = services
        .services
        .iter()
        .filter(|(_, entry)| !std::path::Path::new(&entry.dir).exists())
        .map(|(name, entry)| (name.clone(), entry.clone()))
        .collect();
    stale.sort_by(|a, b| a.0.cmp(&b.0));

    if stale.is_empty() {
        println!("\x1b[32m✓\x1b[0m  no stale services found");
        return Ok(());
    }

    println!("Stale services (project directory no longer exists):\n");
    for (name, entry) in &stale {
        println!("  \x1b[1m{name}\x1b[0m  \x1b[2m{}\x1b[0m", entry.dir);
    }

    let proceed = match confirm {
        Some(v) => v,
        None => {
            print!(
                "\nRemove {} service{}? [y/N] ",
                stale.len(),
                if stale.len() == 1 { "" } else { "s" }
            );
            use std::io::Write as _;
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            input.trim().eq_ignore_ascii_case("y")
        }
    };

    if !proceed {
        println!("aborted");
        return Ok(());
    }

    for (name, entry) in &stale {
        // Kill process (best-effort)
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

        services.remove(name);

        if let Ok(mut registry) = RegistryFile::load() {
            registry.remove(name);
            registry.save().ok();
        }

        let log = std::path::Path::new(&entry.log_file);
        if log.exists() {
            std::fs::remove_file(log).ok();
        }

        remove_from_observatory(name, db_path).ok();

        println!("\x1b[31m✕\x1b[0m \x1b[1m{name}\x1b[0m \x1b[2mremoved\x1b[0m");
    }

    services.save()?;
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

    use crate::state::{ServiceEntry, ServicesFile};

    fn entry_with_missing_dir(log_file: &str) -> ServiceEntry {
        ServiceEntry {
            dir: "/tmp/does_not_exist_ever_xyz".to_string(),
            port: 19001,
            pid: 2_147_483_647,
            started: "2026-01-01T00:00:00Z".to_string(),
            log_file: log_file.to_string(),
        }
    }

    fn entry_with_real_dir(dir: &TempDir, log_file: &str) -> ServiceEntry {
        ServiceEntry {
            dir: dir.path().to_str().unwrap().to_string(),
            port: 19002,
            pid: 2_147_483_647,
            started: "2026-01-01T00:00:00Z".to_string(),
            log_file: log_file.to_string(),
        }
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
            "INSERT OR REPLACE INTO service_state VALUES (?1, 'degraded', '2026-01-01', NULL)",
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
    fn prune_nothing_stale_reports_clean() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");
        // No services registered at all
        let result = run_internal(&state_path, &db_path, Some(true));
        assert!(result.is_ok());
    }

    #[test]
    fn prune_keeps_services_with_existing_dirs() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");

        let project_dir = TempDir::new().unwrap();
        let mut sf = ServicesFile::load_from(&state_path).unwrap();
        sf.insert(
            "keeper".to_string(),
            entry_with_real_dir(&project_dir, "/tmp/keeper.log"),
        );
        sf.save().unwrap();

        run_internal(&state_path, &db_path, Some(true)).unwrap();

        let sf = ServicesFile::load_from(&state_path).unwrap();
        assert!(sf.services.contains_key("keeper"));
    }

    #[test]
    fn prune_confirmed_removes_stale_from_state() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");

        let mut sf = ServicesFile::load_from(&state_path).unwrap();
        sf.insert(
            "stale_svc".to_string(),
            entry_with_missing_dir("/tmp/stale.log"),
        );
        sf.save().unwrap();

        run_internal(&state_path, &db_path, Some(true)).unwrap();

        let sf = ServicesFile::load_from(&state_path).unwrap();
        assert!(!sf.services.contains_key("stale_svc"));
    }

    #[test]
    fn prune_confirmed_deletes_log_file() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");
        let log = NamedTempFile::new().unwrap();
        let log_path = log.path().to_str().unwrap().to_string();

        let mut sf = ServicesFile::load_from(&state_path).unwrap();
        sf.insert("stale_svc".to_string(), entry_with_missing_dir(&log_path));
        sf.save().unwrap();

        run_internal(&state_path, &db_path, Some(true)).unwrap();
        assert!(!std::path::Path::new(&log_path).exists());
    }

    #[test]
    fn prune_confirmed_purges_observatory() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");

        let mut sf = ServicesFile::load_from(&state_path).unwrap();
        sf.insert(
            "stale_svc".to_string(),
            entry_with_missing_dir("/tmp/stale.log"),
        );
        sf.save().unwrap();
        seed_observatory(&db_path, "stale_svc");

        run_internal(&state_path, &db_path, Some(true)).unwrap();
        assert_eq!(count(&db_path, "service_state", "stale_svc"), 0);
        assert_eq!(count(&db_path, "health_checks", "stale_svc"), 0);
    }

    #[test]
    fn prune_aborted_leaves_state_untouched() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");

        let mut sf = ServicesFile::load_from(&state_path).unwrap();
        sf.insert(
            "stale_svc".to_string(),
            entry_with_missing_dir("/tmp/stale.log"),
        );
        sf.save().unwrap();

        run_internal(&state_path, &db_path, Some(false)).unwrap();

        let sf = ServicesFile::load_from(&state_path).unwrap();
        assert!(sf.services.contains_key("stale_svc"));
    }

    #[test]
    fn prune_keeps_other_observatory_entries() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let db_path = dir.path().join("observatory.db");

        let mut sf = ServicesFile::load_from(&state_path).unwrap();
        sf.insert(
            "stale_svc".to_string(),
            entry_with_missing_dir("/tmp/stale.log"),
        );
        sf.save().unwrap();
        seed_observatory(&db_path, "stale_svc");
        seed_observatory(&db_path, "keeper");

        run_internal(&state_path, &db_path, Some(true)).unwrap();
        assert_eq!(count(&db_path, "service_state", "keeper"), 1);
    }
}
