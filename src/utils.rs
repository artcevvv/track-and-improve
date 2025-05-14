use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::path::PathBuf;

pub fn format_duration(duration: Duration) -> String {
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

pub fn get_window_title() -> Result<Option<String>> {
    // TODO: Implement cross-platform window title detection
    // This will require platform-specific code for each OS
    Ok(None)
}

pub fn ensure_directory(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

pub fn get_timestamp() -> DateTime<Utc> {
    Utc::now()
} 