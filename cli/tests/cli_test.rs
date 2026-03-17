mod common;

use assert_cmd::Command;
use common::TempHome;
use predicates::prelude::*;
use tempfile::TempDir;

fn epc(home: &TempDir) -> Command {
    let mut c = Command::cargo_bin("epc").unwrap();
    c.env("HOME", home.path());
    c
}

// ── Help / structure ─────────────────────────────────────────────────────────

#[test]
fn help_lists_all_subcommands() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("deploy"))
        .stdout(predicate::str::contains("ps"))
        .stdout(predicate::str::contains("logs"))
        .stdout(predicate::str::contains("stop"));
}

#[test]
fn no_args_prints_help_and_fails() {
    let home = TempDir::new().unwrap();
    epc(&home).assert().failure();
}

#[test]
fn deploy_help_shows_local_flag() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["deploy", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("local"));
}

#[test]
fn logs_help_shows_name_arg() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["logs", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("name"));
}

#[test]
fn stop_help_shows_name_arg() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["stop", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("name"));
}

#[test]
fn unknown_subcommand_fails() {
    let home = TempDir::new().unwrap();
    epc(&home).arg("notacommand").assert().failure();
}

// ── ps ────────────────────────────────────────────────────────────────────────

#[test]
fn ps_with_no_services_prints_message() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .arg("ps")
        .assert()
        .success()
        .stdout(predicate::str::contains("No services running."));
}

// ── deploy errors ─────────────────────────────────────────────────────────────

#[test]
fn deploy_uninstalled_package_fails_with_hint() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["deploy", "nonexistent_pkg"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not installed"));
}

#[test]
fn deploy_local_missing_path_fails() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["deploy", "x", "--local", "/no/such/path"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("could not resolve"));
}

#[test]
fn deploy_local_no_eps_toml_fails() {
    let home = TempDir::new().unwrap();
    let pkg_dir = TempDir::new().unwrap(); // no eps.toml inside
    epc(&home)
        .args(["deploy", "x", "--local", pkg_dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to read"));
}

#[test]
fn deploy_local_no_service_block_fails_informatively() {
    let home = TempDir::new().unwrap();
    let pkg_dir = TempDir::new().unwrap();
    std::fs::write(
        pkg_dir.path().join("eps.toml"),
        r#"
[package]
name = "no_service_pkg"
version = "0.1.0"
description = "x"
authors = []
license = "MIT"
platforms = []
repository = ""
"#,
    )
    .unwrap();

    epc(&home)
        .args(["deploy", "no_service_pkg", "--local", pkg_dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no [service] block"));
}

// ── stop success ──────────────────────────────────────────────────────────────

#[test]
fn stop_registered_service_prints_styled_confirmation() {
    let home = TempHome::new();
    // Seed services.toml with a stopped (non-running) service
    std::fs::write(
        home.services_toml(),
        r#"[services.my_svc]
dir      = "/tmp/my_svc"
port     = 19876
pid      = 2147483647
started  = "2026-01-01T00:00:00Z"
log_file = "/tmp/.epc/logs/my_svc.log"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("epc").unwrap();
    cmd.env("HOME", home.dir.path())
        .args(["stop", "my_svc"])
        .assert()
        .success()
        .stdout(predicate::str::contains("✕"))
        .stdout(predicate::str::contains("my_svc"))
        .stdout(predicate::str::contains("stopped"));
}

// ── stop errors ───────────────────────────────────────────────────────────────

#[test]
fn stop_unregistered_service_fails_with_hint() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["stop", "nothing_here"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no service named"));
}

// ── observatory ───────────────────────────────────────────────────────────────

#[test]
fn observatory_help_shows_rm_subcommand() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["observatory", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("rm"));
}

#[test]
fn observatory_rm_help_mentions_service_names() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["observatory", "rm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("names"));
}

#[test]
fn observatory_rm_no_args_fails() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["observatory", "rm"])
        .assert()
        .failure();
}

#[test]
fn observatory_rm_missing_db_fails_with_hint() {
    let home = TempDir::new().unwrap();
    // No observatory.db seeded — should fail with a helpful message
    epc(&home)
        .args(["observatory", "rm", "mirror"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("observatory database not found"));
}

// ── observatory success ───────────────────────────────────────────────────────

#[test]
fn observatory_rm_prints_styled_confirmation() {
    use rusqlite::Connection;

    let home = TempHome::new();
    let db_path = home.epc_dir().join("observatory.db");

    // Seed a minimal observatory db
    let conn = Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE service_state (service TEXT PRIMARY KEY, last_status TEXT, last_checked TEXT, repo_url TEXT);
         CREATE TABLE health_checks (id INTEGER PRIMARY KEY AUTOINCREMENT, service TEXT, checked_at TEXT, status TEXT, response_ms INTEGER, status_code INTEGER);
         INSERT INTO service_state VALUES ('mirror', 'stopped', '2026-01-01', NULL);
         INSERT INTO health_checks (service, checked_at, status) VALUES ('mirror', '2026-01-01', 'stopped');",
    ).unwrap();
    drop(conn);

    let mut cmd = Command::cargo_bin("epc").unwrap();
    cmd.env("HOME", home.dir.path())
        .args(["observatory", "rm", "mirror"])
        .assert()
        .success()
        .stdout(predicate::str::contains("✕"))
        .stdout(predicate::str::contains("mirror"))
        .stdout(predicate::str::contains("removed"));
}

// ── ps styled output ──────────────────────────────────────────────────────────

#[test]
fn ps_no_services_contains_message() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .arg("ps")
        .assert()
        .success()
        .stdout(predicate::str::contains("No services running"));
}

// ── logs errors ───────────────────────────────────────────────────────────────

#[test]
fn logs_unregistered_service_fails_with_hint() {
    let home = TempDir::new().unwrap();
    epc(&home)
        .args(["logs", "nothing_here"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no service named"));
}
