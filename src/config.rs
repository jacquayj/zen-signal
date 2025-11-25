use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub enable_autoconnect: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable_autoconnect: false,
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
    pub fn load() -> Self {
        let path = Self::config_path();
        
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(config) = toml::from_str(&contents) {
                return config;
            }
        }
        
        // Return default config if file doesn't exist or can't be parsed
        let config = Self::default();
        let _ = config.save(); // Try to save default config
        config
    }

    /// Save config to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let toml_string = toml::to_string_pretty(self)?;
        fs::write(&path, toml_string)?;
        
        Ok(())
    }
}
