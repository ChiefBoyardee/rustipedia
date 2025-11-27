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

# Binaries to install
BINARIES=("rustipedia-download" "rustipedia-serve" "rustipedia-link-validator" "rustipedia-setup")

# Installation directory
INSTALL_DIR="/usr/local/bin"

echo "Installing Rustipedia..."

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

for binary in "${BINARIES[@]}"; do
    if [ -f "$binary" ]; then
        $SUDO cp "$binary" "$INSTALL_DIR/"
        $SUDO chmod +x "$INSTALL_DIR/$binary"
        echo "✅ Installed $binary"
    else
        echo "❌ Could not find $binary binary"
        exit 1
    fi
done

echo "Installation complete!"
echo "Run 'rustipedia-setup' to configure your server."

# Run setup wizard
# We need to run this as the original user, not root, so that config files end up in the right place
if [ "$EUID" -eq 0 ] && [ -n "$SUDO_USER" ]; then
    sudo -u "$SUDO_USER" "$INSTALL_DIR/rustipedia-setup"
else
    "$INSTALL_DIR/rustipedia-setup"
fi
