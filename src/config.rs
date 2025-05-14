use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub data_dir: PathBuf,
    pub music_dir: Option<PathBuf>,
    pub default_focus_duration: i64, // in minutes
    pub auto_start_focus: bool,
    pub track_window_titles: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("rizeclone"),
            music_dir: dirs::audio_dir(),
            default_focus_duration: 25, // Default to 25 minutes (Pomodoro)
            auto_start_focus: false,
            track_window_titles: true,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rizeclone")
            .join("config.json");

        if config_path.exists() {
            let config_str = std::fs::read_to_string(config_path)?;
            Ok(serde_json::from_str(&config_str)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rizeclone");

        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.json");
        let config_str = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, config_str)?;

        Ok(())
    }
} 