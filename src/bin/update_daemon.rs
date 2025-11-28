//! Update Daemon
//!
//! Background service that monitors the update schedule and triggers updates automatically.

use std::path::PathBuf;
use std::time::Duration;
use anyhow::{Result, Context};
use clap::Parser;
use chrono::Utc;
use tokio::time::sleep;

use rustipedia::{UpdateManager, UpdateConfig, Status};

#[derive(Parser)]
#[command(name = "rustipedia-update-daemon")]
#[command(author, version, about = "Background update service for Rustipedia")]
struct Cli {
    /// Data directory containing Wikipedia data
    #[arg(short, long, default_value = "wikipedia")]
    data_dir: PathBuf,

    /// Check interval in minutes
    #[arg(short, long, default_value = "60")]
    interval: u64,

    /// Run once and exit (for testing)
    #[arg(long)]
    once: bool,

    /// Force update immediately (ignore schedule)
    #[arg(long)]
    force: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    tracing::info!("Rustipedia Update Daemon starting...");
    tracing::info!("Data directory: {}", cli.data_dir.display());

    // Load the update manager
    let manager = UpdateManager::load(&cli.data_dir)
        .context("Failed to load update manager")?;

    // Get the config to check if updates are enabled
    let config_path = UpdateConfig::config_path(&cli.data_dir);
    let config = if config_path.exists() {
        UpdateConfig::load(&config_path)?
    } else {
        tracing::warn!("No update configuration found. Auto-updates are disabled.");
        tracing::info!("Run 'rustipedia-setup' to configure auto-updates.");
        return Ok(());
    };

    if !config.enabled && !cli.force {
        tracing::info!("Auto-updates are disabled in configuration.");
        return Ok(());
    }

    if cli.force {
        tracing::info!("Force update requested, ignoring schedule...");
        perform_update(&manager).await?;
        return Ok(());
    }

    if cli.once {
        tracing::info!("Running in single-check mode...");
        check_and_update(&manager, &config).await?;
        return Ok(());
    }

    // Main daemon loop
    tracing::info!("Entering daemon mode. Checking every {} minutes.", cli.interval);
    tracing::info!("Update schedule: {}", config.schedule.to_human_string());

    loop {
        match check_and_update(&manager, &config).await {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Error during update check: {}", e);
            }
        }

        // Sleep for the configured interval
        sleep(Duration::from_secs(cli.interval * 60)).await;
    }
}

/// Check if an update should run and execute it if needed
async fn check_and_update(manager: &UpdateManager, config: &UpdateConfig) -> Result<()> {
    // Check current status
    let status = manager.get_status().await;

    // Don't start a new update if one is already running
    match status.current_status {
        Status::Downloading | Status::Extracting | Status::Indexing | Status::Checking => {
            tracing::info!("Update already in progress: {}", status.current_status.to_string());
            return Ok(());
        }
        _ => {}
    }

    // Check if we're within the update window (if configured)
    if let Some(ref window) = config.update_window {
        let now = Utc::now();
        if !window.is_within_window(&now) {
            tracing::debug!("Current time is outside update window");
            return Ok(());
        }
    }

    // Check if it's time to update based on the schedule
    if !should_update_now(config, &status) {
        tracing::debug!("Not time to update yet");
        return Ok(());
    }

    tracing::info!("Update scheduled, checking for updates...");

    // Check if updates are available
    match manager.check_for_updates().await {
        Ok(true) => {
            tracing::info!("Updates available, starting update process...");
            perform_update(manager).await?;
        }
        Ok(false) => {
            tracing::info!("No updates needed");
        }
        Err(e) => {
            tracing::error!("Failed to check for updates: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

/// Perform the actual update
async fn perform_update(manager: &UpdateManager) -> Result<()> {
    tracing::info!("Starting Wikipedia update...");

    match manager.perform_update().await {
        Ok(_) => {
            tracing::info!("✅ Update completed successfully!");
            Ok(())
        }
        Err(e) => {
            tracing::error!("❌ Update failed: {}", e);
            
            // Get the config to check retry settings
            // For now, we'll just return the error
            // In the future, we could implement automatic retries here
            Err(e)
        }
    }
}

/// Determine if an update should run now based on the schedule
fn should_update_now(config: &UpdateConfig, status: &rustipedia::UpdateStatus) -> bool {
    use rustipedia::UpdateSchedule;
    use chrono::Timelike;

    let now = Utc::now();
    let current_hour = now.hour() as u8;
    let current_minute = now.minute() as u8;

    // Check if we've already updated recently
    if let Some(last_success) = status.last_success {
        let hours_since_update = (now - last_success).num_hours();
        
        // Don't update more than once per day
        if hours_since_update < 23 {
            return false;
        }
    }

    // Check the schedule
    match &config.schedule {
        UpdateSchedule::Daily { hour, minute } => {
            // Update if we're within 5 minutes of the scheduled time
            current_hour == *hour && current_minute >= *minute && current_minute < minute + 5
        }
        UpdateSchedule::Weekly { day, hour, minute } => {
            use chrono::Datelike;
            let current_day = now.weekday();
            
            // Convert our Weekday to chrono's Weekday
            let scheduled_day = match day {
                rustipedia::Weekday::Sunday => chrono::Weekday::Sun,
                rustipedia::Weekday::Monday => chrono::Weekday::Mon,
                rustipedia::Weekday::Tuesday => chrono::Weekday::Tue,
                rustipedia::Weekday::Wednesday => chrono::Weekday::Wed,
                rustipedia::Weekday::Thursday => chrono::Weekday::Thu,
                rustipedia::Weekday::Friday => chrono::Weekday::Fri,
                rustipedia::Weekday::Saturday => chrono::Weekday::Sat,
            };

            current_day == scheduled_day 
                && current_hour == *hour 
                && current_minute >= *minute 
                && current_minute < minute + 5
        }
        UpdateSchedule::Monthly { day, hour, minute } => {
            use chrono::Datelike;
            let current_day = now.day() as u8;
            
            current_day == *day 
                && current_hour == *hour 
                && current_minute >= *minute 
                && current_minute < minute + 5
        }
        #[cfg(unix)]
        UpdateSchedule::Custom { cron_expression: _ } => {
            // TODO: Implement cron expression parsing
            // For now, just update once per day
            current_hour == 3 && current_minute < 5
        }
    }
}
