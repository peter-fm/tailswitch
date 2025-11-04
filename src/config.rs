use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub tailnets: Vec<Tailnet>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Tailnet {
    pub name: String,
    pub login_server: Option<String>,
    pub auth_key: Option<String>,
    pub flags: Option<Vec<String>>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            // Create default config
            let default_config = Self::default();
            default_config.save()?;
            return Ok(default_config);
        }

        let contents = fs::read_to_string(&config_path)
            .context("Failed to read config file")?;

        let config: Config = toml::from_str(&contents)
            .context("Failed to parse config file")?;

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }

        let contents = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;

        fs::write(&config_path, contents)
            .context("Failed to write config file")?;

        Ok(())
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config directory")?;
        Ok(config_dir.join("tailswitch").join("config.toml"))
    }

    pub fn get_config_path_string() -> Result<String> {
        Ok(Self::config_path()?.to_string_lossy().to_string())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tailnets: vec![
                Tailnet {
                    name: "Personal".to_string(),
                    login_server: None,
                    auth_key: None,
                    flags: None,
                },
                Tailnet {
                    name: "Work".to_string(),
                    login_server: Some("https://login.tailscale.com".to_string()),
                    auth_key: None,
                    flags: None,
                },
            ],
        }
    }
}
