use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sysinfo::{System, SystemExt, ProcessExt};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppInfo {
    pub name: String,
    pub start_time: DateTime<Utc>,
    pub duration: i64, // in seconds
    pub window_title: Option<String>,
    pub is_active: bool,
}

pub struct ProcessTracker {
    sys: System,
    active_apps: HashMap<String, AppInfo>,
    last_update: DateTime<Utc>,
    current_focused: Option<String>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        info!("Initializing ProcessTracker");
        Self {
            sys: System::new_all(),
            active_apps: HashMap::new(),
            last_update: Utc::now(),
            current_focused: None,
        }
    }

    pub fn update(&mut self) -> Result<()> {
        info!("Updating process tracker");
        self.sys.refresh_all();
        let now = Utc::now();
        let elapsed = (now - self.last_update).num_seconds();
        
        // Get the currently focused window
        let (focused_app, window_title) = self.get_focused_app();
        info!("Current focused app: {:?}, Window title: {:?}", focused_app, window_title);
        
        // Log current state of active apps
        info!("Current active apps: {:?}", self.active_apps.keys().collect::<Vec<_>>());
        
        // Update durations for all tracked apps
        for (name, info) in self.active_apps.iter_mut() {
            let was_active = info.is_active;
            info.is_active = Some(name.clone()) == focused_app;
            if info.is_active {
                info.duration += elapsed;
                if let Some(title) = &window_title {
                    info.window_title = Some(title.clone());
                }
                info!("Updated duration for {}: {} seconds (was active: {})", name, info.duration, was_active);
            }
        }
        
        // Add new app if it's not tracked yet
        if let Some(app_name) = &focused_app {
            if !self.active_apps.contains_key(app_name) {
                info!("Adding new app to track: {} with title: {:?}", app_name, window_title);
                self.active_apps.insert(
                    app_name.clone(),
                    AppInfo {
                        name: app_name.clone(),
                        start_time: now,
                        duration: 0,
                        window_title,
                        is_active: true,
                    },
                );
                info!("Current active apps after adding: {:?}", self.active_apps.keys().collect::<Vec<_>>());
            }
        }
        
        self.current_focused = focused_app;
        self.last_update = now;
        Ok(())
    }

    fn get_focused_app(&self) -> (Option<String>, Option<String>) {
        #[cfg(target_os = "linux")]
        {
            // Get active window ID using xwininfo
            match Command::new("xwininfo")
                .args(["-root", "-tree"])
                .output()
            {
                Ok(output) => {
                    if let Ok(output_str) = String::from_utf8(output.stdout) {
                        info!("Window tree: {}", output_str);
                        
                        // Find the active window (it should be the one with focus)
                        if let Some(active_line) = output_str.lines().find(|line| line.contains("has focus")) {
                            info!("Active window line: {}", active_line);
                            
                            // Extract window ID
                            if let Some(window_id) = active_line.split_whitespace().next() {
                                info!("Found window ID: {}", window_id);

                                // Get window title using xwininfo
                                if let Ok(title_output) = Command::new("xwininfo")
                                    .args(["-id", window_id, "-name"])
                                    .output()
                                {
                                    if let Ok(title_str) = String::from_utf8(title_output.stdout) {
                                        let window_title = title_str.lines()
                                            .find(|line| line.contains("xwininfo: Window id:"))
                                            .and_then(|line| line.split(": ").nth(1))
                                            .map(|s| s.trim().to_string());
                                        
                                        info!("Window title: {:?}", window_title);

                                        // Get process name using xprop
                                        if let Ok(xprop_output) = Command::new("xprop")
                                            .args(["-id", window_id, "WM_CLASS"])
                                            .output()
                                        {
                                            if let Ok(xprop_str) = String::from_utf8(xprop_output.stdout) {
                                                info!("xprop output: {}", xprop_str);
                                                
                                                // Try to get the class name from xprop output
                                                if let Some(class_part) = xprop_str.split('"').nth(3) {
                                                    let name = class_part.trim();
                                                    if !name.is_empty() {
                                                        info!("Found active process from WM_CLASS: {} with title: {:?}", name, window_title);
                                                        return (Some(name.to_string()), window_title);
                                                    }
                                                }
                                            }
                                        }

                                        // If we have a window title but no class, try to extract app name from title
                                        if let Some(title) = &window_title {
                                            // Common patterns in window titles
                                            let app_name = title.split(" - ")
                                                .next()
                                                .or_else(|| title.split(" — ").next())
                                                .or_else(|| title.split(" | ").next())
                                                .map(|s| s.trim().to_string());

                                            if let Some(name) = app_name {
                                                if !name.is_empty() {
                                                    info!("Extracted app name from title: {} with title: {:?}", name, window_title);
                                                    return (Some(name), window_title);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to get window tree: {}", e);
                }
            }

            // Last resort: try to get the active window using _NET_ACTIVE_WINDOW
            if let Ok(output) = Command::new("xprop")
                .args(["-root", "_NET_ACTIVE_WINDOW"])
                .output()
            {
                if let Ok(output_str) = String::from_utf8(output.stdout) {
                    if let Some(window_id) = output_str.split_whitespace().last() {
                        info!("Found active window ID from _NET_ACTIVE_WINDOW: {}", window_id);
                        
                        // Get window title
                        if let Ok(title_output) = Command::new("xwininfo")
                            .args(["-id", window_id, "-name"])
                            .output()
                        {
                            if let Ok(title_str) = String::from_utf8(title_output.stdout) {
                                let window_title = title_str.lines()
                                    .find(|line| line.contains("xwininfo: Window id:"))
                                    .and_then(|line| line.split(": ").nth(1))
                                    .map(|s| s.trim().to_string());
                                
                                if let Some(title) = window_title {
                                    // Try to extract app name from title
                                    let app_name = title.split(" - ")
                                        .next()
                                        .or_else(|| title.split(" — ").next())
                                        .or_else(|| title.split(" | ").next())
                                        .map(|s| s.trim().to_string());

                                    if let Some(name) = app_name {
                                        if !name.is_empty() {
                                            info!("Extracted app name from title (fallback): {} with title: {:?}", name, title);
                                            return (Some(name), Some(title));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("osascript")
                .args(["-e", "tell application \"System Events\" to get name of first process where it is frontmost"])
                .output()
            {
                if let Ok(name) = String::from_utf8(output.stdout) {
                    let name = name.trim();
                    if !name.is_empty() {
                        // Get window title on macOS
                        let window_title = Command::new("osascript")
                            .args(["-e", "tell application \"System Events\" to get name of first window of first process where it is frontmost"])
                            .output()
                            .ok()
                            .and_then(|output| String::from_utf8(output.stdout).ok())
                            .map(|title| title.trim().to_string());
                        
                        info!("Found active process on macOS: {} with title: {:?}", name, window_title);
                        return (Some(name.to_string()), window_title);
                    }
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, GetWindowTextW};
            use windows::Win32::Foundation::HWND;
            
            unsafe {
                let hwnd = GetForegroundWindow();
                let mut process_id: u32 = 0;
                GetWindowThreadProcessId(hwnd, &mut process_id);
                
                if process_id != 0 {
                    if let Some(process) = self.sys.process(sysinfo::Pid::from(process_id as usize)) {
                        let name = process.name().to_string();
                        
                        // Get window title
                        let mut title = [0u16; 512];
                        let len = GetWindowTextW(hwnd, &mut title);
                        let window_title = if len > 0 {
                            String::from_utf16_lossy(&title[..len as usize]).into()
                        } else {
                            None
                        };
                        
                        info!("Found active process on Windows: {} with title: {:?}", name, window_title);
                        return (Some(name), window_title);
                    }
                }
            }
        }
        
        warn!("No active window found");
        (None, None)
    }

    pub fn get_active_apps(&self) -> &HashMap<String, AppInfo> {
        &self.active_apps
    }
} 