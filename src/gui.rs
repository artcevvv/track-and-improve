use crate::{
    calendar::Calendar,
    config::Config,
    focus_mode::FocusMode,
    process_tracker::ProcessTracker,
    utils::format_duration,
};
use chrono::{DateTime, Datelike, Duration, Local, Utc};
use eframe::egui;
use std::sync::{Arc, Mutex};

pub struct RizeCloneApp {
    config: Config,
    process_tracker: Arc<Mutex<ProcessTracker>>,
    focus_mode: Arc<Mutex<FocusMode>>,
    calendar: Arc<Mutex<Calendar>>,
    selected_date: DateTime<Local>,
    current_tab: Tab,
}

#[derive(PartialEq)]
enum Tab {
    Dashboard,
    Calendar,
    Focus,
    Settings,
}

impl RizeCloneApp {
    pub fn new(
        config: Config,
        process_tracker: Arc<Mutex<ProcessTracker>>,
        focus_mode: Arc<Mutex<FocusMode>>,
        calendar: Arc<Mutex<Calendar>>,
    ) -> Self {
        Self {
            config,
            process_tracker,
            focus_mode,
            calendar,
            selected_date: Local::now(),
            current_tab: Tab::Dashboard,
        }
    }

    fn render_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.heading("Dashboard");
        
        // Update process tracking
        if let Ok(mut tracker) = self.process_tracker.lock() {
            let _ = tracker.update();
        }
        
        // Active applications section
        ui.collapsing("Active Applications", |ui| {
            if let Ok(tracker) = self.process_tracker.lock() {
                let mut apps: Vec<_> = tracker.get_active_apps().iter().collect();
                apps.sort_by(|a, b| b.1.duration.cmp(&a.1.duration));

                for (name, info) in apps {
                    ui.horizontal(|ui| {
                        if info.is_active {
                            ui.label("●"); // Active indicator
                        } else {
                            ui.label("○"); // Inactive indicator
                        }
                        ui.label(name);
                        ui.label(format_duration(Duration::seconds(info.duration)));
                    });
                }
            }
        });

        // Focus mode section
        ui.collapsing("Focus Mode", |ui| {
            if let Ok(focus) = self.focus_mode.lock() {
                if let Some(session) = focus.get_current_session() {
                    ui.label(format!(
                        "Current Session: {} minutes",
                        session.duration.num_minutes()
                    ));
                    if session.music_enabled {
                        ui.label("Music: Playing");
                    }
                } else {
                    if ui.button("Start Focus Session").clicked() {
                        if let Ok(mut focus) = self.focus_mode.lock() {
                            let _ = focus.start_session(
                                self.config.default_focus_duration,
                                self.config.music_dir.is_some(),
                            );
                        }
                    }
                }
            }
        });
    }

    fn render_calendar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Calendar View");

        // Month navigation
        ui.horizontal(|ui| {
            if ui.button("←").clicked() {
                self.selected_date = self.selected_date - chrono::Duration::days(30);
            }
            ui.label(format!(
                "{} {}",
                self.selected_date.format("%B"),
                self.selected_date.year()
            ));
            if ui.button("→").clicked() {
                self.selected_date = self.selected_date + chrono::Duration::days(30);
            }
        });

        // Calendar grid
        egui::Grid::new("calendar_grid").show(ui, |ui| {
            // Day headers
            for day in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
                ui.label(day);
            }
            ui.end_row();

            // Calendar days
            if let Ok(calendar) = self.calendar.lock() {
                if let Some(activity) = calendar.get_activity_for_date(self.selected_date.into()) {
                    ui.label(format!(
                        "Total Focus Time: {}",
                        format_duration(
                            activity
                                .focus_sessions
                                .iter()
                                .map(|s| s.duration)
                                .sum::<Duration>()
                        )
                    ));
                }
            }
        });
    }

    fn render_focus(&mut self, ui: &mut egui::Ui) {
        ui.heading("Focus Mode");

        if let Ok(mut focus) = self.focus_mode.lock() {
            if focus.is_session_active() {
                if let Some(session) = focus.get_current_session() {
                    let elapsed = Utc::now() - session.start_time;
                    let remaining = session.duration - elapsed;

                    ui.label(format!(
                        "Time Remaining: {}",
                        format_duration(remaining)
                    ));

                    if ui.button("End Session").clicked() {
                        let _ = focus.end_session();
                    }
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label("Duration (minutes):");
                    let mut duration = self.config.default_focus_duration;
                    if ui.add(egui::DragValue::new(&mut duration).speed(1)).changed() {
                        self.config.default_focus_duration = duration;
                    }
                });

                ui.checkbox(&mut self.config.auto_start_focus, "Auto-start focus sessions");

                if ui.button("Start Focus Session").clicked() {
                    let _ = focus.start_session(
                        self.config.default_focus_duration,
                        self.config.music_dir.is_some(),
                    );
                }
            }
        }
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");

        ui.checkbox(&mut self.config.track_window_titles, "Track Window Titles");

        if ui.button("Save Settings").clicked() {
            let _ = self.config.save();
        }
    }
}

impl eframe::App for RizeCloneApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::Dashboard, "Dashboard");
                ui.selectable_value(&mut self.current_tab, Tab::Calendar, "Calendar");
                ui.selectable_value(&mut self.current_tab, Tab::Focus, "Focus");
                ui.selectable_value(&mut self.current_tab, Tab::Settings, "Settings");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_tab {
                Tab::Dashboard => self.render_dashboard(ui),
                Tab::Calendar => self.render_calendar(ui),
                Tab::Focus => self.render_focus(ui),
                Tab::Settings => self.render_settings(ui),
            }
        });
    }
} 