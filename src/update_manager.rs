//! Update Manager
//!
//! Manages automatic Wikipedia updates, including scheduling, execution, and status tracking.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use anyhow::{Result, Context};
use std::process::Command;

use crate::update_config::{UpdateConfig, UpdateMode};

/// Update manager handles the update process
pub struct UpdateManager {
    config: UpdateConfig,
    status: Arc<RwLock<UpdateStatus>>,
}

impl UpdateManager {
    /// Create a new update manager
    pub fn new(config: UpdateConfig) -> Self {
        let status = Arc::new(RwLock::new(UpdateStatus::default()));
        Self { config, status }
    }

    /// Load update manager from config file
    pub fn load(data_dir: &PathBuf) -> Result<Self> {
        let config_path = UpdateConfig::config_path(data_dir);
        let config = if config_path.exists() {
            UpdateConfig::load(&config_path)?
        } else {
            UpdateConfig::default()
        };

        let manager = Self::new(config);
        
        // Try to load existing status
        let status_path = UpdateConfig::status_path(data_dir);
        if status_path.exists() {
            if let Ok(status) = UpdateStatus::load(&status_path) {
                *manager.status.blocking_write() = status;
            }
        }

        Ok(manager)
    }

    /// Save the current configuration
    pub fn save_config(&self) -> Result<()> {
        let config_path = UpdateConfig::config_path(&self.config.data_dir);
        self.config.save(&config_path)
    }

    /// Get the current status
    pub async fn get_status(&self) -> UpdateStatus {
        let mut current = self.status.write().await;
        
        // If we are idle, check if another process is doing something
        if current.current_status == Status::Idle {
            let status_path = UpdateConfig::status_path(&self.config.data_dir);
            if status_path.exists() {
                if let Ok(disk_status) = UpdateStatus::load(&status_path) {
                    *current = disk_status;
                }
            }
        }
        
        current.clone()
    }

    /// Check if an update is needed
    pub async fn check_for_updates(&self) -> Result<bool> {
        // Update status
        {
            let mut status = self.status.write().await;
            status.last_check = Some(Utc::now());
            status.current_status = Status::Checking;
        }

        // For now, we'll just check if enough time has passed since last update
        // In the future, we could check Wikipedia's dump metadata
        let status = self.status.read().await;
        
        let needs_update = if let Some(last_update) = status.last_success {
            let days_since_update = (Utc::now() - last_update).num_days();
            days_since_update >= 7 // Update if it's been more than a week
        } else {
            true // Never updated, so update is needed
        };

        // Update status back to idle
        {
            let mut status = self.status.write().await;
            status.current_status = Status::Idle;
        }

        self.save_status().await?;
        Ok(needs_update)
    }

    /// Perform the update
    pub async fn perform_update(&self) -> Result<()> {
        tracing::info!("Starting Wikipedia update");

        // Check if update is already in progress
        {
            let status = self.status.read().await;
            match status.current_status {
                Status::Downloading | Status::Extracting | Status::Indexing => {
                    anyhow::bail!("Update already in progress");
                }
                _ => {}
            }
        }

        // Check if we're within the update window
        if let Some(ref window) = self.config.update_window {
            if !window.is_within_window(&Utc::now()) {
                anyhow::bail!("Current time is outside the configured update window");
            }
        }

        // Update status
        {
            let mut status = self.status.write().await;
            status.current_status = Status::Downloading;
            status.progress = Some(UpdateProgress {
                phase: "Initializing".to_string(),
                percent: 0.0,
                bytes_downloaded: 0,
                total_bytes: None,
                eta_seconds: None,
            });
        }
        self.save_status().await?;

        // Perform the actual update based on mode
        let result = match self.config.mode {
            UpdateMode::Full => self.perform_full_update().await,
            UpdateMode::Incremental => {
                anyhow::bail!("Incremental updates not yet implemented")
            }
        };

        // Update final status
        {
            let mut status = self.status.write().await;
            match result {
                Ok(_) => {
                    status.current_status = Status::Success;
                    status.last_success = Some(Utc::now());
                    status.last_update = Some(Utc::now());
                    status.error_message = None;
                    
                    tracing::info!("Wikipedia update completed successfully");
                    
                    // Log success if configured
                    if self.config.notifications.on_success {
                        self.log_update_result(true, None).await?;
                    }
                }
                Err(ref e) => {
                    status.current_status = Status::Failed;
                    status.last_failure = Some(Utc::now());
                    status.error_message = Some(e.to_string());
                    
                    tracing::error!("Wikipedia update failed: {}", e);
                    
                    // Log failure if configured
                    if self.config.notifications.on_failure {
                        self.log_update_result(false, Some(e.to_string())).await?;
                    }
                }
            }
            status.progress = None;
        }

        self.save_status().await?;
        result
    }

    /// Perform a full update (re-download and re-index)
    async fn perform_full_update(&self) -> Result<()> {
        // Find the rustipedia-download executable
        let exe_name = if cfg!(windows) {
            "rustipedia-download.exe"
        } else {
            "rustipedia-download"
        };

        // Try to find it in the same directory as the current executable
        let exe_dir = std::env::current_exe()?
            .parent()
            .context("Failed to get executable directory")?
            .to_path_buf();
        
        let download_exe = exe_dir.join(exe_name);

        if !download_exe.exists() {
            anyhow::bail!("Could not find {} in {}", exe_name, exe_dir.display());
        }

        // Build the command
        let mut cmd = Command::new(&download_exe);
        cmd.arg("--lang").arg(&self.config.language);
        cmd.arg("--output").arg(&self.config.data_dir);
        cmd.arg("--skip-download"); // Skip if already downloaded
        
        // Update status
        {
            let mut status = self.status.write().await;
            status.progress = Some(UpdateProgress {
                phase: "Downloading Wikipedia dump".to_string(),
                percent: 10.0,
                bytes_downloaded: 0,
                total_bytes: None,
                eta_seconds: None,
            });
        }
        self.save_status().await?;

        // Execute the download command
        tracing::info!("Executing: {:?}", cmd);
        let output = cmd.output()
            .context("Failed to execute rustipedia-download")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Download failed: {}", stderr);
        }

        // Update status - extraction
        {
            let mut status = self.status.write().await;
            status.current_status = Status::Extracting;
            status.progress = Some(UpdateProgress {
                phase: "Extracting articles".to_string(),
                percent: 50.0,
                bytes_downloaded: 0,
                total_bytes: None,
                eta_seconds: None,
            });
        }
        self.save_status().await?;

        // Update status - indexing
        {
            let mut status = self.status.write().await;
            status.current_status = Status::Indexing;
            status.progress = Some(UpdateProgress {
                phase: "Building search index".to_string(),
                percent: 80.0,
                bytes_downloaded: 0,
                total_bytes: None,
                eta_seconds: None,
            });
        }
        self.save_status().await?;

        Ok(())
    }

    /// Retry a failed update
    pub async fn retry_failed_update(&self) -> Result<()> {
        let status = self.status.read().await;
        
        if status.current_status != Status::Failed {
            anyhow::bail!("No failed update to retry");
        }

        drop(status); // Release the lock before calling perform_update
        self.perform_update().await
    }

    /// Cancel an ongoing update
    pub async fn cancel_update(&self) -> Result<()> {
        let mut status = self.status.write().await;
        
        match status.current_status {
            Status::Downloading | Status::Extracting | Status::Indexing | Status::Checking => {
                status.current_status = Status::Idle;
                status.progress = None;
                status.error_message = Some("Update cancelled by user".to_string());
                Ok(())
            }
            _ => {
                anyhow::bail!("No update in progress to cancel")
            }
        }
    }

    /// Save the current status to disk
    async fn save_status(&self) -> Result<()> {
        let status = self.status.read().await;
        let status_path = UpdateConfig::status_path(&self.config.data_dir);
        status.save(&status_path)
    }

    /// Log update result
    async fn log_update_result(&self, success: bool, error: Option<String>) -> Result<()> {
        let log_path = &self.config.notifications.log_file;
        
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        let result = if success { "SUCCESS" } else { "FAILED" };
        let message = if let Some(err) = error {
            format!("[{}] Update {}: {}\n", timestamp, result, err)
        } else {
            format!("[{}] Update {}\n", timestamp, result)
        };

        // Append to log file
        use std::fs::OpenOptions;
        use std::io::Write;
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        
        file.write_all(message.as_bytes())?;
        Ok(())
    }

    /// Get update history (last N lines of log)
    pub async fn get_history(&self, lines: usize) -> Result<Vec<String>> {
        let log_path = &self.config.notifications.log_file;
        
        if !log_path.exists() {
            return Ok(Vec::new());
        }

        // Simple implementation: read whole file and take last N lines.
        let content = tokio::fs::read_to_string(log_path).await?;
        let log_lines: Vec<String> = content
            .lines()
            .rev()
            .take(lines)
            .map(String::from)
            .collect();
            
        Ok(log_lines)
    }
}

/// Current update status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatus {
    /// Last time we checked for updates
    pub last_check: Option<DateTime<Utc>>,
    
    /// Last time an update was attempted
    pub last_update: Option<DateTime<Utc>>,
    
    /// Last successful update
    pub last_success: Option<DateTime<Utc>>,
    
    /// Last failed update
    pub last_failure: Option<DateTime<Utc>>,
    
    /// Current status
    pub current_status: Status,
    
    /// Current progress (if updating)
    pub progress: Option<UpdateProgress>,
    
    /// Error message (if failed)
    pub error_message: Option<String>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            last_check: None,
            last_update: None,
            last_success: None,
            last_failure: None,
            current_status: Status::Idle,
            progress: None,
            error_message: None,
        }
    }
}

impl UpdateStatus {
    /// Load status from file
    pub fn load(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let status: UpdateStatus = serde_json::from_str(&content)?;
        Ok(status)
    }

    /// Save status to file
    pub fn save(&self, path: &PathBuf) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// Update status enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Status {
    /// Not doing anything
    Idle,
    
    /// Checking for updates
    Checking,
    
    /// Downloading Wikipedia dump
    Downloading,
    
    /// Extracting articles
    Extracting,
    
    /// Building search index
    Indexing,
    
    /// Update failed
    Failed,
    
    /// Update succeeded
    Success,
}

impl Status {
    /// Convert to human-readable string
    pub fn to_string(&self) -> &'static str {
        match self {
            Status::Idle => "Idle",
            Status::Checking => "Checking for updates",
            Status::Downloading => "Downloading",
            Status::Extracting => "Extracting",
            Status::Indexing => "Indexing",
            Status::Failed => "Failed",
            Status::Success => "Success",
        }
    }
}

/// Update progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProgress {
    /// Current phase description
    pub phase: String,
    
    /// Progress percentage (0-100)
    pub percent: f32,
    
    /// Bytes downloaded so far
    pub bytes_downloaded: u64,
    
    /// Total bytes to download (if known)
    pub total_bytes: Option<u64>,
    
    /// Estimated time remaining in seconds (if known)
    pub eta_seconds: Option<u64>,
}

impl UpdateProgress {
    /// Format bytes as human-readable string
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }

    /// Format ETA as human-readable string
    pub fn format_eta(&self) -> String {
        if let Some(eta) = self.eta_seconds {
            let hours = eta / 3600;
            let minutes = (eta % 3600) / 60;
            let seconds = eta % 60;

            if hours > 0 {
                format!("{}h {}m", hours, minutes)
            } else if minutes > 0 {
                format!("{}m {}s", minutes, seconds)
            } else {
                format!("{}s", seconds)
            }
        } else {
            "Unknown".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_serialization() {
        let status = UpdateStatus {
            last_check: Some(Utc::now()),
            last_update: None,
            last_success: None,
            last_failure: None,
            current_status: Status::Idle,
            progress: None,
            error_message: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: UpdateStatus = serde_json::from_str(&json).unwrap();
        
        assert_eq!(status.current_status, deserialized.current_status);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(UpdateProgress::format_bytes(500), "500 bytes");
        assert_eq!(UpdateProgress::format_bytes(1024), "1.00 KB");
        assert_eq!(UpdateProgress::format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(UpdateProgress::format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }
}
