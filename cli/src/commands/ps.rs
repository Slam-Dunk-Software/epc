use std::time::Duration;

use anyhow::Result;

use crate::{eps::EpsManifest, state::ServicesFile, tailscale};

pub async fn run() -> Result<()> {
    let services = ServicesFile::load()?;

    if services.services.is_empty() {
        println!("\x1b[2mNo services running.\x1b[0m");
        return Ok(());
    }

    let host = tailscale::ip().await?;
    let client = reqwest::Client::new();

    // Column widths
    let name_w = services.services.keys().map(|k| k.len()).max().unwrap_or(4).max(4);

    println!(
        "\x1b[2m{:<name_w$}  {:>5}  {:>7}  {:<8}  {}\x1b[0m",
        "NAME", "PORT", "PID", "STATUS", "URL",
        name_w = name_w,
    );
    println!("\x1b[2m{}\x1b[0m", "─".repeat(name_w + 5 + 7 + 8 + 40 + 4 * 2));

    let mut names: Vec<&String> = services.services.keys().collect();
    names.sort();

    for name in names {
        let entry = &services.services[name];

        let status_str = if !ServicesFile::is_port_listening(entry.port) {
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

        let status_colored = match status_str {
            "running"  => format!("\x1b[32m{:<8}\x1b[0m", status_str),
            "degraded" => format!("\x1b[33m{:<8}\x1b[0m", status_str),
            _          => format!("\x1b[31m{:<8}\x1b[0m", status_str),
        };

        let url = format!("http://{}:{}", host, entry.port);
        println!(
            "\x1b[1m{:<name_w$}\x1b[0m  {:>5}  {:>7}  {}  \x1b[36m{}\x1b[0m",
            name, entry.port, entry.pid, status_colored, url,
            name_w = name_w,
        );
    }

    Ok(())
}

