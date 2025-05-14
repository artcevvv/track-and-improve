use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct DailyActivity {
    pub date: DateTime<Utc>,
    pub process_durations: HashMap<String, Duration>,
    pub focus_sessions: Vec<FocusSessionSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FocusSessionSummary {
    pub start_time: DateTime<Utc>,
    pub duration: Duration,
    pub music_used: bool,
}

pub struct Calendar {
    activities: HashMap<String, DailyActivity>, // Key: YYYY-MM-DD
}

impl Calendar {
    pub fn new() -> Self {
        Self {
            activities: HashMap::new(),
        }
    }

    pub fn add_activity(&mut self, process_name: String, duration: Duration) -> Result<()> {
        let today = Utc::now();
        let date_key = format!("{}-{:02}-{:02}", today.year(), today.month(), today.day());

        let activity = self.activities.entry(date_key).or_insert(DailyActivity {
            date: today,
            process_durations: HashMap::new(),
            focus_sessions: Vec::new(),
        });

        *activity.process_durations.entry(process_name).or_insert(Duration::zero()) += duration;
        Ok(())
    }

    pub fn add_focus_session(&mut self, session: FocusSessionSummary) -> Result<()> {
        let date_key = format!(
            "{}-{:02}-{:02}",
            session.start_time.year(),
            session.start_time.month(),
            session.start_time.day()
        );

        let activity = self.activities.entry(date_key).or_insert(DailyActivity {
            date: session.start_time,
            process_durations: HashMap::new(),
            focus_sessions: Vec::new(),
        });

        activity.focus_sessions.push(session);
        Ok(())
    }

    pub fn get_activity_for_date(&self, date: DateTime<Utc>) -> Option<&DailyActivity> {
        let date_key = format!("{}-{:02}-{:02}", date.year(), date.month(), date.day());
        self.activities.get(&date_key)
    }
} 