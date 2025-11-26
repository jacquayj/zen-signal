//! # Connection Management Module
//!
//! Handles Bluetooth device connection lifecycle for Polar sensors.
//! Encapsulates the complexity of connecting, configuring, and managing
//! the async runtime for sensor communication.
//!
//! ## Key Components
//! - `ConnectionManager`: Manages connection thread and command processing
//! - `ConnectionCommand`: Commands sent from UI to connection thread
//! - Connection state tracking with atomic stop flags
//!
//! ## Why
//! Separating connection management improves testability and makes it easier
//! to add features like recording that need to hook into the connection lifecycle.

use crate::error::ConnectionError;
use crate::sensor::{start_data_collection, Handler, SensorUpdate};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum ConnectionCommand {
    Connect(String),
    Disconnect,
}

/// Manages the connection lifecycle for Polar sensors.
/// 
/// Runs in a dedicated thread with its own Tokio runtime to avoid blocking
/// the UI thread. Processes connection commands and manages stop flags for
/// graceful disconnection.
pub struct ConnectionManager {
    command_receiver: mpsc::Receiver<ConnectionCommand>,
    sensor_sender: mpsc::Sender<SensorUpdate>,
}

impl ConnectionManager {
    /// Creates a new ConnectionManager.
    ///
    /// Returns the manager and a sender for issuing commands from the UI thread.
    pub fn new(sensor_sender: mpsc::Sender<SensorUpdate>) -> (Self, mpsc::Sender<ConnectionCommand>) {
        let (command_sender, command_receiver) = mpsc::channel();
        
        let manager = ConnectionManager {
            command_receiver,
            sensor_sender,
        };
        
        (manager, command_sender)
    }
    
    /// Runs the connection management loop.
    ///
    /// This should be called in a spawned thread. It will block until the command
    /// channel is closed.
    ///
    /// A separate thread is used because the connection process involves blocking
    /// async operations. Running in a separate thread with its own Tokio runtime
    /// prevents blocking the UI.
    pub fn run(self) {
        let rt = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(e) => {
                let error = ConnectionError::RuntimeCreation(e.to_string());
                log::error!("{}", error);
                let _ = self.sensor_sender.send(SensorUpdate::ConnectionStatus(
                    crate::sensor::ConnectionStatus::Error(error.to_string())
                ));
                return;
            }
        };
        
        let mut stop_flag: Option<Arc<AtomicBool>> = None;
        
        // Wait for connection commands
        while let Ok(command) = self.command_receiver.recv() {
            match command {
                ConnectionCommand::Connect(device_id) => {
                    log::info!("Connection manager: Connecting to device: {}", device_id);
                    
                    // Create a new stop flag for this connection
                    // Why: Each connection needs its own cancellation mechanism
                    let should_stop = Arc::new(AtomicBool::new(false));
                    stop_flag = Some(should_stop.clone());
                    
                    let sender_clone = self.sensor_sender.clone();
                    
                    // Spawn the connection task instead of blocking
                    // Why: Allows processing other commands (like disconnect) while connecting
                    rt.spawn(async move {
                        match arctic::PolarSensor::new(device_id.clone()).await {
                            Ok(sensor) => {
                                let polar = Arc::new(RwLock::new(sensor));
                                let handler = Handler::new(sender_clone.clone());
                                start_data_collection(polar, handler, should_stop).await;
                            }
                            Err(e) => {
                                let error = ConnectionError::DeviceConnection {
                                    device_id: device_id.clone(),
                                    reason: format!("{:?}", e),
                                };
                                log::error!("{}", error);
                                let _ = sender_clone.send(SensorUpdate::ConnectionStatus(
                                    crate::sensor::ConnectionStatus::Error(error.to_string())
                                ));
                            }
                        }
                    });
                }
                ConnectionCommand::Disconnect => {
                    log::info!("Connection manager: Disconnect requested");
                    if let Some(flag) = &stop_flag {
                        log::debug!("Connection manager: Setting stop flag");
                        flag.store(true, Ordering::Relaxed);
                    }
                    stop_flag = None;
                }
            }
        }
        
        log::info!("Connection manager: Command channel closed, shutting down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_connection_manager_creation() {
        let (sensor_sender, _sensor_receiver) = mpsc::channel();
        let (_manager, command_sender) = ConnectionManager::new(sensor_sender);
        
        // Verify we can send commands
        assert!(command_sender.send(ConnectionCommand::Disconnect).is_ok());
    }
}
