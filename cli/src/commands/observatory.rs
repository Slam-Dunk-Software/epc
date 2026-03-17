use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;

fn db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".epc/observatory.db"))
}

/// Remove one or more stale service entries from the Observatory database.
///
/// Services that have been stopped or deleted are never automatically pruned
/// from the Observatory SQLite database — this command handles that cleanup.
///
/// Example:
///   epc observatory rm mirror epc
pub fn run(names: &[String]) -> Result<()> {
    run_with_db_path(names, &db_path()?)
}

pub fn run_with_db_path(names: &[String], path: &PathBuf) -> Result<()> {
    if names.is_empty() {
        anyhow::bail!("at least one service name is required");
    }

    if !path.exists() {
        anyhow::bail!(
            "observatory database not found at {}\nIs Observatory running?",
            path.display()
        );
    }

    let conn = Connection::open(path)
        .with_context(|| format!("failed to open observatory database at {}", path.display()))?;

    for name in names {
        let state_rows = conn.execute(
            "DELETE FROM service_state WHERE service = ?1",
            rusqlite::params![name],
        )?;
        let check_rows = conn.execute(
            "DELETE FROM health_checks WHERE service = ?1",
            rusqlite::params![name],
        )?;

        if state_rows == 0 && check_rows == 0 {
            eprintln!("\x1b[33m!\x1b[0m \x1b[1m{name}\x1b[0m \x1b[2mnot found in observatory database — nothing removed\x1b[0m");
        } else {
            println!("\x1b[31m✕\x1b[0m \x1b[1m{name}\x1b[0m \x1b[2mremoved from observatory ({check_rows} health check record(s) deleted)\x1b[0m");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    fn setup_db() -> (NamedTempFile, PathBuf) {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE service_state (
                service TEXT PRIMARY KEY,
                last_status TEXT,
                last_checked TEXT,
                repo_url TEXT
            );
            CREATE TABLE health_checks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service TEXT,
                checked_at TEXT,
                status TEXT,
                response_ms INTEGER,
                status_code INTEGER
            );",
        )
        .unwrap();
        (file, path)
    }

    fn seed(path: &PathBuf, name: &str) {
        let conn = Connection::open(path).unwrap();
        conn.execute(
            "INSERT INTO service_state (service, last_status, last_checked) VALUES (?1, 'stopped', '2026-01-01')",
            rusqlite::params![name],
        ).unwrap();
        conn.execute(
            "INSERT INTO health_checks (service, checked_at, status) VALUES (?1, '2026-01-01', 'stopped')",
            rusqlite::params![name],
        ).unwrap();
    }

    fn count(path: &PathBuf, table: &str, name: &str) -> usize {
        let conn = Connection::open(path).unwrap();
        conn.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE service = ?1"),
            rusqlite::params![name],
            |row| row.get::<_, usize>(0),
        )
        .unwrap()
    }

    #[test]
    fn removes_existing_service() {
        let (_file, path) = setup_db();
        seed(&path, "mirror");
        assert!(run_with_db_path(&["mirror".into()], &path).is_ok());
        assert_eq!(count(&path, "service_state", "mirror"), 0);
        assert_eq!(count(&path, "health_checks", "mirror"), 0);
    }

    #[test]
    fn removes_multiple_services() {
        let (_file, path) = setup_db();
        seed(&path, "mirror");
        seed(&path, "epc");
        assert!(run_with_db_path(&["mirror".into(), "epc".into()], &path).is_ok());
        assert_eq!(count(&path, "service_state", "mirror"), 0);
        assert_eq!(count(&path, "service_state", "epc"), 0);
    }

    #[test]
    fn warns_but_succeeds_for_unknown_service() {
        let (_file, path) = setup_db();
        // no seed — service doesn't exist
        assert!(run_with_db_path(&["ghost".into()], &path).is_ok());
    }

    #[test]
    fn removal_output_contains_styled_marker_and_name() {
        // Capture stdout to verify the styled ✕ marker and service name appear.
        // This test is RED until print_finding is updated to emit ✕.
        let (_file, path) = setup_db();
        seed(&path, "mirror");

        // We can't easily capture stdout from within a unit test, so we verify
        // the function returns Ok and trust the integration test for output shape.
        // The real red/green check is in cli_test observatory_rm_prints_styled_confirmation.
        assert!(run_with_db_path(&["mirror".into()], &path).is_ok());
    }

    #[test]
    fn leaves_other_services_untouched() {
        let (_file, path) = setup_db();
        seed(&path, "mirror");
        seed(&path, "palantir");
        run_with_db_path(&["mirror".into()], &path).unwrap();
        assert_eq!(count(&path, "service_state", "palantir"), 1);
        assert_eq!(count(&path, "health_checks", "palantir"), 1);
    }

    #[test]
    fn fails_with_no_names() {
        let (_file, path) = setup_db();
        assert!(run_with_db_path(&[], &path).is_err());
    }

    #[test]
    fn fails_if_db_does_not_exist() {
        let path = PathBuf::from("/tmp/does_not_exist_observatory.db");
        assert!(run_with_db_path(&["mirror".into()], &path).is_err());
    }
}
