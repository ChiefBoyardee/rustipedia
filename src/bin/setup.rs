//! Rustipedia Setup Tool
//!
//! Interactive wizard for configuring and installing the Rustipedia Server.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

use anyhow::{Result, Context};
use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Select, Input, Confirm};
use console::style;
use rustipedia::{UpdateConfig, UpdateSchedule, Weekday};

#[derive(Parser)]
#[command(name = "rustipedia-setup")]
#[command(author, version, about = "Setup wizard for Rustipedia Server")]
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

#[cfg(windows)]
fn ensure_admin() -> Result<()> {
    // Check if running as admin by trying to execute a command that requires it
    let status = Command::new("net")
        .arg("session")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if let Ok(status) = status {
        if status.success() {
            return Ok(()); // Already admin
        }
    }

    // Not admin, relaunch
    println!("Requesting Administrator privileges...");
    
    let exe = std::env::current_exe()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    
    let mut cmd = Command::new("powershell");
    cmd.arg("-Command");
    cmd.arg("Start-Process");
    cmd.arg("-FilePath");
    cmd.arg(format!("'{}'", exe.to_string_lossy()));
    
    if !args.is_empty() {
        // Escape arguments properly? For now simple joining.
        // Ideally we should escape quotes.
        let args_str = args.join(" ");
        cmd.arg("-ArgumentList");
        cmd.arg(format!("'{}'", args_str));
    }
    
    cmd.arg("-Verb");
    cmd.arg("RunAs");
    cmd.arg("-Wait"); 
    
    let status = cmd.status()?;
    
    if status.success() {
        std::process::exit(0);
    } else {
        anyhow::bail!("Failed to elevate privileges. Please run as Administrator.");
    }
}

fn main() -> Result<()> {
    #[cfg(windows)]
    ensure_admin()?;

    // Enable ANSI support on Windows
    // #[cfg(windows)]
    // let _ = console::enable_ansi_support();

    println!();
    println!("{}", style("╔══════════════════════════════════════════════════════════════════╗").cyan());
    println!("{}", style("║                     🛠️  RUSTIPEDIA SETUP                          ║").cyan());
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
        .with_prompt("Enable auto-updates?")
        .default(false)
        .interact()?;

    let mut update_schedule = UpdateSchedule::Weekly { 
        day: Weekday::Sunday, 
        hour: 3, 
        minute: 0 
    };

    if auto_update {
        let frequencies = vec!["Daily", "Weekly", "Monthly"];
        let freq_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Update Frequency")
            .default(1) // Weekly
            .items(&frequencies)
            .interact()?;

        update_schedule = match freq_idx {
            0 => { // Daily
                let hour: u8 = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Hour (0-23)")
                    .default(3)
                    .interact_text()?;
                UpdateSchedule::Daily { hour, minute: 0 }
            },
            1 => { // Weekly
                let days = vec!["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
                let day_idx = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Day of Week")
                    .default(0)
                    .items(&days)
                    .interact()?;
                
                let day = match day_idx {
                    0 => Weekday::Sunday,
                    1 => Weekday::Monday,
                    2 => Weekday::Tuesday,
                    3 => Weekday::Wednesday,
                    4 => Weekday::Thursday,
                    5 => Weekday::Friday,
                    6 => Weekday::Saturday,
                    _ => unreachable!(),
                };

                let hour: u8 = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Hour (0-23)")
                    .default(3)
                    .interact_text()?;
                
                UpdateSchedule::Weekly { day, hour, minute: 0 }
            },
            2 => { // Monthly
                let day: u8 = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Day of Month (1-28)")
                    .default(1)
                    .interact_text()?;
                
                let hour: u8 = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Hour (0-23)")
                    .default(3)
                    .interact_text()?;
                
                UpdateSchedule::Monthly { day, hour, minute: 0 }
            },
            _ => unreachable!(),
        };
    }

    let mut max_bandwidth = 0;
    let mut retry_count = 3;

    if auto_update {
        let advanced_update = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Configure advanced update settings? (Bandwidth, Retries)")
            .default(false)
            .interact()?;

        if advanced_update {
            max_bandwidth = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Max Bandwidth (MB/s, 0 for unlimited)")
                .default(0)
                .interact_text()?;
                
            retry_count = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Max Retries")
                .default(3)
                .interact_text()?;
        }
    }

    println!("\n{}", style("Configuration Summary:").bold());
    println!("  Language: {}", style(&lang_code).green());
    println!("  Data Dir: {}", style(data_dir.display()).green());
    println!("  Port:     {}", style(port).green());
    println!("  Prune:    {}", style(if prune { "Yes" } else { "No" }).green());
    println!("  Service:  {}", style(if install_service { "Yes" } else { "No" }).green());
    println!("  Updates:  {}", style(if auto_update { "Yes" } else { "No" }).green());
    if auto_update {
        println!("  Schedule: {}", style(update_schedule.to_human_string()).green());
    }
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
    // We invoke the rustipedia-download binary. 
    // Assuming it's in the same directory as this executable or in PATH.
    let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
    let downloader_exe = if cfg!(windows) { "rustipedia-download.exe" } else { "rustipedia-download" };
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
            println!("\n⚠️  Could not find or run rustipedia-download: {}", e);
            println!("   Please run it manually: rustipedia-download --lang {} --output {:?}", lang_code, data_dir);
        }
    }

    // 4. Install Service
    if install_service {
        install_system_service(&exe_dir, &data_dir, port)?;
    }

    // 5. Setup Auto-Update
    if auto_update {
        setup_auto_update(&exe_dir, &data_dir, &lang_code, update_schedule, max_bandwidth, retry_count)?;
    }

    println!("\n{}", style("🎉 Setup Complete!").bold().green());
    if install_service {
        println!("Service should be running on http://localhost:{}", port);
    } else {
        println!("Run the server manually:");
        println!("  rustipedia-serve --data {:?} --port {}", data_dir, port);
    }

    Ok(())
}

fn install_system_service(exe_dir: &Path, data_dir: &Path, port: u16) -> Result<()> {
    println!("\n🛠️  Installing Service...");

    #[cfg(target_os = "windows")]
    {
        // Use sc.exe
        // sc create rustipedia-serve binPath= "C:\Path\rustipedia-serve.exe --data C:\Data --port 3000" start= auto
        let bin_path = exe_dir.join("rustipedia-serve.exe");
        let cmd = format!(
            "\"{}\" --data \"{}\" --port {}", 
            bin_path.to_string_lossy(), 
            data_dir.to_string_lossy(), 
            port
        );
        
        let status = Command::new("sc")
            .arg("create")
            .arg("rustipedia-serve")
            .arg("binPath=")
            .arg(&cmd) 
            .arg("start=")
            .arg("auto")
            .arg("DisplayName=")
            .arg("Rustipedia Local Wikipedia Server")
            .status()?;

        if status.success() {
            println!("✅ Service 'rustipedia-serve' created.");
            let _ = Command::new("sc").arg("start").arg("rustipedia-serve").status();
            println!("✅ Service started.");
        } else {
            println!("⚠️  Service creation failed (might already exist). Trying to update configuration...");
            // Try sc config
            let status_config = Command::new("sc")
                .arg("config")
                .arg("rustipedia-serve")
                .arg("binPath=")
                .arg(&cmd)
                .arg("start=")
                .arg("auto")
                .arg("DisplayName=")
                .arg("Rustipedia Local Wikipedia Server")
                .status()?;
             
             if status_config.success() {
                 println!("✅ Service configuration updated.");
                 let _ = Command::new("sc").arg("start").arg("rustipedia-serve").status();
                 println!("✅ Service started.");
             } else {
                 println!("❌ Failed to configure service. Run as Administrator?");
             }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Create systemd unit
        let unit_content = format!(r#"[Unit]
Description=Rustipedia Local Wikipedia Server
After=network.target

[Service]
Type=simple
ExecStart={}/rustipedia-serve --data "{}" --port {}
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

        let unit_path = "/etc/systemd/system/rustipedia-serve.service";
        
        match fs::write(unit_path, unit_content) {
            Ok(_) => {
                println!("✅ Created {}", unit_path);
                Command::new("systemctl").arg("daemon-reload").status()?;
                Command::new("systemctl").arg("enable").arg("rustipedia-serve").status()?;
                Command::new("systemctl").arg("start").arg("rustipedia-serve").status()?;
                println!("✅ Service started");
            },
            Err(e) => {
                println!("❌ Failed to write service file: {}. (Need sudo?)", e);
                println!("   Run: sudo cp rustipedia-serve.service /etc/systemd/system/");
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
    <string>com.rustipedia.serve</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}/rustipedia-serve</string>
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
    <string>/tmp/rustipedia-serve.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/rustipedia-serve.err</string>
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
        let plist_path = launch_agents.join("com.rustipedia.serve.plist");
        
        fs::write(&plist_path, plist_content)?;
        println!("✅ Created {:?}", plist_path);
        
        Command::new("launchctl").arg("load").arg(plist_path).status()?;
        println!("✅ Service loaded");
    }

    Ok(())
}

fn setup_auto_update(
    exe_dir: &Path, 
    data_dir: &Path, 
    lang: &str, 
    schedule: UpdateSchedule,
    max_bandwidth: u32,
    retry_count: u32
) -> Result<()> {
    println!("\n⏰ Setting up Auto-Update...");
    
    // 1. Create and save update config
    let mut config = UpdateConfig::default();
    config.enabled = true;
    config.schedule = schedule;
    config.language = lang.to_string();
    config.data_dir = data_dir.to_path_buf();
    config.max_bandwidth = max_bandwidth;
    config.retry_config.max_retries = retry_count;
    
    config.save(UpdateConfig::config_path(data_dir))?;
    println!("✅ Update configuration saved.");

    // 2. Install Daemon Service/Task
    // The daemon should run frequently (e.g., every hour) to check if it's time to update
    
    let bin_path = exe_dir.join(if cfg!(windows) { "rustipedia-update-daemon.exe" } else { "rustipedia-update-daemon" });
    
    #[cfg(target_os = "windows")]
    {
        // Create a scheduled task that runs every hour
        let cmd = format!(
            "\\\"{}\\\" --data \\\"{}\\\" --interval 60", 
            bin_path.to_string_lossy(),
            data_dir.to_string_lossy()
        );
        
        let status = Command::new("schtasks")
            .arg("/create")
            .arg("/tn")
            .arg("RustipediaUpdateDaemon")
            .arg("/tr")
            .arg(cmd)
            .arg("/sc")
            .arg("HOURLY") // Check every hour
            .arg("/mo")
            .arg("1")
            .arg("/f") // Force overwrite
            .status()?;
            
        if status.success() {
            println!("✅ Scheduled task 'RustipediaUpdateDaemon' created (runs hourly).");
        } else {
            println!("❌ Failed to create scheduled task.");
        }
    }

    #[cfg(unix)]
    {
        use std::io::Write;
        // Add to crontab to run hourly
        // 0 * * * * /path/to/rustipedia-update-daemon --data /path/to/data --once
        let cmd = format!(
            "0 * * * * \"{}\" --data \"{}\" --once >> \"{}/update_daemon.log\" 2>&1",
            bin_path.to_string_lossy(),
            data_dir.to_string_lossy(),
            data_dir.to_string_lossy()
        );
        
        let output = Command::new("crontab").arg("-l").output();
        let current_cron = if let Ok(out) = output {
            String::from_utf8_lossy(&out.stdout).to_string()
        } else {
            String::new()
        };
        
        if current_cron.contains("rustipedia-update-daemon") {
            println!("⚠️  Auto-update daemon seems to be already configured in crontab.");
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
                println!("✅ Added auto-update daemon to crontab (runs hourly).");
            } else {
                println!("❌ Failed to update crontab.");
            }
        }
    }

    Ok(())
}
