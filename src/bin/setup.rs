//! Wiki Setup Tool
//!
//! Interactive wizard for configuring and installing the Wiki Server.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;
use std::io::Write;

use anyhow::{Result, Context};
use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Select, Input, Confirm, MultiSelect};
use console::style;

#[derive(Parser)]
#[command(name = "wiki-setup")]
#[command(author, version, about = "Setup wizard for Wiki Server")]
struct Cli {
    /// Non-interactive mode (use defaults or flags)
    #[arg(long)]
    non_interactive: bool,

    /// Install directory
    #[arg(long)]
    install_dir: Option<PathBuf>,

    /// Data directory
    #[arg(long)]
    data_dir: Option<PathBuf>,

    /// Port
    #[arg(long)]
    port: Option<u16>,

    /// Language to download (simple, en, etc.)
    #[arg(long)]
    lang: Option<String>,

    /// Prune links
    #[arg(long)]
    prune: Option<bool>,
}

fn main() -> Result<()> {
    // Enable ANSI support on Windows
    // #[cfg(windows)]
    // let _ = console::enable_ansi_support();

    println!();
    println!("{}", style("╔══════════════════════════════════════════════════════════════════╗").cyan());
    println!("{}", style("║                     🛠️  WIKI SETUP                                ║").cyan());
    println!("{}", style("╚══════════════════════════════════════════════════════════════════╝").cyan());
    println!();

    let cli = Cli::parse();

    if cli.non_interactive {
        println!("Running in non-interactive mode...");
        // TODO: Implement non-interactive logic
        return Ok(());
    }

    // 1. Language Selection
    let languages = vec![
        "simple (Recommended for testing, ~300MB)",
        "en (Full English Wikipedia, ~22GB download, ~90GB extracted)",
        "de (German)",
        "fr (French)",
        "es (Spanish)",
        "custom (Enter code manually)",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select Wikipedia Language")
        .default(0)
        .items(&languages)
        .interact()?;

    let lang_code = match selection {
        0 => "simple".to_string(),
        1 => "en".to_string(),
        2 => "de".to_string(),
        3 => "fr".to_string(),
        4 => "es".to_string(),
        5 => Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter language code")
            .interact_text()?,
        _ => unreachable!(),
    };

    // 2. Data Directory
    let default_data_dir = std::env::current_dir()?.join("wikipedia");
    let data_dir: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Where should data be stored?")
        .default(default_data_dir.to_string_lossy().to_string())
        .interact_text()?;
    let data_dir = PathBuf::from(data_dir);

    // 3. Port
    let port: u16 = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Web Server Port")
        .default(3000)
        .interact_text()?;

    // 4. Pruning
    let default_prune = lang_code == "simple";
    let prune = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Prune broken links? (Removes links to missing articles)")
        .default(default_prune)
        .interact()?;

    // 5. Service Installation
    let install_service = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Install as a background service?")
        .default(true)
        .interact()?;

    // 6. Auto-Update
    let auto_update = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Enable auto-updates? (Weekly check)")
        .default(false)
        .interact()?;

    println!("\n{}", style("Configuration Summary:").bold());
    println!("  Language: {}", style(&lang_code).green());
    println!("  Data Dir: {}", style(data_dir.display()).green());
    println!("  Port:     {}", style(port).green());
    println!("  Prune:    {}", style(if prune { "Yes" } else { "No" }).green());
    println!("  Service:  {}", style(if install_service { "Yes" } else { "No" }).green());
    println!("  Updates:  {}", style(if auto_update { "Yes" } else { "No" }).green());
    println!();

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Proceed with installation?")
        .default(true)
        .interact()?
    {
        println!("Aborted.");
        return Ok(());
    }

    // --- Execution ---

    // 1. Create directories
    fs::create_dir_all(&data_dir).context("Failed to create data directory")?;

    // 2. Save Config
    let config_path = data_dir.join("config.json");
    let config_json = serde_json::json!({
        "language": lang_code,
        "port": port,
        "prune": prune,
        "auto_update": auto_update
    });
    fs::write(&config_path, serde_json::to_string_pretty(&config_json)?)?;
    println!("✅ Configuration saved to {:?}", config_path);

    // 3. Download Content (if needed)
    // We invoke the wiki-download binary. 
    // Assuming it's in the same directory as this executable or in PATH.
    let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
    let downloader_exe = if cfg!(windows) { "wiki-download.exe" } else { "wiki-download" };
    let downloader_path = exe_dir.join(downloader_exe);

    // Check if we should run download
    // For now, let's just run it if the user confirmed.
    
    println!("\n🚀 Starting Download & Extraction...");
    println!("   (This may take a while depending on your selection)\n");

    let mut args = vec![
        "--lang".to_string(), lang_code.clone(),
        "--output".to_string(), data_dir.to_string_lossy().to_string(),
    ];
    
    if prune {
        args.push("--prune-links".to_string());
    }

    // We use standard Command to run it and let it inherit stdout/stderr so user sees progress bars
    let status = Command::new(&downloader_path)
        .args(&args)
        .status();

    match status {
        Ok(s) if s.success() => println!("\n✅ Download complete!"),
        Ok(s) => println!("\n❌ Download failed with exit code: {:?}", s.code()),
        Err(e) => {
            println!("\n⚠️  Could not find or run wiki-download: {}", e);
            println!("   Please run it manually: wiki-download --lang {} --output {:?}", lang_code, data_dir);
        }
    }

    // 4. Install Service
    if install_service {
        install_system_service(&exe_dir, &data_dir, port)?;
    }

    // 5. Setup Auto-Update
    if auto_update {
        setup_auto_update(&exe_dir, &data_dir, &lang_code)?;
    }

    println!("\n{}", style("🎉 Setup Complete!").bold().green());
    if install_service {
        println!("Service should be running on http://localhost:{}", port);
    } else {
        println!("Run the server manually:");
        println!("  wiki-serve --data {:?} --port {}", data_dir, port);
    }

    Ok(())
}

fn install_system_service(exe_dir: &Path, data_dir: &Path, port: u16) -> Result<()> {
    println!("\n🛠️  Installing Service...");

    #[cfg(target_os = "windows")]
    {
        // Use sc.exe
        // sc create wiki-serve binPath= "C:\Path\wiki-serve.exe --data C:\Data --port 3000" start= auto
        let bin_path = exe_dir.join("wiki-serve.exe");
        let cmd = format!(
            "\"{}\" --data \"{}\" --port {}", 
            bin_path.to_string_lossy(), 
            data_dir.to_string_lossy(), 
            port
        );
        
        // We need to be careful with quoting for sc.exe binPath
        // sc create wiki-serve binPath= "\"C:\Path\wiki-serve.exe\" --data ..."
        // let sc_bin_path = format!("\\\"{}\\\" --data \\\"{}\\\" --port {}", 
        //     bin_path.to_string_lossy(),
        //     data_dir.to_string_lossy(),
        //     port
        // );

        let status = Command::new("sc")
            .arg("create")
            .arg("wiki-serve")
            .arg("binPath=")
            .arg(&cmd) // Rust Command handles quoting of the argument itself, but sc expects the string to contain the command line
            .arg("start=")
            .arg("auto")
            .arg("DisplayName=")
            .arg("Local Wikipedia Server")
            .status()?;

        if status.success() {
            println!("✅ Service 'wiki-serve' created.");
            let _ = Command::new("sc").arg("start").arg("wiki-serve").status();
            println!("✅ Service started.");
        } else {
            println!("❌ Failed to create service. Run as Administrator?");
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Create systemd unit
        let unit_content = format!(r#"[Unit]
Description=Local Wikipedia Server
After=network.target

[Service]
Type=simple
ExecStart={}/wiki-serve --data "{}" --port {}
Restart=on-failure
User={}

[Install]
WantedBy=multi-user.target
"#, 
            exe_dir.to_string_lossy(),
            data_dir.to_string_lossy(),
            port,
            std::env::var("USER").unwrap_or("root".to_string())
        );

        let unit_path = "/etc/systemd/system/wiki-serve.service";
        
        // We might need sudo. If we are not root, this will fail.
        // For now, just try to write.
        match fs::write(unit_path, unit_content) {
            Ok(_) => {
                println!("✅ Created {}", unit_path);
                Command::new("systemctl").arg("daemon-reload").status()?;
                Command::new("systemctl").arg("enable").arg("wiki-serve").status()?;
                Command::new("systemctl").arg("start").arg("wiki-serve").status()?;
                println!("✅ Service started");
            },
            Err(e) => {
                println!("❌ Failed to write service file: {}. (Need sudo?)", e);
                println!("   Run: sudo cp wiki-serve.service /etc/systemd/system/");
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Create LaunchAgent
        let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.wiki-download.serve</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}/wiki-serve</string>
        <string>--data</string>
        <string>{}</string>
        <string>--port</string>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/wiki-serve.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/wiki-serve.err</string>
</dict>
</plist>
"#,
            exe_dir.to_string_lossy(),
            data_dir.to_string_lossy(),
            port
        );

        let home = std::env::var("HOME").unwrap();
        let launch_agents = PathBuf::from(home).join("Library/LaunchAgents");
        fs::create_dir_all(&launch_agents)?;
        let plist_path = launch_agents.join("com.wiki-download.serve.plist");
        
        fs::write(&plist_path, plist_content)?;
        println!("✅ Created {:?}", plist_path);
        
        Command::new("launchctl").arg("load").arg(plist_path).status()?;
        println!("✅ Service loaded");
    }

    Ok(())
}

fn setup_auto_update(exe_dir: &Path, data_dir: &Path, lang: &str) -> Result<()> {
    println!("\n⏰ Setting up Auto-Update (Weekly)...");
    
    let bin_path = exe_dir.join(if cfg!(windows) { "wiki-download.exe" } else { "wiki-download" });
    let log_path = data_dir.join("update.log");
    
    #[cfg(target_os = "windows")]
    {
        // schtasks /create /tn "WikiUpdate" /tr "\"C:\Path\wiki-download.exe\" --lang en --output \"C:\Data\"" /sc weekly /d SUN /st 03:00
        let cmd = format!(
            "\\\"{}\\\" --lang {} --output \\\"{}\\\" > \\\"{}\\\" 2>&1", 
            bin_path.to_string_lossy(),
            lang,
            data_dir.to_string_lossy(),
            log_path.to_string_lossy()
        );
        
        let status = Command::new("schtasks")
            .arg("/create")
            .arg("/tn")
            .arg("WikiUpdate")
            .arg("/tr")
            .arg(cmd) // schtasks expects the command to be passed as one argument
            .arg("/sc")
            .arg("weekly")
            .arg("/d")
            .arg("SUN")
            .arg("/st")
            .arg("03:00")
            .arg("/f") // Force overwrite
            .status()?;
            
        if status.success() {
            println!("✅ Scheduled task 'WikiUpdate' created.");
        } else {
            println!("❌ Failed to create scheduled task.");
        }
    }

    #[cfg(unix)]
    {
        // (crontab -l 2>/dev/null; echo "0 3 * * 0 /path/to/wiki-download ...") | crontab -
        let cmd = format!(
            "0 3 * * 0 \"{}\" --lang {} --output \"{}\" >> \"{}\" 2>&1",
            bin_path.to_string_lossy(),
            lang,
            data_dir.to_string_lossy(),
            log_path.to_string_lossy()
        );
        
        // We need to be careful not to duplicate.
        // For simplicity, we'll just append.
        // A better way is to write a file to /etc/cron.d/ if root, or use crontab.
        
        // Let's try adding to current user's crontab
        let output = Command::new("crontab").arg("-l").output();
        let current_cron = if let Ok(out) = output {
            String::from_utf8_lossy(&out.stdout).to_string()
        } else {
            String::new()
        };
        
        if current_cron.contains("wiki-download") {
            println!("⚠️  Auto-update seems to be already configured in crontab.");
        } else {
            let new_cron = format!("{}\n{}\n", current_cron.trim(), cmd);
            
            let mut child = Command::new("crontab")
                .arg("-")
                .stdin(std::process::Stdio::piped())
                .spawn()?;
                
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(new_cron.as_bytes())?;
            }
            
            let status = child.wait()?;
            if status.success() {
                println!("✅ Added auto-update to crontab.");
            } else {
                println!("❌ Failed to update crontab.");
            }
        }
    }

    Ok(())
}
