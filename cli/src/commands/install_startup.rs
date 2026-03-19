use anyhow::{Context, Result};

pub fn run() -> Result<()> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    let epc_path = std::env::current_exe().context("could not determine epc binary path")?;

    let log_path = home.join(".epc").join("logs").join("startup.log");
    let plist_label = "com.eps.epc-startup";
    let agents_dir = home.join("Library").join("LaunchAgents");
    let plist_path = agents_dir.join(format!("{plist_label}.plist"));

    // Check if already installed
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
