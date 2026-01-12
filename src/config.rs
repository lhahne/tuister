//! Configuration persistence module
//!
//! Handles saving and loading user preferences like selected models.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "config.json";
const APP_NAME: &str = "tuister";

/// Application configuration that is persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// IDs of the models that the user has selected
    #[serde(default)]
    pub selected_model_ids: Vec<String>,
}

impl Config {
    /// Get the config directory path
    fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join(APP_NAME))
    }

    /// Get the full path to the config file
    fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join(CONFIG_FILE_NAME))
    }

    /// Load configuration from disk
    /// Returns default config if file doesn't exist or can't be parsed
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save configuration to disk
    /// Creates the config directory if it doesn't exist
    pub fn save(&self) -> Result<(), std::io::Error> {
        let Some(dir) = Self::config_dir() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine config directory",
            ));
        };

        let Some(path) = Self::config_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine config path",
            ));
        };

        // Create config directory if it doesn't exist
        fs::create_dir_all(&dir)?;

        // Serialize and write
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&path, content)
    }

    /// Create a new config with the given selected model IDs
    pub fn with_selected_models(model_ids: Vec<String>) -> Self {
        Self {
            selected_model_ids: model_ids,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.selected_model_ids.is_empty());
    }

    #[test]
    fn test_config_with_selected_models() {
        let config = Config::with_selected_models(vec![
            "openai/gpt-4".to_string(),
            "anthropic/claude-3-opus".to_string(),
        ]);
        assert_eq!(config.selected_model_ids.len(), 2);
        assert_eq!(config.selected_model_ids[0], "openai/gpt-4");
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = Config::with_selected_models(vec!["test-model".to_string()]);
        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.selected_model_ids, loaded.selected_model_ids);
    }
}
