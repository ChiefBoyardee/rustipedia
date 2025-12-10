# 🔧 Service Path Fix - Quick Guide

## Current Status

✅ **Data Preservation Works!** - Your Wikipedia data survived the upgrade
❌ **Service Points to Wrong Location** - Service still configured for old path

## The Problem

The Windows service updater doesn't change service arguments during upgrades. Your service is still pointing to:
```
"C:\Program Files\Rustipedia\rustipedia-serve.exe" --data "C:\Program Files\Rustipedia\wikipedia"
```

But it SHOULD point to:
```
"C:\Program Files\Rustipedia\rustipedia-serve.exe" --data "C:\ProgramData\Rustipedia\wikipedia"
```

## Quick Fix (DO THIS NOW)

**Open PowerShell as Administrator** and run:

```powershell
# Navigate to project
cd G:\Projects\wiki_download

# Stop service
Stop-Service rustipedia-serve -Force

# Delete old service configuration
sc.exe delete rustipedia-serve

# Recreate service with correct path
sc.exe create rustipedia-serve `
    binPath= "`"C:\Program Files\Rustipedia\rustipedia-serve.exe`" --data `"C:\ProgramData\Rustipedia\wikipedia`"" `
    DisplayName= "Rustipedia Local Wikipedia Server" `
 start= demand

# Verify the new configuration
sc qc rustipedia-serve

# Start the service
Start-Service rustipedia-serve

# Check status
Get-Service rustipedia-serve
```

## Verify It Worked

```powershell
# Check service command
$svc = Get-WmiObject win32_service | Where-Object {$_.Name -eq 'rustipedia-serve'}
$svc.PathName

# Should show: "C:\Program Files\Rustipedia\rustipedia-serve.exe" --data "C:\ProgramData\Rustipedia\wikipedia"
```

If you see "ProgramData" in the path, it's fixed! ✓

## Test the Web Server

```powershell
# Start the service
Start-Service rustipedia-serve

# Wait a moment
Start-Sleep -Seconds 3

# Test the web interface
Start-Process "http://localhost:8080"
```

---

## Long-Term Fix (For Future Installers)

I need to update the WiX installer to properly handle service upgrades. There are two approaches:

### Option A: Force Service Reinstall on Upgrade
Add custom action to remove/recreate service during MSI upgrade

### Option B: Manual Service Configuration
Document that users need to run the fix script after upgrade

### Option C: Use WiX ServiceConfig
Use `ServiceConfig` element to update service arguments

## Which Fix Do You Prefer?

1. **Quick Fix Only** - Just fix it manually this time, document for users
2. **Full Installer Fix** - Update WiX to handle this automatically (will rebuild MSI)
3. **Both** - Fix it now manually, and I'll also fix the installer for future releases

Let me know and I'll implement it!

---

## Also: Adding Upgrade/Repair/Uninstall UI

You mentioned wanting the standard Windows installer experience. I can add:

### Standard Windows Installer Behavior

When running the MSI on an existing installation, show:
- **Repair** - Fix broken/missing files
- **Remove** - Uninstall the application  
- **Change** - Modify installation (if applicable)

This requires switching from using `WixUI_InstallDir` to `WixUI_Mondo` or custom UI sequence.

**Want me to implement this too?**

---

## Next Steps

Tell me:
1. Should I fix the service path issue in the installer? (Yes/No)
2. Should I add proper upgrade/repair/remove UI? (Yes/No)
3. Any other installer improvements you want?

Then I'll create a new MSI with all the fixes!
