use anyhow::{Context, Result};

pub fn run() -> Result<()> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    let epc_path = std::env::current_exe().context("could not determine epc binary path")?;
    let log_path = home.join(".epc").join("logs").join("startup.log");

    #[cfg(target_os = "macos")]
    return run_macos(&home, &epc_path, &log_path);

    #[cfg(target_os = "linux")]
    return run_linux(&home, &epc_path, &log_path);

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        anyhow::bail!("epc install-startup is not supported on this platform.\nRun `epc startup` manually at login instead.");
    }
}

#[cfg(target_os = "macos")]
fn run_macos(home: &std::path::Path, epc_path: &std::path::Path, log_path: &std::path::Path) -> Result<()> {
    let plist_label = "com.eps.epc-startup";
    let agents_dir = home.join("Library").join("LaunchAgents");
    let plist_path = agents_dir.join(format!("{plist_label}.plist"));

    if plist_path.exists() {
        println!("EPC startup is already installed.");
        println!("  Plist: {}", plist_path.display());
        println!("\nTo reinstall:");
        println!("  launchctl unload {}", plist_path.display());
        println!("  rm {}", plist_path.display());
        println!("  epc install-startup");
        return Ok(());
    }

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{epc}</string>
        <string>startup</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        label = plist_label,
        epc = epc_path.display(),
        log = log_path.display(),
    );

    std::fs::create_dir_all(&agents_dir)?;
    std::fs::write(&plist_path, &plist)?;

    let status = std::process::Command::new("launchctl")
        .args(["load", &plist_path.to_string_lossy()])
        .status()
        .context("failed to run launchctl")?;

    if status.success() {
        println!("\x1b[32m✓\x1b[0m EPC startup installed");
        println!("  Your services will restart automatically on login.");
        println!("  Plist:  {}", plist_path.display());
        println!("  Logs:   {}", log_path.display());
        println!("  Binary: {}", epc_path.display());
        println!("\nTo test it now:  epc startup");
        println!(
            "To uninstall:    launchctl unload {path} && rm {path}",
            path = plist_path.display()
        );
    } else {
        eprintln!("Warning: plist written but launchctl load failed.");
        eprintln!("Try manually:");
        eprintln!("  launchctl load {}", plist_path.display());
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn run_linux(home: &std::path::Path, epc_path: &std::path::Path, log_path: &std::path::Path) -> Result<()> {
    let systemd_dir = home.join(".config").join("systemd").join("user");
    let unit_path = systemd_dir.join("epc-startup.service");

    if unit_path.exists() {
        println!("EPC startup is already installed.");
        println!("  Unit: {}", unit_path.display());
        println!("\nTo reinstall:");
        println!("  systemctl --user disable --now epc-startup");
        println!("  rm {}", unit_path.display());
        println!("  epc install-startup");
        return Ok(());
    }

    let unit = format!(
        r#"[Unit]
Description=EPC startup — restart EPS services on login
After=network.target

[Service]
Type=oneshot
ExecStart={epc} startup
StandardOutput=append:{log}
StandardError=append:{log}
RemainAfterExit=yes

[Install]
WantedBy=default.target
"#,
        epc = epc_path.display(),
        log = log_path.display(),
    );

    std::fs::create_dir_all(&systemd_dir)?;
    std::fs::write(&unit_path, &unit)?;

    // Enable and start via systemctl --user
    let enable_ok = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "epc-startup"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if enable_ok {
        println!("\x1b[32m✓\x1b[0m EPC startup installed");
        println!("  Your services will restart automatically on login.");
        println!("  Unit:   {}", unit_path.display());
        println!("  Logs:   {}", log_path.display());
        println!("  Binary: {}", epc_path.display());
        println!("\nTo test it now:  epc startup");
        println!("To uninstall:    systemctl --user disable --now epc-startup && rm {}", unit_path.display());
    } else {
        // systemctl may not be available (e.g. container, no dbus) — unit is written, give manual instructions
        println!("\x1b[32m✓\x1b[0m Unit file written to {}", unit_path.display());
        println!("  Could not run systemctl --user (is systemd running?)");
        println!("  Enable manually:");
        println!("    systemctl --user enable --now epc-startup");
        println!("    loginctl enable-linger $USER");
    }

    Ok(())
}
