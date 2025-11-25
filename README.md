# ZenSignal

Real-time biosignal monitoring for Polar H10 heart rate sensors.

<img width="2672" height="1454" alt="image" src="https://github.com/user-attachments/assets/3f2f4a73-2af1-40a6-b7d1-17977ed2d6b8" />

## Features

- **Bluetooth Device Discovery** - Scan and connect to nearby Polar devices
- **Live Data Visualization** - Real-time charts for ECG, heart rate, RR intervals, and accelerometer data
- **Connection Management** - Easy connect/disconnect with visual feedback
- **Modern UI** - Clean interface built with Iced

## Platform Support

ZenSignal is **fully cross-platform** and supports:

- ✅ **Windows 10/11** - Uses Windows Bluetooth LE APIs
- ✅ **macOS** - Native CoreBluetooth support
- ✅ **Linux** - BlueZ D-Bus integration

### Platform-Specific Requirements

#### macOS
- macOS Big Sur (11) or later
- Bluetooth permission required: Go to **System Settings → Privacy & Security → Bluetooth** and enable for your terminal application

#### Windows
- Windows 10 version 1703 (Creators Update) or later
- Bluetooth LE adapter
- No additional permissions required

#### Linux
- BlueZ 5.44 or later
- D-Bus (usually pre-installed)
- User must be in the `bluetooth` group: `sudo usermod -a -G bluetooth $USER`
- May require additional permissions: `sudo setcap cap_net_raw+ep target/debug/zen-signal`

## Requirements

- Rust 1.70+
- Bluetooth LE adapter
- Polar H10 heart rate sensor
- Platform-specific dependencies (see above)

## Usageing

### All Platforms

```bash
# Clone the repository
git clone https://github.com/jacquayj/zen-signal.git
cd zen-signal

# Build and run
cargo run --release
```

### Linux-Specific Setup

If you encounter Bluetooth permission issues:

```bash
# Add your user to the bluetooth group
sudo usermod -a -G bluetooth $USER

# Log out and back in, then build
cargo build --release

# Grant network capabilities (required for BLE scanning)
sudo setcap cap_net_raw+ep target/release/zen-signal

# Run the application
./target/release/zen-signal
```

## Usage

```bash
cargo run
```

1. Click "Scan for Devices" to discover nearby Polar sensors
2. Select your device from the list
3. Click "Connect" to start streaming data
4. View real-time biosignal charts
5. Click "Disconnect" to stop

## Architecture

- **Backend**: Arctic library for Polar Bluetooth communication
- **Frontend**: Iced GUI framework
- **Plotting**: Plotters for real-time chart rendering
- **Async Runtime**: Tokio for connection management

## License

MIT
