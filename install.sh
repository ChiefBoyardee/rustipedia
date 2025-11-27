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

INSTALL_DIR="/usr/local/bin"

echo "Installing binaries to $INSTALL_DIR..."

# Copy binaries
# Assumes we are running from the extracted tarball or build directory
if [ -f "wiki-serve" ]; then
    cp wiki-serve "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/wiki-serve"
    echo "✅ Installed wiki-serve"
else
    echo "❌ Could not find wiki-serve binary in current directory"
    exit 1
fi

if [ -f "wiki-download" ]; then
    cp wiki-download "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/wiki-download"
    echo "✅ Installed wiki-download"
else
    echo "❌ Could not find wiki-download binary"
    exit 1
fi

if [ -f "wiki-setup" ]; then
    cp wiki-setup "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/wiki-setup"
    echo "✅ Installed wiki-setup"
else
    echo "❌ Could not find wiki-setup binary"
    exit 1
fi

echo "Installation complete."
echo "Running setup wizard..."
echo ""

# Run setup
"$INSTALL_DIR/wiki-setup"
