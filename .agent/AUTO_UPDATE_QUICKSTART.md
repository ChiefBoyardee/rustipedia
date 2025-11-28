# Auto-Update Feature - Quick Start Guide

## What's Been Implemented

### Phase 1: Core Infrastructure ✅

We've completed the foundational components for the auto-update system:

#### 1. **Update Configuration Module** (`src/update_config.rs`)
- Comprehensive configuration structure for auto-updates
- Flexible scheduling options:
  - Daily updates at a specific time
  - Weekly updates on a specific day
  - Monthly updates on a specific day
  - Custom cron expressions (Unix only)
- Advanced features:
  - Update time windows (only update during specific hours)
  - Bandwidth limiting
  - Retry configuration
  - Notification settings
- Full validation and serialization support

#### 2. **Update Manager** (`src/update_manager.rs`)
- Core update orchestration logic
- Status tracking with detailed progress information
- Automatic retry on failure
- Logging and notification support
- Safe concurrent access using async RwLock
- Phases tracked:
  - Checking for updates
  - Downloading Wikipedia dump
  - Extracting articles
  - Building search index

#### 3. **Library Integration**
- Both modules exported from `lib.rs`
- Ready to be used by other binaries
- Comprehensive test coverage

## Current Capabilities

### Configuration
```rust
use rustipedia::{UpdateConfig, UpdateSchedule, Weekday};

// Create a weekly update schedule
let config = UpdateConfig {
    enabled: true,
    schedule: UpdateSchedule::Weekly {
        day: Weekday::Sunday,
        hour: 3,
        minute: 0,
    },
    language: "simple".to_string(),
    data_dir: PathBuf::from("wikipedia"),
    mode: UpdateMode::Full,
    max_bandwidth: 0, // unlimited
    update_window: Some(TimeWindow::new(2, 6)?), // Only update between 2 AM and 6 AM
    retry_config: RetryConfig {
        max_retries: 3,
        retry_delay_minutes: 30,
    },
    notifications: NotificationConfig {
        on_success: true,
        on_failure: true,
        log_file: PathBuf::from("update.log"),
    },
};

// Save configuration
config.save(PathBuf::from("wikipedia/update_config.json"))?;
```

### Update Management
```rust
use rustipedia::UpdateManager;

// Load the update manager
let manager = UpdateManager::load(&PathBuf::from("wikipedia"))?;

// Check if updates are needed
if manager.check_for_updates().await? {
    println!("Updates available!");
    
    // Perform the update
    manager.perform_update().await?;
}

// Get current status
let status = manager.get_status().await;
println!("Status: {:?}", status.current_status);
if let Some(progress) = status.progress {
    println!("Progress: {:.1}% - {}", progress.percent, progress.phase);
}
```

## What's Next

### Immediate Next Steps (Sprint 2)

1. **Create Update Daemon Binary** (`src/bin/update_daemon.rs`)
   - Standalone service that runs in the background
   - Monitors schedule and triggers updates
   - Respects update windows
   - Handles system sleep/wake events

2. **Improve Scheduler Integration** (Update `src/bin/setup.rs`)
   - Use the new UpdateConfig structure
   - Support all scheduling options
   - Better platform-specific integration:
     - Windows: Task Scheduler with advanced options
     - Linux: systemd timers instead of cron
     - macOS: LaunchDaemons with calendar intervals

3. **Add Manual Update Command**
   - New subcommand for rustipedia-download: `update`
   - Uses UpdateManager for consistent behavior
   - Respects update configuration

### Future Sprints

- **Sprint 3-4**: Web UI for settings management
- **Sprint 5**: Enhanced setup wizard
- **Sprint 6**: Polish, testing, and documentation

## Testing the Current Implementation

### Unit Tests
```bash
# Run all tests
cargo test

# Run update-specific tests
cargo test update_config
cargo test update_manager
```

### Manual Testing
```rust
// Example: Create and save a config
use rustipedia::UpdateConfig;
use std::path::PathBuf;

let config = UpdateConfig::default();
config.save(PathBuf::from("test_config.json"))?;

// Load it back
let loaded = UpdateConfig::load(PathBuf::from("test_config.json"))?;
println!("{:#?}", loaded);
```

## Configuration File Format

The update configuration is stored as JSON in `wikipedia/update_config.json`:

```json
{
  "enabled": true,
  "schedule": {
    "type": "Weekly",
    "day": "Sunday",
    "hour": 3,
    "minute": 0
  },
  "language": "simple",
  "data_dir": "wikipedia",
  "mode": "Full",
  "max_bandwidth": 0,
  "update_window": {
    "start_hour": 2,
    "end_hour": 6
  },
  "retry_config": {
    "max_retries": 3,
    "retry_delay_minutes": 30
  },
  "notifications": {
    "on_success": true,
    "on_failure": true,
    "log_file": "update.log"
  }
}
```

## Status Tracking

Update status is stored in `wikipedia/update_status.json`:

```json
{
  "last_check": "2025-11-27T11:00:00Z",
  "last_update": "2025-11-27T11:30:00Z",
  "last_success": "2025-11-27T11:30:00Z",
  "last_failure": null,
  "current_status": "Success",
  "progress": null,
  "error_message": null
}
```

During an update, the progress field contains:

```json
{
  "phase": "Downloading Wikipedia dump",
  "percent": 45.5,
  "bytes_downloaded": 1073741824,
  "total_bytes": 2147483648,
  "eta_seconds": 300
}
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Update System                            │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐      ┌──────────────┐                     │
│  │ UpdateConfig │      │ UpdateStatus │                     │
│  │              │      │              │                     │
│  │ - Schedule   │      │ - Last Check │                     │
│  │ - Language   │      │ - Progress   │                     │
│  │ - Mode       │      │ - Errors     │                     │
│  │ - Bandwidth  │      │              │                     │
│  └──────┬───────┘      └──────▲───────┘                     │
│         │                     │                              │
│         │                     │                              │
│         ▼                     │                              │
│  ┌─────────────────────────────────┐                        │
│  │      UpdateManager              │                        │
│  │                                 │                        │
│  │  - check_for_updates()          │                        │
│  │  - perform_update()             │                        │
│  │  - get_status()                 │                        │
│  │  - cancel_update()              │                        │
│  │  - retry_failed_update()        │                        │
│  └─────────────┬───────────────────┘                        │
│                │                                             │
│                ▼                                             │
│  ┌─────────────────────────────────┐                        │
│  │   rustipedia-download           │                        │
│  │   (Existing binary)             │                        │
│  └─────────────────────────────────┘                        │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

## Key Design Decisions

1. **Async-First**: Uses Tokio for async operations, allowing non-blocking updates
2. **Separation of Concerns**: Configuration, status, and execution logic are separate
3. **Platform Agnostic**: Core logic works on all platforms, with platform-specific scheduling
4. **Fail-Safe**: Comprehensive error handling and retry logic
5. **Observable**: Detailed status tracking and progress reporting
6. **Configurable**: Every aspect can be customized
7. **Testable**: Pure functions with comprehensive test coverage

## Benefits

### For Users
- Set it and forget it - Wikipedia stays up-to-date automatically
- Full control over when and how updates happen
- Bandwidth limiting to avoid network congestion
- Update windows to avoid peak usage times
- Detailed logging for troubleshooting

### For Developers
- Clean, modular architecture
- Easy to extend with new features
- Comprehensive error handling
- Well-documented code
- Type-safe configuration

## Known Limitations (To Be Addressed)

1. **No Incremental Updates**: Currently only supports full re-download
2. **No Web UI**: Configuration must be done via JSON files or setup wizard
3. **No Real-Time Monitoring**: Status must be polled, no WebSocket support yet
4. **No Update Daemon**: Relies on system schedulers (cron, Task Scheduler, etc.)
5. **Limited Notification Options**: Only file logging, no email or webhooks yet

## Contributing

To contribute to the auto-update feature:

1. Review the [implementation plan](AUTO_UPDATE_IMPLEMENTATION_PLAN.md)
2. Pick a task from the current sprint
3. Write tests first (TDD approach)
4. Implement the feature
5. Update documentation
6. Submit a pull request

## Questions?

- **Q: Will updates interrupt active users?**
  - A: No, updates run in the background. The web server continues serving the old content until the update completes.

- **Q: What happens if an update fails?**
  - A: The system will retry based on the retry configuration. The old content remains available.

- **Q: Can I update multiple languages?**
  - A: Not yet, but this is planned for a future release.

- **Q: How much disk space is needed?**
  - A: During updates, you need space for both the old and new content, plus the compressed dump file.

- **Q: Can I schedule updates at different times on different days?**
  - A: Not yet with the built-in scheduler, but you can use custom cron expressions on Unix systems.

## Changelog

### 2025-11-27 - Sprint 1 Complete ✅
- ✅ Created UpdateConfig structure with full validation
- ✅ Implemented UpdateManager with status tracking
- ✅ Added progress reporting
- ✅ Implemented retry logic
- ✅ Added comprehensive logging
- ✅ Integrated with library exports
- ✅ Added unit tests
- ✅ Documentation

### Next: Sprint 2 - Background Service
- ⏳ Create update daemon binary
- ⏳ Improve scheduler integration
- ⏳ Add manual update command
- ⏳ Cross-platform testing
