# Auto-Update Feature Implementation Plan

## Overview
Implement a comprehensive automatic article update system that runs in the background to keep Wikipedia content up-to-date. This feature will be configurable during setup and through a settings management interface.

## Current State Analysis

### Existing Components
1. **Setup Wizard** (`src/bin/setup.rs`)
   - Already has basic `setup_auto_update()` function
   - Creates scheduled tasks (Windows) or cron jobs (Unix)
   - Runs weekly on Sundays at 3:00 AM
   - Limited to simple scheduling, no configuration options

2. **Config System** (`src/config.rs`)
   - Has `Config` struct for download settings
   - Has `ServerConfig` struct for server settings
   - No auto-update specific configuration

3. **Download Tool** (`src/bin/download.rs`)
   - Can download and extract Wikipedia dumps
   - No incremental update support
   - No update-specific logic

4. **Web Server** (`src/bin/serve.rs`)
   - No settings management interface
   - No update status monitoring

### Limitations
- No way to modify update schedule after initial setup
- No update status tracking or logging
- No incremental updates (always full re-download)
- No web UI for managing updates
- No notification system for update completion/failures
- No bandwidth throttling or update window configuration

## Implementation Plan

### Phase 1: Enhanced Configuration System

#### 1.1 Create Auto-Update Configuration Structure
**File**: `src/update_config.rs` (new)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Enable automatic updates
    pub enabled: bool,
    
    /// Update schedule (cron format for Unix, or custom for Windows)
    pub schedule: UpdateSchedule,
    
    /// Language to update
    pub language: String,
    
    /// Data directory
    pub data_dir: PathBuf,
    
    /// Update mode (full or incremental)
    pub mode: UpdateMode,
    
    /// Maximum bandwidth (MB/s, 0 = unlimited)
    pub max_bandwidth: u32,
    
    /// Update window (only update during these hours)
    pub update_window: Option<TimeWindow>,
    
    /// Retry settings
    pub retry_config: RetryConfig,
    
    /// Notification settings
    pub notifications: NotificationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateSchedule {
    Daily { hour: u8, minute: u8 },
    Weekly { day: Weekday, hour: u8, minute: u8 },
    Monthly { day: u8, hour: u8, minute: u8 },
    Custom { cron_expression: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateMode {
    Full,           // Full re-download
    Incremental,    // Only changed articles (future enhancement)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start_hour: u8,
    pub end_hour: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub retry_delay_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub on_success: bool,
    pub on_failure: bool,
    pub log_file: PathBuf,
}
```

#### 1.2 Extend Main Config
**File**: `src/config.rs`

Add auto-update config to the main config:
```rust
pub struct Config {
    // ... existing fields ...
    pub auto_update: Option<UpdateConfig>,
}
```

### Phase 2: Update Manager Service

#### 2.1 Create Update Manager
**File**: `src/update_manager.rs` (new)

Core update manager that handles:
- Checking for updates
- Downloading new dumps
- Extracting and indexing
- Logging and status tracking
- Error handling and retries

```rust
pub struct UpdateManager {
    config: UpdateConfig,
    status: Arc<RwLock<UpdateStatus>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub last_check: Option<DateTime<Utc>>,
    pub last_update: Option<DateTime<Utc>>,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub current_status: Status,
    pub progress: Option<UpdateProgress>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Status {
    Idle,
    Checking,
    Downloading,
    Extracting,
    Indexing,
    Failed,
    Success,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProgress {
    pub phase: String,
    pub percent: f32,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub eta_seconds: Option<u64>,
}
```

#### 2.2 Implement Update Logic
Key methods:
- `check_for_updates()` - Check if new dump is available
- `perform_update()` - Execute the update process
- `get_status()` - Get current update status
- `cancel_update()` - Cancel ongoing update
- `retry_failed_update()` - Retry a failed update

### Phase 3: Background Update Service

#### 3.1 Create Update Daemon/Service
**File**: `src/bin/update_daemon.rs` (new)

A standalone binary that:
- Runs in the background
- Monitors the update schedule
- Executes updates at scheduled times
- Respects update windows
- Handles system sleep/wake
- Logs all activities

#### 3.2 Improve Scheduler Integration
Enhance `setup_auto_update()` in `setup.rs`:
- Support flexible scheduling options
- Create proper service/daemon configuration
- Add update window constraints
- Implement bandwidth limiting

**Windows**: Use Task Scheduler with more advanced options
**Linux**: Use systemd timers instead of cron for better control
**macOS**: Use LaunchDaemons with calendar intervals

### Phase 4: Web UI for Settings Management

#### 4.1 Create Settings Page
**File**: `src/bin/serve.rs` - Add new routes

Add routes:
- `GET /settings` - Settings management page
- `GET /api/settings` - Get current settings (JSON)
- `POST /api/settings` - Update settings (JSON)
- `GET /api/update-status` - Get update status (JSON)
- `POST /api/update/trigger` - Manually trigger update
- `POST /api/update/cancel` - Cancel ongoing update
Allow users to:
- Choose specific days/times
- Set multiple update windows
- Configure retry behavior

### Phase 6: Logging and Monitoring

#### 6.1 Update Logging System
**File**: `src/update_logger.rs` (new)

Implement comprehensive logging:
- Structured logging with timestamps
- Separate log files for each update attempt
- Rotation of old logs
- Summary statistics
- Error categorization

#### 6.2 Update History
Store update history in database/JSON:
- Timestamp of each update
- Success/failure status
- Duration
- Articles added/modified/removed
- Errors encountered
- Dump file metadata

### Phase 7: Incremental Updates (Future Enhancement)

#### 7.1 Change Detection
Implement logic to:
- Compare dump file timestamps
- Download only changed articles
- Merge with existing database
- Update search index incrementally

#### 7.2 Differential Downloads
- Use Wikipedia's revision history API
# For scheduling
cron = "0.12"  # Cron expression parsing
chrono = { version = "0.4", features = ["serde"] }  # Already included

# For async background tasks
tokio = { version = "1.35", features = ["full"] }  # Already included

# For WebSocket (optional, for real-time updates)
tokio-tungstenite = "0.21"

# For bandwidth limiting
tokio-util = { version = "0.7", features = ["codec"] }
```

### Security Considerations
1. **Authentication**: Add basic auth for settings page
2. **CSRF Protection**: Add CSRF tokens for POST requests
3. **Input Validation**: Validate all user inputs
4. **File Permissions**: Ensure update daemon runs with appropriate permissions
5. **Rate Limiting**: Prevent abuse of manual update triggers

### Platform-Specific Challenges

#### Windows
- Task Scheduler permissions
- Service installation requires admin
- Handling system sleep/hibernate

#### Linux
- systemd vs cron choice
- Different distro variations
- Permission handling

#### macOS
- LaunchDaemon vs LaunchAgent
- System Integrity Protection
- Notarization requirements

## Testing Strategy

### Unit Tests
- UpdateConfig serialization/deserialization
- Schedule parsing and validation
- Update manager state transitions
- Retry logic

### Integration Tests
- Full update cycle
- Schedule triggering
- Settings persistence
- API endpoints

### Platform Tests
- Windows Task Scheduler integration
- Linux systemd timer integration
- macOS LaunchDaemon integration

### Manual Testing
- Install wizard flow
- Settings page functionality
- Update execution
- Error scenarios
- System restart handling

## Documentation Updates

### README.md
- Add auto-update feature description
- Document configuration options
- Show settings page screenshots

### User Guide
- How to enable/disable auto-updates
- How to change schedule
- How to monitor update status
- Troubleshooting guide

### Developer Guide
- Architecture overview
- Adding new update modes
- Extending notification system

## Success Metrics

1. **Functionality**
   - Updates execute on schedule
   - Settings persist across restarts
   - Web UI is responsive and intuitive
   - Works on all supported platforms

2. **Reliability**
   - Handles network failures gracefully
   - Retries failed updates
   - Doesn't interfere with server operation
   - Logs all activities

3. **User Experience**
   - Easy to configure during setup
   - Simple to modify settings later
   - Clear status visibility
   - Helpful error messages

## Future Enhancements

1. **Incremental Updates**: Only download changed articles
2. **Multiple Languages**: Update multiple wikis simultaneously
3. **Smart Scheduling**: Avoid peak usage times automatically
4. **Email Notifications**: Send email on update completion/failure
5. **Webhook Support**: Trigger external systems on updates
6. **Update Rollback**: Revert to previous version if update fails
7. **Bandwidth Scheduling**: Different limits for different times
8. **Update Channels**: Stable vs. latest dumps
9. **Peer-to-Peer Updates**: Share updates between local instances
10. **Mobile App**: Monitor and control updates from mobile device

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Update during active use | High | Implement update windows and usage detection |
| Disk space exhaustion | High | Check available space before update |
| Network failures | Medium | Implement robust retry logic |
| Corrupted downloads | Medium | Verify checksums, keep previous version |
| Schedule conflicts | Low | Validate schedule before saving |
| Permission issues | Medium | Clear error messages, documentation |

## Conclusion

This implementation plan provides a comprehensive roadmap for adding automatic update functionality to Rustipedia. The phased approach allows for incremental development and testing, while the modular design enables future enhancements without major refactoring.

The feature will significantly improve the user experience by keeping Wikipedia content fresh without manual intervention, while providing full control through an intuitive web interface.
