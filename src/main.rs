mod process_tracker;
mod focus_mode;
mod calendar;
mod config;
mod utils;
mod gui;

use log::info;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> eframe::Result<()> {
    // Initialize logging
    env_logger::init();
    info!("Starting RizeClone productivity application");

    // Load configuration
    let config = config::Config::load().expect("Failed to load configuration");

    // Initialize components
    let process_tracker = Arc::new(Mutex::new(process_tracker::ProcessTracker::new()));
    let focus_mode = Arc::new(Mutex::new(focus_mode::FocusMode::new()));
    let calendar = Arc::new(Mutex::new(calendar::Calendar::new()));

    // Create the GUI application
    let app = gui::RizeCloneApp::new(
        config,
        process_tracker,
        focus_mode,
        calendar,
    );

    // Run the GUI
    let options = eframe::NativeOptions {
        window_builder: Some(Box::new(|builder| {
            builder.with_inner_size(egui::vec2(800.0, 600.0))
        })),
        ..Default::default()
    };

    eframe::run_native(
        "RizeClone",
        options,
        Box::new(|_cc| Box::new(app)),
    )
}
