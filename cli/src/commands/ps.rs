use std::time::Duration;

use anyhow::Result;

use crate::{eps::EpsManifest, state::ServicesFile, tailscale};

pub async fn run() -> Result<()> {
    let services = ServicesFile::load()?;

    if services.services.is_empty() {
        println!("No services running.");
        return Ok(());
    }

    let host = tailscale::ip().await?;
    let client = reqwest::Client::new();

    // Column widths
    let name_w = services.services.keys().map(|k| k.len()).max().unwrap_or(4).max(4);

    println!(
        "{:<name_w$}  {:>5}  {:>7}  {:<8}  {}",
        "NAME", "PORT", "PID", "STATUS", "URL",
        name_w = name_w,
    );
    println!("{}", "-".repeat(name_w + 5 + 7 + 8 + 40 + 4 * 2));

    let mut names: Vec<&String> = services.services.keys().collect();
    names.sort();

    for name in names {
        let entry = &services.services[name];

        let status = if !ServicesFile::is_port_listening(entry.port) {
            "stopped"
        } else {
            // Check if eps.toml declares a health_check endpoint
            let eps_path = std::path::Path::new(&entry.dir).join("eps.toml");
            let health_check = EpsManifest::from_file(&eps_path).ok()
                .and_then(|m| m.service)
                .and_then(|s| s.health_check);

            if let Some(_check) = health_check {
                let url = format!("http://{}:{}/health", host, entry.port);
                let ok = client
                    .get(&url)
                    .timeout(Duration::from_secs(2))
                    .send().await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false);
                if ok { "running" } else { "degraded" }
            } else {
                "running"
            }
        };

        let url = format!("http://{}:{}", host, entry.port);
        println!(
            "{:<name_w$}  {:>5}  {:>7}  {:<8}  {}",
            name, entry.port, entry.pid, status, url,
            name_w = name_w,
        );
    }

    Ok(())
}

