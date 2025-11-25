use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothDevice {
    pub id: String,
    pub name: String,
}

impl BluetoothDevice {
    pub fn new(id: String, name: String) -> Self {
        Self { id, name }
    }
}

/// Scans for nearby Polar Bluetooth devices
pub async fn scan_devices() -> Result<Vec<BluetoothDevice>, String> {
    // Try to scan for devices using btleplug
    match scan_with_btleplug().await {
        Ok(devices) => Ok(devices),
        Err(e) => Err(format!("Failed to scan for devices: {}", e)),
    }
}

async fn scan_with_btleplug() -> Result<Vec<BluetoothDevice>, String> {
    use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
    use btleplug::platform::Manager;

    let manager = Manager::new()
        .await
        .map_err(|e| format!("Failed to create BLE manager: {}", e))?;

    let adapters = manager
        .adapters()
        .await
        .map_err(|e| format!("Failed to get adapters: {}", e))?;

    let central = adapters
        .into_iter()
        .next()
        .ok_or_else(|| "No Bluetooth adapter found".to_string())?;

    // Start scanning for devices with filter
    central
        .start_scan(ScanFilter::default())
        .await
        .map_err(|e| format!("Failed to start scan: {}", e))?;

    // Scan for 5 seconds
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Stop scanning
    central
        .stop_scan()
        .await
        .map_err(|e| format!("Failed to stop scan: {}", e))?;

    // Get the discovered peripherals
    let peripherals = central
        .peripherals()
        .await
        .map_err(|e| format!("Failed to get peripherals: {}", e))?;

    let mut devices = Vec::new();

    for peripheral in peripherals {
        // Get peripheral properties
        if let Ok(Some(props)) = peripheral.properties().await {
            if let Some(local_name) = props.local_name {
                if local_name.to_lowercase().contains("polar") {
                    // Try to extract device ID from the local name (e.g., "Polar H10 12345678")
                    let id = local_name
                        .split_whitespace()
                        .last()
                        .unwrap_or(&peripheral.address().to_string())
                        .to_string();

                    devices.push(BluetoothDevice::new(id, local_name));
                }
            }
        }
    }

    Ok(devices)
}
