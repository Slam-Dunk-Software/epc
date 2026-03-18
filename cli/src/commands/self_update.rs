use std::io::Write;

use anyhow::{bail, Context, Result};

const REPO: &str = "Slam-Dunk-Software/epc";

fn asset_name() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "epc-macos-aarch64"
        } else {
            "epc-macos-x86_64"
        }
    } else if cfg!(target_arch = "x86_64") {
        "epc-linux-x86_64"
    } else {
        "epc-linux-x86_64"
    }
}

pub async fn run() -> Result<()> {
    eprintln!("\x1b[2mChecking for updates...\x1b[0m");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("epc/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp = client
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .send()
        .await
        .context("failed to reach GitHub API")?;

    if !resp.status().is_success() {
        bail!("GitHub API returned {}", resp.status());
    }

    let json: serde_json::Value = resp.json().await?;
    let latest = json["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');
    let current = env!("CARGO_PKG_VERSION");

    if latest.is_empty() {
        bail!("could not determine latest version — check https://github.com/{REPO}/releases");
    }

    let latest_ver = semver::Version::parse(latest)?;
    let current_ver = semver::Version::parse(current)?;

    if latest_ver <= current_ver {
        println!("\x1b[32m✓\x1b[0m Already up to date \x1b[2m(v{current})\x1b[0m");
        return Ok(());
    }

    println!(
        "\x1b[2mUpdating epc\x1b[0m \x1b[1mv{current}\x1b[0m \x1b[2m→\x1b[0m \x1b[1mv{latest}\x1b[0m\x1b[2m...\x1b[0m"
    );

    // Find the download URL for our platform's asset
    let asset_name = asset_name();
    let download_url = json["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| a["name"].as_str() == Some(asset_name))
                .and_then(|a| a["browser_download_url"].as_str())
                .map(String::from)
        })
        .with_context(|| format!("no asset '{asset_name}' found in release v{latest}"))?;

    // Download to a temp file next to the current binary
    let current_exe = std::env::current_exe().context("could not determine current binary path")?;
    let tmp_path = current_exe.with_extension("tmp");

    let bytes = client
        .get(&download_url)
        .send()
        .await
        .context("failed to download release")?
        .bytes()
        .await
        .context("failed to read download")?;

    {
        let mut f = std::fs::File::create(&tmp_path)
            .with_context(|| format!("failed to create temp file {}", tmp_path.display()))?;
        f.write_all(&bytes)?;
    }

    // Make it executable and atomically replace the current binary
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    std::fs::rename(&tmp_path, &current_exe).with_context(|| {
        format!(
            "failed to replace binary at {} — try running with sudo?",
            current_exe.display()
        )
    })?;

    println!("\n\x1b[32m✓\x1b[0m \x1b[1mepc v{latest}\x1b[0m installed.");
    Ok(())
}
