#!/bin/bash
# Build script for creating distributable ZenSignal binaries
# Location: scripts/build.sh
# Usage: ./scripts/build.sh

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}ZenSignal Build Script${NC}"
echo "======================================"

# Detect OS
if [[ "$OSTYPE" == "darwin"* ]]; then
    OS="macos"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS="linux"
else
    echo -e "${RED}Unsupported OS: $OSTYPE${NC}"
    echo "This script supports macOS and Linux."
    echo "For Windows, use build.ps1"
    exit 1
fi

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Cargo not found${NC}"
    echo "Install Rust from: https://rustup.rs/"
    exit 1
fi

echo "Detected OS: $OS"
echo ""

# Function to build for macOS
build_macos() {
    echo -e "${YELLOW}Building for macOS...${NC}"
    
    # Check if targets are installed
    if ! rustup target list | grep -q "aarch64-apple-darwin (installed)"; then
        echo "Installing aarch64-apple-darwin target..."
        rustup target add aarch64-apple-darwin
    fi
    
    if ! rustup target list | grep -q "x86_64-apple-darwin (installed)"; then
        echo "Installing x86_64-apple-darwin target..."
        rustup target add x86_64-apple-darwin
    fi
    
    echo "Building for Apple Silicon (aarch64)..."
    cargo build --release --target aarch64-apple-darwin
    
    echo "Building for Intel (x86_64)..."
    cargo build --release --target x86_64-apple-darwin
    
    echo "Creating universal binary..."
    lipo -create \
        target/aarch64-apple-darwin/release/zen-signal \
        target/x86_64-apple-darwin/release/zen-signal \
        -output zen-signal-macos-universal
    
    # Strip symbols
    echo "Stripping symbols..."
    strip zen-signal-macos-universal
    strip target/aarch64-apple-darwin/release/zen-signal
    strip target/x86_64-apple-darwin/release/zen-signal
    
    # Create archives
    echo "Creating archives..."
    mkdir -p release
    tar -czf release/zen-signal-macos-universal.tar.gz zen-signal-macos-universal
    tar -czf release/zen-signal-macos-aarch64.tar.gz -C target/aarch64-apple-darwin/release zen-signal
    tar -czf release/zen-signal-macos-x86_64.tar.gz -C target/x86_64-apple-darwin/release zen-signal
    
    echo -e "${GREEN}macOS binaries created:${NC}"
    echo "  - release/zen-signal-macos-universal.tar.gz"
    echo "  - release/zen-signal-macos-aarch64.tar.gz"
    echo "  - release/zen-signal-macos-x86_64.tar.gz"
}

# Function to build for Linux
build_linux() {
    echo -e "${YELLOW}Building for Linux...${NC}"
    
    # Check for required dependencies
    if ! pkg-config --exists dbus-1; then
        echo -e "${RED}Error: libdbus-1-dev not found${NC}"
        echo "Install with: sudo apt-get install libdbus-1-dev pkg-config"
        exit 1
    fi
    
    # Check if target is installed
    if ! rustup target list | grep -q "x86_64-unknown-linux-gnu (installed)"; then
        echo "Installing x86_64-unknown-linux-gnu target..."
        rustup target add x86_64-unknown-linux-gnu
    fi
    
    echo "Building for Linux (x86_64)..."
    cargo build --release --target x86_64-unknown-linux-gnu
    
    echo "Stripping symbols..."
    strip target/x86_64-unknown-linux-gnu/release/zen-signal
    
    echo "Creating archive..."
    mkdir -p release
    tar -czf release/zen-signal-linux-x86_64.tar.gz -C target/x86_64-unknown-linux-gnu/release zen-signal
    
    echo -e "${GREEN}Linux binary created:${NC}"
    echo "  - release/zen-signal-linux-x86_64.tar.gz"
}

# Build based on OS
if [ "$OS" == "macos" ]; then
    build_macos
elif [ "$OS" == "linux" ]; then
    build_linux
fi

# Generate checksums
echo ""
echo -e "${YELLOW}Generating checksums...${NC}"
cd release
shasum -a 256 *.tar.gz > checksums.txt
cd ..

echo -e "${GREEN}Checksums created: release/checksums.txt${NC}"

# Print summary
echo ""
echo "======================================"
echo -e "${GREEN}Build completed successfully!${NC}"
echo ""
echo "Release files are in the 'release' directory:"
ls -lh release/
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "1. Test the binaries"
echo "2. Create a git tag: git tag v0.1.0"
echo "3. Push the tag: git push origin v0.1.0"
echo "4. Or create a manual release with these files"
echo ""
echo "See BUILDING_AND_RELEASES.md for more details."
