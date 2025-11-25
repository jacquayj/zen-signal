# ZenSignal

Real-time biosignal monitoring for Polar H10 heart rate sensors.

<img width="2628" height="1477" alt="image" src="https://github.com/user-attachments/assets/7d19c7cc-34e0-4058-abf2-46db1ebc5449" />

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
