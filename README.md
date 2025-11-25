# ZenSignal

Real-time biosignal monitoring for Polar H10 heart rate sensors.

<img width="2672" height="1454" alt="image" src="https://github.com/user-attachments/assets/3f2f4a73-2af1-40a6-b7d1-17977ed2d6b8" />

## Features

- **Bluetooth Device Discovery** - Scan and connect to nearby Polar devices
- **Live Data Visualization** - Real-time charts for ECG, heart rate, RR intervals, and accelerometer data
- **Connection Management** - Easy connect/disconnect with visual feedback
- **Modern UI** - Clean interface built with Iced

## Requirements

- Rust 1.70+
- Bluetooth adapter
- Polar H10 heart rate sensor

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
