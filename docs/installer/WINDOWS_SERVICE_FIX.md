# Windows Service Fix

## Problem
The Rustipedia Windows service was failing to start with the error:
```
The service did not respond to the start or control request in a timely fashion.
A timeout was reached (30000 milliseconds) while waiting for the Rustipedia Local Wikipedia Server service to connect.
```

## Root Cause
The `rustipedia-serve.exe` binary was being installed as a Windows service, but it was just a regular console application. It didn't implement the required Windows Service Control Manager (SCM) API calls that Windows expects from a service executable.

When Windows tried to start it as a service:
1. Windows would launch the exe and wait for it to call `StartServiceCtrlDispatcher()`
2. The exe would start normally (as a console app) and begin running the web server
3. Windows would wait 30 seconds for the service handshake that never came
4. Windows would timeout and kill the process

## Solution
The `rustipedia-serve.exe` now supports **both** Windows service mode and regular CLI mode:

### What was changed:
1. **Added `windows-service` dependency** in `Cargo.toml`
2. **Implemented proper Windows service support** in `serve.rs`:
   - Service entry point that responds to Windows Service Control Manager
   - Proper service status reporting (Starting, Running, Stopping, Stopped)
   - Graceful shutdown handling when service is stopped
   - Service control handler for Stop signals

3. **Automatic mode detection**:
   - When launched by Windows SCM → runs as service
   - When launched from command line → runs as normal CLI app
   - Both modes share the same server code

4. **Service-specific features**:
   - Logs to `C:\ProgramData\Rustipedia\server.log` instead of console
   - Handles service stop signals gracefully
   - Reports status to Windows throughout lifecycle

### Testing the fix:
1. Uninstall the old version (if installed)
2. Install the new MSI package from `target\wix\rustipedia-*.msi`
3. Open Services (`Win+R`, type `services.msc`)
4. Find "Rustipedia Local Wikipedia Server"
5. Start the service - it should now start successfully
6. Check the service status - should show "Running"
7. Open browser to `http://localhost:8080` (or configured port)
8. Stop the service - should stop gracefully

### Log locations:
- **Service mode**: `C:\ProgramData\Rustipedia\server.log`
- **Service errors**: `C:\ProgramData\Rustipedia\service_error.log`
- **CLI mode**: stdout/stderr as usual

### Manual testing (CLI mode):
The server still works perfectly when run manually:
```powershell
# Run as normal
rustipedia-serve

# With custom options
rustipedia-serve --data "C:\MyData" --port 3000
```

## Technical Details

The implementation uses the standard Windows service pattern:
1. `main()` tries to start as service via `service_dispatcher::start()`
2. If that fails (not running as service), falls back to CLI mode
3. Service mode:
   - Registers service control handler
   - Reports `StartPending` → `Running` → `StopPending` → `Stopped`
   - Runs server in background thread
   - Monitors shutdown channel
   - Triggers graceful shutdown on stop signal
4. Both modes use the same `run_server()` function with different logging configs

This is a zero-breaking-change fix - all existing functionality is preserved.
