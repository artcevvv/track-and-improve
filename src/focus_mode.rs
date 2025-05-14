use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct FocusSession {
    pub start_time: DateTime<Utc>,
    pub duration: Duration,
    pub music_enabled: bool,
    pub music_path: Option<PathBuf>,
}

pub struct FocusMode {
    current_session: Option<FocusSession>,
    music_playlist: Vec<PathBuf>,
}

impl FocusMode {
    pub fn new() -> Self {
        Self {
            current_session: None,
            music_playlist: Vec::new(),
        }
    }

    pub fn start_session(&mut self, duration_minutes: i64, music_enabled: bool) -> Result<()> {
        let session = FocusSession {
            start_time: Utc::now(),
            duration: Duration::minutes(duration_minutes),
            music_enabled,
            music_path: if music_enabled {
                self.music_playlist.first().cloned()
            } else {
                None
            },
        };

        self.current_session = Some(session);
        Ok(())
    }

    pub fn end_session(&mut self) -> Result<()> {
        self.current_session = None;
        Ok(())
    }

    pub fn add_music(&mut self, path: PathBuf) {
        self.music_playlist.push(path);
    }

    pub fn get_current_session(&self) -> Option<&FocusSession> {
        self.current_session.as_ref()
    }

    pub fn is_session_active(&self) -> bool {
        self.current_session.is_some()
    }
} 