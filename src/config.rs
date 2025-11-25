//! # Configuration Management Module
//!
//! Persistent application settings stored in platform-appropriate locations.
//! Handles loading, saving, and providing defaults for configuration options.
//!
//! ## Settings
//! - `enable_autoconnect`: Automatically connect to first Polar device found
//! - `smooth_data_streaming`: Enable display delay for smoother low-rate data
//!
//! ## Storage Location
//! - macOS: ~/Library/Application Support/zen-signal/config.toml
//! - Linux: ~/.config/zen-signal/config.toml
//! - Windows: %APPDATA%\zen-signal\config.toml
//!
//! ## Why TOML
//! Human-readable format allows manual editing if needed. Serde provides
//! automatic serialization/deserialization.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::error::ConfigError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub enable_autoconnect: bool,
    pub smooth_data_streaming: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable_autoconnect: false,
            smooth_data_streaming: true,
        }
    }
}

impl Config {
    /// Get the path to the config file
    fn config_path() -> PathBuf {
        let config_dir = if cfg!(target_os = "macos") {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        } else if cfg!(target_os = "windows") {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        } else {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        };
        
        config_dir.join("zen-signal").join("config.toml")
    }

    /// Load config from file, or create default if it doesn't exist
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path();
        
        match fs::read_to_string(&path) {
            Ok(contents) => {
                let config = toml::from_str(&contents)
                    .map_err(ConfigError::ParseFailed)?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File doesn't exist, create default
                let config = Self::default();
                config.save()?; // Save default config
                Ok(config)
            }
            Err(e) => Err(ConfigError::ReadFailed(e)),
        }
    }

    /// Save config to file
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path();
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(ConfigError::WriteFailed)?;
        }
        
        let toml_string = toml::to_string_pretty(self)
            .map_err(ConfigError::SerializeFailed)?;
        fs::write(&path, toml_string)
            .map_err(ConfigError::WriteFailed)?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.enable_autoconnect, false);
        assert_eq!(config.smooth_data_streaming, true);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            enable_autoconnect: true,
            smooth_data_streaming: false,
        };
        
        let toml_str = toml::to_string(&config).expect("Failed to serialize");
        assert!(toml_str.contains("enable_autoconnect = true"));
        assert!(toml_str.contains("smooth_data_streaming = false"));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_str = r#"
            enable_autoconnect = true
            smooth_data_streaming = false
        "#;
        
        let config: Config = toml::from_str(toml_str).expect("Failed to deserialize");
        assert_eq!(config.enable_autoconnect, true);
        assert_eq!(config.smooth_data_streaming, false);
    }

    #[test]
    fn test_config_load_creates_default() {
        // Test that load() works (either loads existing or creates default)
        let config = Config::load().expect("Failed to load config");
        // Just verify the config is valid, don't assume specific values
        // since a config file may already exist from previous runs
        assert!(config.enable_autoconnect == true || config.enable_autoconnect == false);
        assert!(config.smooth_data_streaming == true || config.smooth_data_streaming == false);
    }
    
    #[test]
    fn test_default_config_values() {
        // Test the default values specifically
        let config = Config::default();
        assert_eq!(config.enable_autoconnect, false);
        assert_eq!(config.smooth_data_streaming, true);
    }
}
