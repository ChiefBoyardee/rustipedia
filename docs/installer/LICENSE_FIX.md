# EULA/License Update

## Issue
The Windows installer (MSI) was displaying "Lorem ipsum" placeholder text in the End-User License Agreement dialog, which is not appropriate for a production release.

## Solution
Replaced the placeholder EULA with the proper MIT License text to match the repository's licensing.

## Changes Made

### 1. Added `LICENSE` file to repository
Created `LICENSE` file at the project root containing the standard MIT License text with 2024 copyright for "Rustipedia Contributors".

### 2. Created `wix/License.rtf`
Created an RTF-formatted version of the MIT License for the Windows installer EULA dialog. WiX requires license files to be in RTF format for proper display in the installer UI.

### 3. Updated `wix/main.wxs`
Added the following line to specify the license file:
```xml
<WixVariable Id="WixUILicenseRtf" Value="wix\License.rtf" />
```

This tells WiX to use our MIT License RTF file instead of generating a default/placeholder EULA.

## Result
The Windows installer now displays the proper MIT License in the EULA dialog, matching the repository's licensing and ensuring users can see the actual terms under which the software is distributed.

## Files Created/Modified
- ✅ `LICENSE` - New MIT License file for the repository
- ✅ `wix/License.rtf` - RTF-formatted MIT License for installer
- ✅ `wix/main.wxs` - Updated to reference the license file

## Testing
The installer has been rebuilt and can be found at:
```
target\wix\rustipedia-0.2.2-x86_64.msi
```

When you run the installer, the EULA dialog will now show the MIT License instead of "Lorem ipsum" placeholder text.
