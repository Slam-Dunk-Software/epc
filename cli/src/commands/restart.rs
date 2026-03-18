use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::{
    eps::EpsManifest,
    state::{ServiceEntry, ServicesFile},
    tailscale,
};

pub async fn run(name: &str) -> Result<()> {
    run_with_state(name, &ServicesFile::default_path()?).await
}

pub async fn run_with_state(name: &str, state_path: &Path) -> Result<()> {
    let mut services = ServicesFile::load_from(state_path)?;

    let entry = match services.services.get(name) {
        Some(e) => e.clone(),
        None => bail!("no service named '{name}' is registered"),
    };

    // Kill the entire process group first (PGID = entry.pid, set at deploy time
    // via process_group(0)). This catches the bash wrapper and any children that
    // haven't bound to the port yet — lsof only sees already-listening processes.
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

    // Wait for the port to go quiet (up to 5 s)
    let deadline = Instant::now() + Duration::from_secs(5);
    while ServicesFile::is_port_listening(entry.port) {
        if Instant::now() >= deadline {
            bail!("port {} is still occupied after stopping; cannot restart '{name}'", entry.port);
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Remove the stale state entry before respawning
    services.remove(name);
    services.save()?;

    // Re-read manifest from the directory already recorded in state
    let pkg_dir = std::path::PathBuf::from(&entry.dir);
    let manifest = EpsManifest::from_file(&pkg_dir.join("eps.toml"))?;
    let svc = manifest.require_service()?;

    // Create a fresh log file
    let log_dir = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".epc")
        .join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join(format!("{name}.log"));
    let log_file = File::create(&log_path)
        .with_context(|| format!("failed to create log file {}", log_path.display()))?;
    let log_stderr = log_file.try_clone()?;

    // Spawn the service process again, in its own process group.
    let child = tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&svc.start)
        .current_dir(&pkg_dir)
        .env("PORT", svc.port.to_string())
        .process_group(0)
        .stdout(log_file)
        .stderr(log_stderr)
        .spawn()
        .with_context(|| format!("failed to spawn '{}'", svc.start))?;

    let pid = child.id().context("failed to get PID of spawned process")?;

    let host = tailscale::ip().await?;

    let started = chrono::Utc::now().to_rfc3339();
    let new_entry = ServiceEntry {
        dir: entry.dir,
        port: svc.port,
        pid,
        started,
        log_file: log_path.to_string_lossy().to_string(),
    };
    services.insert(name.to_string(), new_entry);
    services.save()?;

    println!("\n\x1b[32m✓\x1b[0m \x1b[1m{name}\x1b[0m restarted \x1b[36m→ http://{host}:{}\x1b[0m", svc.port);
    println!("  \x1b[2mpid   {pid}\x1b[0m");
    println!("  \x1b[2mlogs  {}\x1b[0m", log_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn restart_nonexistent_service_errors() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let err = run_with_state("ghost", &state_path).await.unwrap_err();
        assert!(err.to_string().contains("no service named"));
    }
}
