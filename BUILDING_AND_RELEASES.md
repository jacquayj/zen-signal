# Building and Releasing ZenSignal

Complete guide for building binaries and creating releases.

## Table of Contents
- [Quick Start](#quick-start)
- [Local Building](#local-building)
- [Creating Releases](#creating-releases)
- [For End Users](#for-end-users)

---

## Quick Start

### For Developers - Building from Source

```bash
# Clone and build
git clone https://github.com/jacquayj/zen-signal.git
cd zen-signal
cargo run --release
```

### For Releasing

```bash
# Update version in Cargo.toml, then:
git add Cargo.toml
git commit -m "Bump version to 0.1.0"
git push origin main
git tag v0.1.0
git push origin v0.1.0

# GitHub Actions will automatically build and publish binaries
# Watch at: https://github.com/jacquayj/zen-signal/actions
```

---

## Local Building

### Prerequisites

**All Platforms:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Linux Only:**
```bash
# Ubuntu/Debian
sudo apt-get install libdbus-1-dev pkg-config

# Fedora
sudo dnf install dbus-devel pkg-config

# Arch
sudo pacman -S dbus pkg-config
```

### Using Build Scripts

**macOS/Linux:**
```bash
./scripts/build.sh
# Creates: release/zen-signal-macos-universal.tar.gz (and others)
```

**Windows (PowerShell):**
```powershell
.\scripts\build.ps1
# Creates: release\zen-signal-windows-x86_64.zip
```

### Manual Building

**macOS Universal Binary:**
```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
lipo -create \
  target/aarch64-apple-darwin/release/zen-signal \
  target/x86_64-apple-darwin/release/zen-signal \
  -output zen-signal-macos-universal
strip zen-signal-macos-universal
tar -czf zen-signal-macos-universal.tar.gz zen-signal-macos-universal
```

**Windows:**
```powershell
cargo build --release --target x86_64-pc-windows-msvc
Compress-Archive -Path target\x86_64-pc-windows-msvc\release\zen-signal.exe `
  -DestinationPath zen-signal-windows-x86_64.zip
```

**Linux:**
```bash
cargo build --release --target x86_64-unknown-linux-gnu
strip target/x86_64-unknown-linux-gnu/release/zen-signal
tar -czf zen-signal-linux-x86_64.tar.gz \
  -C target/x86_64-unknown-linux-gnu/release zen-signal
```

---

## Creating Releases

### Automated Release (Recommended)

GitHub Actions automatically builds binaries for all platforms when you push a tag.

**Step 1: Update Version**
```toml
# Cargo.toml
[package]
version = "0.1.0"  # Bump this version
```

**Step 2: Commit and Tag**
```bash
git add Cargo.toml
git commit -m "Bump version to 0.1.0"
git push origin main

git tag v0.1.0
git push origin v0.1.0
```

**Step 3: Wait for Build**
- Go to https://github.com/jacquayj/zen-signal/actions
- Wait ~10-15 minutes for all builds to complete
- Check https://github.com/jacquayj/zen-signal/releases

**What Gets Built:**
- macOS DMG Installer (Universal - recommended)
- macOS Universal tarball (Apple Silicon + Intel)
- macOS Apple Silicon only
- macOS Intel only
- Windows 64-bit
- Linux 64-bit
- SHA256 checksums

### Manual Release

If you need to create a release without GitHub Actions:

1. Build locally using scripts (see above)
2. Go to https://github.com/jacquayj/zen-signal/releases
3. Click "Draft a new release"
4. Create tag `v0.1.0`
5. Upload all files from `release/` directory
6. Add release notes (template below)
7. Publish

**Release Notes Template:**
```markdown
## ZenSignal v0.1.0

### Downloads
- **macOS**: zen-signal-macos-universal.tar.gz (recommended)
- **Windows**: zen-signal-windows-x86_64.zip
- **Linux**: zen-signal-linux-x86_64.tar.gz

### Installation
See README.md for platform-specific installation instructions.

### Changes
- Initial release
- Real-time ECG, HR, RR, HRV monitoring
- Cross-platform support

### Requirements
- Bluetooth LE adapter
- Polar H10 sensor
```

---

## For End Users

If you downloaded a pre-built binary from the releases page:

### macOS
```bash
tar -xzf zen-signal-macos-universal.tar.gz
chmod +x zen-signal-macos-universal
./zen-signal-macos-universal
```

**Or use DMG Installer (Recommended):**
1. Download `ZenSignal-macos.dmg`
2. Open the DMG file
3. Drag `ZenSignal.app` to Applications folder
4. Launch from Applications (Launchpad or Finder)

**First run:** Right-click → Open (to bypass Gatekeeper)
**Permissions:** System Settings → Privacy & Security → Bluetooth

### Windows
1. Extract `zen-signal-windows-x86_64.zip`
2. Run `zen-signal.exe`
3. If blocked: Click "More info" → "Run anyway"

### Linux
```bash
tar -xzf zen-signal-linux-x86_64.tar.gz
chmod +x zen-signal
sudo usermod -a -G bluetooth $USER  # Add to bluetooth group
sudo setcap cap_net_raw+ep zen-signal  # Grant BLE permissions
./zen-signal
```

---

## Troubleshooting

### macOS: "Cannot be opened because developer cannot be verified"
```bash
xattr -d com.apple.quarantine zen-signal-macos-universal
```

### Linux: Permission denied on Bluetooth
```bash
sudo usermod -a -G bluetooth $USER
sudo setcap cap_net_raw+ep zen-signal
# Log out and back in
```

### Windows: Antivirus blocks executable
Add exception in Windows Defender or temporarily disable antivirus.

### GitHub Actions build fails
- Check Actions tab for logs
- Verify Cargo.toml syntax
- Ensure all dependencies are specified

---

## Testing Before Release

Test each binary before publishing:

```bash
# Extract binary
# Make executable (macOS/Linux)
# Run application
# Click "Scan for Devices"
# Connect to Polar device
# Verify data streams appear
```

---

## Quick Reference

**Build locally:**
```bash
./scripts/build.sh  # macOS/Linux
.\scripts\build.ps1  # Windows
```

**Create release:**
```bash
git tag v0.1.0
git push origin v0.1.0
```

**Monitor build:**
https://github.com/jacquayj/zen-signal/actions

**View releases:**
https://github.com/jacquayj/zen-signal/releases

---

## File Locations

After building, files are in `release/`:
- `ZenSignal-macos.dmg` - macOS installer (drag & drop to Applications)
- `zen-signal-macos-universal.tar.gz` - macOS (both architectures)
- `zen-signal-macos-aarch64.tar.gz` - macOS Apple Silicon
- `zen-signal-macos-x86_64.tar.gz` - macOS Intel
- `zen-signal-windows-x86_64.zip` - Windows
- `zen-signal-linux-x86_64.tar.gz` - Linux
- `checksums.txt` - SHA256 hashes

---

## Configuration

Config files are stored in:
- **macOS**: `~/Library/Application Support/zen-signal/config.toml`
- **Windows**: `%APPDATA%\zen-signal\config.toml`
- **Linux**: `~/.config/zen-signal/config.toml`

```toml
enable_autoconnect = false
smooth_data_streaming = true
```

---

## GitHub Actions Workflow

The release workflow (`.github/workflows/release.yml`) automatically:
1. Builds binaries for all platforms
2. Strips symbols to reduce size
3. Creates compressed archives
4. Generates SHA256 checksums
5. Creates GitHub Release with all files attached
6. Adds release notes

Triggers on:
- Tag push matching `v*.*.*` pattern
- Manual workflow dispatch

---

## Platform Requirements

**macOS**: Big Sur (11) or later
**Windows**: Windows 10 version 1703 or later
**Linux**: BlueZ 5.44 or later

All platforms need:
- Bluetooth LE adapter
- Polar H10 sensor

See README.md for detailed platform-specific setup.
