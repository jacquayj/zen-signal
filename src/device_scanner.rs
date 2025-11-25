//! # Bluetooth Device Scanner Module
//!
//! Discovers nearby Polar heart rate sensors using btleplug.
//! Filters for devices with "polar" in their advertised name.
//!
//! ## Scan Process
//! 1. Get system Bluetooth adapter
//! 2. Start BLE scan with default filter
//! 3. Wait 5 seconds for device discovery
//! 4. Stop scan and collect peripherals
//! 5. Filter for Polar devices only
//!
//! ## Device Identification
//! Polar devices advertise with names like "Polar H10 12345678".
//! The ID is extracted from the last segment of the name for connection.
//!
//! ## Why Async
//! BLE scanning operations are async by nature. Running scan in async context
//! allows non-blocking discovery while UI remains responsive.

use crate::error::ScanError;
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
pub async fn scan_devices() -> Result<Vec<BluetoothDevice>, ScanError> {
    scan_with_btleplug().await
}

async fn scan_with_btleplug() -> Result<Vec<BluetoothDevice>, ScanError> {
    use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
    use btleplug::platform::Manager;

    let manager = Manager::new()
        .await
        .map_err(|e| ScanError::ManagerInit(e.to_string()))?;

    let adapters = manager
        .adapters()
        .await
        .map_err(|e| ScanError::ManagerInit(format!("Failed to get adapters: {}", e)))?;

    let central = adapters
        .into_iter()
        .next()
        .ok_or(ScanError::NoAdapters)?;

    // Start scanning for devices with filter
    central
        .start_scan(ScanFilter::default())
        .await
        .map_err(|e| ScanError::ScanFailed(e.to_string()))?;

    // Scan for 5 seconds
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Stop scanning
    central
        .stop_scan()
        .await
        .map_err(|e| ScanError::ScanFailed(format!("Failed to stop scan: {}", e)))?;

    // Get the discovered peripherals
    let peripherals = central
        .peripherals()
        .await
        .map_err(|e| ScanError::ScanFailed(format!("Failed to get peripherals: {}", e)))?;

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
