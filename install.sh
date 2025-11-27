#!/bin/bash
set -e

# Wiki Download Installer for Linux

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║                     🛠️  WIKI INSTALLER                             ║"
echo "╚══════════════════════════════════════════════════════════════════╝"

# Check for root
if [ "$EUID" -ne 0 ]; then 
  echo "Please run as root (sudo ./install.sh)"
  exit 1
fi

# Binaries to install
BINARIES=("rustipedia-download" "rustipedia-serve" "rustipedia-link-validator" "rustipedia-setup")

# Installation directory
INSTALL_DIR="/usr/local/bin"

echo "Installing Rustipedia binaries to $INSTALL_DIR..."

# Copy binaries
# Assumes we are running from the extracted tarball or build directory
for binary in "${BINARIES[@]}"; do
    if [ -f "$binary" ]; then
        cp "$binary" "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/$binary"
        echo "✅ Installed $binary"
    else
        echo "❌ Could not find $binary binary in current directory"
        exit 1
    fi
done

echo "Installation complete!"
echo "Run 'rustipedia-setup' to configure your server."
echo ""

# Run setup wizard
"$INSTALL_DIR/rustipedia-setup"
