//! # Error Types Module
//!
//! Centralized error handling for the ZenSignal application.
//! Provides custom error types for each module with proper context and error chaining.
//!
//! ## Error Types
//! - `ConnectionError`: Bluetooth connection and runtime failures
//! - `ConfigError`: Configuration file I/O and parsing errors
//! - `ScanError`: Bluetooth device scanning errors
//!
//! ## Usage Examples
//! ```rust
//! // Config module uses ConfigError
//! pub fn load() -> Result<Config, ConfigError> { ... }
//! pub fn save(&self) -> Result<(), ConfigError> { ... }
//!
//! // Sensor module uses ConnectionError
//! async fn connect_to_device(...) -> Result<(), ConnectionError> { ... }
//!
//! // Scanner module uses ScanError
//! pub async fn scan_devices() -> Result<Vec<BluetoothDevice>, ScanError> { ... }
//! ```
//!
//! ## Why Custom Errors
//! - Better error messages for users and developers
//! - Type-safe error handling with match expressions
//! - Easier debugging with context preservation
//! - Enables proper error recovery strategies

use std::fmt;

/// Errors that can occur during connection management
#[derive(Debug)]
pub enum ConnectionError {
    /// Failed to create Tokio runtime
    RuntimeCreation(String),
    /// Bluetooth adapter not found or not available
    NoAdapter,
    /// Failed to connect to specific device
    DeviceConnection { device_id: String, reason: String },
    /// Connection was interrupted
    Interrupted,
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionError::RuntimeCreation(msg) => {
                write!(f, "Failed to create async runtime: {}", msg)
            }
            ConnectionError::NoAdapter => {
                write!(f, "No Bluetooth adapter found. Please ensure Bluetooth is enabled.")
            }
            ConnectionError::DeviceConnection { device_id, reason } => {
                write!(f, "Failed to connect to device {}: {}", device_id, reason)
            }
            ConnectionError::Interrupted => {
                write!(f, "Connection was interrupted by user")
            }
        }
    }
}

impl std::error::Error for ConnectionError {}

/// Errors that can occur during configuration operations
#[derive(Debug)]
pub enum ConfigError {
    /// Failed to read config file
    ReadFailed(std::io::Error),
    /// Failed to write config file
    WriteFailed(std::io::Error),
    /// Failed to parse config file
    ParseFailed(toml::de::Error),
    /// Failed to serialize config
    SerializeFailed(toml::ser::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::ReadFailed(e) => {
                write!(f, "Failed to read config file: {}", e)
            }
            ConfigError::WriteFailed(e) => {
                write!(f, "Failed to write config file: {}", e)
            }
            ConfigError::ParseFailed(e) => {
                write!(f, "Failed to parse config file: {}", e)
            }
            ConfigError::SerializeFailed(e) => {
                write!(f, "Failed to serialize config: {}", e)
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::ReadFailed(e) => Some(e),
            ConfigError::WriteFailed(e) => Some(e),
            ConfigError::ParseFailed(e) => Some(e),
            ConfigError::SerializeFailed(e) => Some(e),
        }
    }
}

/// Errors that can occur during device scanning
#[derive(Debug, Clone)]
pub enum ScanError {
    /// Bluetooth manager initialization failed
    ManagerInit(String),
    /// No Bluetooth adapters available
    NoAdapters,
    /// Scan operation failed
    ScanFailed(String),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::ManagerInit(msg) => {
                write!(f, "Failed to initialize Bluetooth manager: {}", msg)
            }
            ScanError::NoAdapters => {
                write!(f, "No Bluetooth adapters found")
            }
            ScanError::ScanFailed(msg) => {
                write!(f, "Scan operation failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for ScanError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error_display() {
        let err = ConnectionError::NoAdapter;
        assert!(err.to_string().contains("Bluetooth"));
    }

    #[test]
    fn test_config_error_chain() {
        use std::error::Error;
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = ConfigError::ReadFailed(io_err);
        assert!(err.source().is_some());
    }
}
