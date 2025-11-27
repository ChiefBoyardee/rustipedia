#!/bin/bash
set -e

# Wiki Download Installer for macOS

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║                     🛠️  WIKI INSTALLER                             ║"
echo "╚══════════════════════════════════════════════════════════════════╝"

# macOS doesn't necessarily need root if installing to user bin, but /usr/local/bin usually needs it.
# However, we want to run setup as the USER, not root, for LaunchAgent.
# So we might need to be careful.

if [ "$EUID" -eq 0 ]; then 
  echo "⚠️  Running as root. This is fine for copying binaries, but setup should be run as regular user."
  echo "   We will drop privileges for the setup phase if possible, or you should run setup manually."
fi

INSTALL_DIR="/usr/local/bin"

echo "Installing binaries to $INSTALL_DIR..."

# Ensure /usr/local/bin exists
if [ ! -d "$INSTALL_DIR" ]; then
    sudo mkdir -p "$INSTALL_DIR"
fi

# Copy binaries (using sudo if needed)
# We assume the script is run with sudo if needed, or we ask for it.
if [ ! -w "$INSTALL_DIR" ]; then
    echo "Need sudo access to write to $INSTALL_DIR"
    SUDO="sudo"
else
    SUDO=""
fi

if [ -f "wiki-serve" ]; then
    $SUDO cp wiki-serve "$INSTALL_DIR/"
    $SUDO chmod +x "$INSTALL_DIR/wiki-serve"
    echo "✅ Installed wiki-serve"
else
    echo "❌ Could not find wiki-serve binary"
    exit 1
fi

if [ -f "wiki-download" ]; then
    $SUDO cp wiki-download "$INSTALL_DIR/"
    $SUDO chmod +x "$INSTALL_DIR/wiki-download"
    echo "✅ Installed wiki-download"
else
    echo "❌ Could not find wiki-download binary"
    exit 1
fi

if [ -f "wiki-setup" ]; then
    $SUDO cp wiki-setup "$INSTALL_DIR/"
    $SUDO chmod +x "$INSTALL_DIR/wiki-setup"
    echo "✅ Installed wiki-setup"
else
    echo "❌ Could not find wiki-setup binary"
    exit 1
fi

echo "Installation complete."
echo "Running setup wizard..."
echo ""

# Run setup
# If we are root, we should try to run as the original user if SUDO_USER is set
if [ "$EUID" -eq 0 ] && [ -n "$SUDO_USER" ]; then
    echo "Switching to user '$SUDO_USER' for configuration..."
    sudo -u "$SUDO_USER" "$INSTALL_DIR/wiki-setup"
else
    "$INSTALL_DIR/wiki-setup"
fi
