# Rustipedia Installer Testing Guide

## Overview
This guide will help you test that the new Windows installer (v0.2.2) properly preserves Wikipedia data when upgrading from the previous version (v0.2.1).

## Test Scenario
We'll simulate a real-world upgrade:
1. Create test data in the new persistent location
2. Run the v0.2.2 installer (upgrade)
3. Verify data was preserved
4. Verify service works correctly

---

## Step-by-Step Testing

### Step 1: Prepare Test Environment

**Open PowerShell as Administrator** (required for service management and installation)

```powershell
# Navigate to project directory
cd G:\Projects\wiki_download

# Check current installation
Get-Service rustipedia-serve

# Check for existing data
dir "C:\ProgramData\Rustipedia\wikipedia" -ErrorAction SilentlyContinue
```

### Step 2: Create Test Data

This creates a marker file so we can verify it survives the upgrade:

```powershell
# Create test data directory if it doesn't exist
$testDir = "C:\ProgramData\Rustipedia\wikipedia"
if (-not (Test-Path $testDir)) {
    New-Item -Path $testDir -ItemType Directory -Force
}

# Create a test marker file with timestamp
$markerFile = Join-Path $testDir "UPGRADE_TEST_MARKER.txt"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
@"
==============================================
Rustipedia Data Preservation Test
==============================================
Created: $timestamp

If you can read this after upgrading,
then data preservation is working! ✓

Test conducted by: $env:USERNAME
Machine: $env:COMPUTERNAME
==============================================
"@ | Set-Content $markerFile

Write-Host "✓ Test marker created: $markerFile" -ForegroundColor Green
Write-Host "  Timestamp: $timestamp" -ForegroundColor Cyan
```

### Step 3: Record Current State

```powershell
# Count files in data directory (if any exist)
if (Test-Path "C:\ProgramData\Rustipedia\wikipedia") {
    $beforeCount = (Get-ChildItem "C:\ProgramData\Rustipedia\wikipedia" -Recurse -File).Count
    Write-Host "Files before upgrade: $beforeCount" -ForegroundColor Yellow
} else {
    $beforeCount = 0
    Write-Host "No existing data directory" -ForegroundColor Gray
}

# Save for later comparison
$beforeCount | Out-File "upgrade-test-before.txt"
```

### Step 4: Stop Existing Service (if running)

```powershell
# Stop service if it's running
$service = Get-Service rustipedia-serve -ErrorAction SilentlyContinue
if ($service -and $service.Status -eq 'Running') {
    Write-Host "Stopping service..." -ForegroundColor Yellow
    Stop-Service rustipedia-serve -Force
    Write-Host "✓ Service stopped" -ForegroundColor Green
}
```

### Step 5: Install/Upgrade to v0.2.2

**IMPORTANT**: This will launch the Windows Installer GUI.

```powershell
# Launch the new installer
Start-Process msiexec -ArgumentList '/i','G:\Projects\wiki_download\target\wix\rustipedia-0.2.2-x86_64.msi' -Wait

Write-Host ""
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "Installer completed!" -ForegroundColor Green
Write-Host "Now run the verification tests below..." -ForegroundColor Yellow
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""
```

### Step 6: Verify Installation

After the installer completes, run these verification checks:

```powershell
# Test 1: Check if binaries were updated
Write-Host "`n[Test 1] Check Installation" -ForegroundColor Cyan
$exePath = "C:\Program Files\Rustipedia\rustipedia-serve.exe"
if (Test-Path $exePath) {
    $version = (Get-Item $exePath).VersionInfo
    Write-Host "✓ Binary found: $exePath" -ForegroundColor Green
    Write-Host "  Version: $($version.ProductVersion)" -ForegroundColor Gray
} else {
    Write-Host "✗ Binary not found!" -ForegroundColor Red
}

# Test 2: Check DATA PRESERVATION (CRITICAL!)
Write-Host "`n[Test 2] Check Data Preservation" -ForegroundColor Cyan
$markerFile = "C:\ProgramData\Rustipedia\wikipedia\UPGRADE_TEST_MARKER.txt"
if (Test-Path $markerFile) {
    Write-Host "✓✓✓ SUCCESS! Test marker file EXISTS!" -ForegroundColor Green
    Write-Host "`nMarker file content:" -ForegroundColor Cyan
    Get-Content $markerFile
    Write-Host "`n✓ DATA PRESERVATION WORKS!" -ForegroundColor Green -BackgroundColor DarkGreen
} else {
    Write-Host "✗✗✗ FAILED! Test marker file was DELETED!" -ForegroundColor Red
    Write-Host "This means data was NOT preserved during upgrade!" -ForegroundColor Red
}

# Test 3: Verify file count didn't decrease
if (Test-Path "upgrade-test-before.txt") {
    $beforeCount = Get-Content "upgrade-test-before.txt"
    $afterCount = (Get-ChildItem "C:\ProgramData\Rustipedia\wikipedia" -Recurse -File -ErrorAction SilentlyContinue).Count
    Write-Host "`n[Test 3] File Count Verification" -ForegroundColor Cyan
    Write-Host "  Before upgrade: $beforeCount files" -ForegroundColor Gray
    Write-Host "  After upgrade:  $afterCount files" -ForegroundColor Gray
    
    if ($afterCount -ge $beforeCount) {
        Write-Host "✓ File count preserved or increased" -ForegroundColor Green
    } else {
        Write-Host "✗ Files were lost!" -ForegroundColor Red
    }
}

# Test 4: Check registry settings
Write-Host "`n[Test 4] Registry Configuration" -ForegroundColor Cyan
$reg = Get-ItemProperty "HKLM:\Software\Rustipedia" -ErrorAction SilentlyContinue
if ($reg) {
    Write-Host "✓ Registry key exists" -ForegroundColor Green
    Write-Host "  DataDirectory: $($reg.DataDirectory)" -ForegroundColor Gray
    Write-Host "  Version: $($reg.Version)" -ForegroundColor Gray
} else {
    Write-Host "✗ Registry key not found" -ForegroundColor Red
}

# Test 5: Check service configuration
Write-Host "`n[Test 5] Service Configuration" -ForegroundColor Cyan
$service = Get-Service rustipedia-serve -ErrorAction SilentlyContinue
if ($service) {
    Write-Host "✓ Service exists: $($service.DisplayName)" -ForegroundColor Green
    
    # Check service command line
    $svc = Get-WmiObject win32_service | Where-Object {$_.Name -eq 'rustipedia-serve'}
    if ($svc.PathName -match 'ProgramData') {
        Write-Host "✓ Service points to ProgramData location" -ForegroundColor Green
        Write-Host "  Command: $($svc.PathName)" -ForegroundColor Gray
    } else {
        Write-Host "⚠ Service may not be using ProgramData" -ForegroundColor Yellow
        Write-Host "  Command: $($svc.PathName)" -ForegroundColor Gray
    }
} else {
    Write-Host "✗ Service not found" -ForegroundColor Red
}

# Test 6: Try starting the service
Write-Host "`n[Test 6] Service Functionality" -ForegroundColor Cyan
try {
    Start-Service rustipedia-serve -ErrorAction Stop
    Start-Sleep -Seconds 3
    
    $service = Get-Service rustipedia-serve
    if ($service.Status -eq 'Running') {
        Write-Host "✓ Service started successfully!" -ForegroundColor Green
        
        # Try accessing the web server
        try {
            $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing
            Write-Host "✓ Web server is responding!" -ForegroundColor Green
            Write-Host "  Status: $($response.StatusCode)" -ForegroundColor Gray
        } catch {
            Write-Host "⚠ Web server not responding (may need data)" -ForegroundColor Yellow
        }
        
        # Stop the service
        Stop-Service rustipedia-serve
        Write-Host "✓ Service stopped cleanly" -ForegroundColor Green
    } else {
        Write-Host "⚠ Service started but not running" -ForegroundColor Yellow
    }
} catch {
    Write-Host "✗ Service failed to start: $_" -ForegroundColor Red
}
```

### Step 7: Final Summary

```powershell
Write-Host "`n"
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "         INSTALLATION TEST COMPLETE" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""
Write-Host "Key Success Criteria:" -ForegroundColor White
Write-Host "  1. Test marker file still exists ✓" -ForegroundColor Green
Write-Host "  2. File count not decreased ✓" -ForegroundColor Green
Write-Host "  3. Registry configured properly ✓" -ForegroundColor Green
Write-Host "  4. Service points to ProgramData ✓" -ForegroundColor Green
Write-Host "  5. Service can start ✓" -ForegroundColor Green
Write-Host ""

# Cleanup
if (Test-Path "upgrade-test-before.txt") {
    Remove-Item "upgrade-test-before.txt"
}
```

---

## Expected Results

### ✅ SUCCESS Indicators:
1. **UPGRADE_TEST_MARKER.txt** still exists in `C:\ProgramData\Rustipedia\wikipedia\`
2. File count in data directory stayed the same or increased
3. Registry has `DataDirectory` pointing to `C:\ProgramData\Rustipedia\wikipedia`
4. Service command includes `--data "C:\ProgramData\Rustipedia\wikipedia"`
5. Service starts without errors

### ❌ FAILURE Indicators:
1. Test marker file disappeared (data was deleted)
2. File count decreased (files were lost)
3. Registry missing or pointing elsewhere
4. Service crashes or fails to start

---

## Quick One-Command Test

If you want to run everything at once:

```powershell
# One-liner test (run as Administrator)
powershell -Command "& {
    $marker = 'C:\ProgramData\Rustipedia\wikipedia\UPGRADE_TEST_MARKER.txt';
    New-Item (Split-Path $marker) -ItemType Directory -Force | Out-Null;
    'Test created at ' + (Get-Date) | Set-Content $marker;
    Stop-Service rustipedia-serve -Force -ErrorAction SilentlyContinue;
    Start-Process msiexec -ArgumentList '/i','G:\Projects\wiki_download\target\wix\rustipedia-0.2.2-x86_64.msi' -Wait;
    if (Test-Path $marker) { 
        Write-Host '✓✓✓ DATA PRESERVED!' -ForegroundColor Green 
    } else { 
        Write-Host '✗✗✗ DATA LOST!' -ForegroundColor Red 
    }
}"
```

---

## Troubleshooting

### If tests fail:

1. **Check Windows Event Viewer**:
   - Open Event Viewer
   - Look in: Windows Logs → Application
   - Filter for source: MsiInstaller

2. **Check Installer Logs**:
   ```powershell
   # Run installer with logging
   msiexec /i "path\to\rustipedia-0.2.2-x86_64.msi" /l*v install.log
   
   # Review the log
   notepad install.log
   ```

3. **Manual Check**:
   ```powershell
   # List all Rustipedia-related registry keys
   Get-ChildItem "HKLM:\Software\Rustipedia" -Recurse
   
   # List all data directory contents
   Get-ChildItem "C:\ProgramData\Rustipedia" -Recurse
   
   # Check service
   Get-Service rustipedia-serve | Format-List *
   sc qc rustipedia-serve
   ```

---

## Next Steps After Successful Test

1. **Download Wikipedia data** (if not already done):
   ```powershell
   rustipedia-download --language en --data "C:\ProgramData\Rustipedia\wikipedia"
   ```

2. **Configure auto-updates**:
   ```powershell
   rustipedia-setup
   ```

3. **Start the service**:
   ```powershell
   Start-Service rustipedia-serve
   ```

4. **Access the web interface**:
   - Open browser to: http://localhost:8080

---

## Summary

This test verifies that:
- ✅ Wikipedia data survives upgrades
- ✅ Configuration persists
- ✅ Service continues to work
- ✅ Data is stored in the correct persistent location
- ✅ Future upgrades will preserve your data automatically

The new installer follows Windows best practices by storing user data in `ProgramData`, which is designed to persist across application upgrades!
