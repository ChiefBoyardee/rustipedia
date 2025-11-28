//! Auto-Update Configuration
//!
//! Configuration structures for the automatic Wikipedia update system.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Timelike};

/// Main auto-update configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Enable automatic updates
    pub enabled: bool,
    
    /// Update schedule
    pub schedule: UpdateSchedule,
    
    /// Language to update
    pub language: String,
    
    /// Data directory
    pub data_dir: PathBuf,
    
    /// Update mode (full or incremental)
    pub mode: UpdateMode,
    
    /// Maximum bandwidth in MB/s (0 = unlimited)
    pub max_bandwidth: u32,
    
    /// Update window (only update during these hours)
    pub update_window: Option<TimeWindow>,
    
    /// Retry settings
    pub retry_config: RetryConfig,
    
    /// Notification settings
    pub notifications: NotificationConfig,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schedule: UpdateSchedule::Weekly {
                day: Weekday::Sunday,
                hour: 3,
                minute: 0,
            },
            language: "simple".to_string(),
            data_dir: PathBuf::from("wikipedia"),
            mode: UpdateMode::Full,
            max_bandwidth: 0, // unlimited
            update_window: None,
            retry_config: RetryConfig::default(),
            notifications: NotificationConfig::default(),
        }
    }
}

impl UpdateConfig {
    /// Create a new update config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Load config from file
    pub fn load(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let content = std::fs::read_to_string(&path)?;
        let config: UpdateConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save config to file
    pub fn save(&self, path: impl Into<PathBuf>) -> anyhow::Result<()> {
        let path = path.into();
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the path to the update config file
    pub fn config_path(data_dir: &std::path::Path) -> PathBuf {
        data_dir.join("update_config.json")
    }

    /// Get the path to the update status file
    pub fn status_path(data_dir: &std::path::Path) -> PathBuf {
        data_dir.join("update_status.json")
    }

    /// Get the path to the update log file
    pub fn log_path(data_dir: &std::path::Path) -> PathBuf {
        data_dir.join("update.log")
    }

    /// Validate the configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate schedule
        self.schedule.validate()?;

        // Validate time window if present
        if let Some(ref window) = self.update_window {
            window.validate()?;
        }

        // Validate retry config
        self.retry_config.validate()?;

        Ok(())
    }
}

/// Update schedule options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UpdateSchedule {
    /// Daily at a specific time
    Daily { hour: u8, minute: u8 },
    
    /// Weekly on a specific day and time
    Weekly { day: Weekday, hour: u8, minute: u8 },
    
    /// Monthly on a specific day and time
    Monthly { day: u8, hour: u8, minute: u8 },
    
    /// Custom cron expression (Unix only)
    #[cfg(unix)]
    Custom { cron_expression: String },
}

impl UpdateSchedule {
    /// Validate the schedule
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            UpdateSchedule::Daily { hour, minute } => {
                if *hour > 23 {
                    anyhow::bail!("Hour must be between 0 and 23");
                }
                if *minute > 59 {
                    anyhow::bail!("Minute must be between 0 and 59");
                }
            }
            UpdateSchedule::Weekly { day: _, hour, minute } => {
                if *hour > 23 {
                    anyhow::bail!("Hour must be between 0 and 23");
                }
                if *minute > 59 {
                    anyhow::bail!("Minute must be between 0 and 59");
                }
            }
            UpdateSchedule::Monthly { day, hour, minute } => {
                if *day < 1 || *day > 31 {
                    anyhow::bail!("Day must be between 1 and 31");
                }
                if *hour > 23 {
                    anyhow::bail!("Hour must be between 0 and 23");
                }
                if *minute > 59 {
                    anyhow::bail!("Minute must be between 0 and 59");
                }
            }
            #[cfg(unix)]
            UpdateSchedule::Custom { cron_expression } => {
                // Basic validation - could use a cron parser library
                if cron_expression.is_empty() {
                    anyhow::bail!("Cron expression cannot be empty");
                }
            }
        }
        Ok(())
    }

    /// Convert to a human-readable string
    pub fn to_human_string(&self) -> String {
        match self {
            UpdateSchedule::Daily { hour, minute } => {
                format!("Daily at {:02}:{:02}", hour, minute)
            }
            UpdateSchedule::Weekly { day, hour, minute } => {
                format!("Weekly on {} at {:02}:{:02}", day.to_string(), hour, minute)
            }
            UpdateSchedule::Monthly { day, hour, minute } => {
                format!("Monthly on day {} at {:02}:{:02}", day, hour, minute)
            }
            #[cfg(unix)]
            UpdateSchedule::Custom { cron_expression } => {
                format!("Custom: {}", cron_expression)
            }
        }
    }
}

/// Days of the week
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Weekday {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl Weekday {
    /// Convert to string
    pub fn to_string(&self) -> &'static str {
        match self {
            Weekday::Sunday => "Sunday",
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
        }
    }

    /// Convert to Windows Task Scheduler day code
    #[cfg(windows)]
    pub fn to_windows_code(&self) -> &'static str {
        match self {
            Weekday::Sunday => "SUN",
            Weekday::Monday => "MON",
            Weekday::Tuesday => "TUE",
            Weekday::Wednesday => "WED",
            Weekday::Thursday => "THU",
            Weekday::Friday => "FRI",
            Weekday::Saturday => "SAT",
        }
    }

    /// Convert to cron day code (0-6, Sunday = 0)
    #[cfg(unix)]
    pub fn to_cron_code(&self) -> u8 {
        match self {
            Weekday::Sunday => 0,
            Weekday::Monday => 1,
            Weekday::Tuesday => 2,
            Weekday::Wednesday => 3,
            Weekday::Thursday => 4,
            Weekday::Friday => 5,
            Weekday::Saturday => 6,
        }
    }
}

/// Update mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UpdateMode {
    /// Full re-download and re-index
    Full,
    
    /// Incremental update (future feature)
    #[allow(dead_code)]
    Incremental,
}

/// Time window for updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    /// Start hour (0-23)
    pub start_hour: u8,
    
    /// End hour (0-23)
    pub end_hour: u8,
}

impl TimeWindow {
    /// Create a new time window
    pub fn new(start_hour: u8, end_hour: u8) -> anyhow::Result<Self> {
        let window = Self { start_hour, end_hour };
        window.validate()?;
        Ok(window)
    }

    /// Validate the time window
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.start_hour > 23 {
            anyhow::bail!("Start hour must be between 0 and 23");
        }
        if self.end_hour > 23 {
            anyhow::bail!("End hour must be between 0 and 23");
        }
        if self.start_hour == self.end_hour {
            anyhow::bail!("Start and end hours cannot be the same");
        }
        Ok(())
    }

    /// Check if the current time is within the window
    pub fn is_within_window(&self, now: &DateTime<Utc>) -> bool {
        let hour = now.hour() as u8;
        
        if self.start_hour < self.end_hour {
            // Normal case: e.g., 2:00 - 6:00
            hour >= self.start_hour && hour < self.end_hour
        } else {
            // Wraps around midnight: e.g., 22:00 - 4:00
            hour >= self.start_hour || hour < self.end_hour
        }
    }
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    
    /// Delay between retries in minutes
    pub retry_delay_minutes: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_minutes: 30,
        }
    }
}

impl RetryConfig {
    /// Validate the retry config
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.max_retries > 10 {
            anyhow::bail!("Maximum retries cannot exceed 10");
        }
        if self.retry_delay_minutes == 0 {
            anyhow::bail!("Retry delay must be at least 1 minute");
        }
        if self.retry_delay_minutes > 1440 {
            anyhow::bail!("Retry delay cannot exceed 24 hours (1440 minutes)");
        }
        Ok(())
    }
}

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Send notification on successful update
    pub on_success: bool,
    
    /// Send notification on failed update
    pub on_failure: bool,
    
    /// Path to log file
    pub log_file: PathBuf,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            on_success: true,
            on_failure: true,
            log_file: PathBuf::from("update.log"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_window_validation() {
        assert!(TimeWindow::new(2, 6).is_ok());
        assert!(TimeWindow::new(22, 4).is_ok());
        assert!(TimeWindow::new(5, 5).is_err());
        assert!(TimeWindow::new(25, 6).is_err());
    }

    #[test]
    fn test_time_window_check() {
        let window = TimeWindow::new(2, 6).unwrap();
        
        // Create test times
        let time_3am = Utc::now().date_naive().and_hms_opt(3, 0, 0).unwrap().and_utc();
        let time_7am = Utc::now().date_naive().and_hms_opt(7, 0, 0).unwrap().and_utc();
        
        assert!(window.is_within_window(&time_3am));
        assert!(!window.is_within_window(&time_7am));
    }

    #[test]
    fn test_schedule_validation() {
        let valid = UpdateSchedule::Daily { hour: 12, minute: 30 };
        assert!(valid.validate().is_ok());

        let invalid_hour = UpdateSchedule::Daily { hour: 25, minute: 30 };
        assert!(invalid_hour.validate().is_err());

        let invalid_minute = UpdateSchedule::Daily { hour: 12, minute: 70 };
        assert!(invalid_minute.validate().is_err());
    }

    #[test]
    fn test_retry_config_validation() {
        let valid = RetryConfig {
            max_retries: 3,
            retry_delay_minutes: 30,
        };
        assert!(valid.validate().is_ok());

        let too_many_retries = RetryConfig {
            max_retries: 15,
            retry_delay_minutes: 30,
        };
        assert!(too_many_retries.validate().is_err());

        let zero_delay = RetryConfig {
            max_retries: 3,
            retry_delay_minutes: 0,
        };
        assert!(zero_delay.validate().is_err());
    }
}
