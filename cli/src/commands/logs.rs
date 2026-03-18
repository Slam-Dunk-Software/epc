use anyhow::{bail, Context, Result};

use crate::state::ServicesFile;

pub async fn run(name: &str) -> Result<()> {
    let services = ServicesFile::load()?;

    let entry = match services.services.get(name) {
        Some(e) => e,
        None => bail!("no service named '{name}' is registered"),
    };

    let log_path = std::path::Path::new(&entry.log_file);
    if !log_path.exists() {
        bail!("log file not found: {}", log_path.display());
    }

    eprintln!("\x1b[2mstreaming logs for\x1b[0m \x1b[1m{name}\x1b[0m \x1b[2m— Ctrl-C to stop\x1b[0m");

    // Stream the log file with `tail -f`
    let status = tokio::process::Command::new("tail")
        .args(["-f", &entry.log_file])
        .status()
        .await
        .context("failed to run tail")?;

    if !status.success() {
        bail!("tail exited with status {status}");
    }

    Ok(())
}
