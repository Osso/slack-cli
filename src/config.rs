use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct Config {
    pub token: Option<String>,
}

fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("slack")
        .join("config.json")
}

pub fn load_config() -> Result<Config> {
    load_config_from(&default_config_path())
}

pub fn save_config(config: &Config) -> Result<()> {
    save_config_to(config, &default_config_path())
}

pub fn load_config_from(path: &PathBuf) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save_config_to(config: &Config, path: &PathBuf) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.token, None);
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let config = load_config_from(&path).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn test_save_and_load_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = Config {
            token: Some("xoxb-test-token".to_string()),
        };

        save_config_to(&config, &path).unwrap();
        let loaded = load_config_from(&path).unwrap();

        assert_eq!(loaded.token, Some("xoxb-test-token".to_string()));
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("config.json");

        let config = Config {
            token: Some("test".to_string()),
        };

        save_config_to(&config, &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_load_invalid_json_returns_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "not valid json").unwrap();

        let result = load_config_from(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            token: Some("xoxb-123".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("xoxb-123"));

        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.token, config.token);
    }
}
