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
            // First try Wayland using swaymsg
            if let Ok(output) = Command::new("swaymsg")
                .args(["-t", "get_tree"])
                .output()
            {
                if let Ok(output_str) = String::from_utf8(output.stdout) {
                    info!("Sway tree: {}", output_str);
                    
                    // Try to find the focused window
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
                        if let Some(focused) = find_focused_window(&json) {
                            if let (Some(name), Some(title)) = (focused.get("name"), focused.get("title")) {
                                let app_name = name.as_str().unwrap_or("").to_string();
                                let window_title = title.as_str().unwrap_or("").to_string();
                                
                                if !app_name.is_empty() {
                                    info!("Found Wayland window: {} with title: {:?}", app_name, window_title);
                                    return (Some(app_name), Some(window_title));
                                }
                            }
                        }
                    }
                }
            }

            // Fallback to X11 detection
            // Get the window tree using xwininfo
            match Command::new("xwininfo")
                .args(["-root", "-tree"])
                .output()
            {
                Ok(output) => {
                    if let Ok(output_str) = String::from_utf8(output.stdout) {
                        info!("Window tree: {}", output_str);
                        
                        // Find windows that have a name (visible windows)
                        for line in output_str.lines() {
                            if line.contains("has no name") {
                                continue;
                            }
                            
                            // Extract window ID
                            if let Some(window_id) = line.split_whitespace().next() {
                                info!("Found window: {}", line);
                                
                                // Get window properties using xprop
                                if let Ok(xprop_output) = Command::new("xprop")
                                    .args(["-id", window_id])
                                    .output()
                                {
                                    if let Ok(xprop_str) = String::from_utf8(xprop_output.stdout) {
                                        info!("Window properties: {}", xprop_str);
                                        
                                        // Check if window is visible and mapped
                                        let is_visible = xprop_str.contains("_NET_WM_STATE(ATOM)") && 
                                                       !xprop_str.contains("_NET_WM_STATE_HIDDEN");
                                        
                                        if is_visible {
                                            // Get window title
                                            let window_title = xprop_str.lines()
                                                .find(|line| line.contains("WM_NAME"))
                                                .and_then(|line| line.split('"').nth(1))
                                                .map(|s| s.trim().to_string());
                                            
                                            info!("Window title: {:?}", window_title);
                                            
                                            // Get window class
                                            let window_class = xprop_str.lines()
                                                .find(|line| line.contains("WM_CLASS"))
                                                .and_then(|line| {
                                                    let parts: Vec<&str> = line.split('"').collect();
                                                    if parts.len() >= 4 {
                                                        Some(parts[3].trim().to_string())
                                                    } else {
                                                        None
                                                    }
                                                });
                                            
                                            info!("Window class: {:?}", window_class);
                                            
                                            // Use class name if available, otherwise use title
                                            if let Some(class) = &window_class {
                                                if !class.is_empty() {
                                                    // Clean up the name
                                                    let clean_name = class.to_lowercase()
                                                        .replace("window", "")
                                                        .replace("browser", "")
                                                        .replace("client", "")
                                                        .trim()
                                                        .to_string();
                                                    
                                                    if !clean_name.is_empty() {
                                                        info!("Found window with class: {} and title: {:?}", clean_name, window_title);
                                                        return (Some(clean_name), window_title);
                                                    }
                                                }
                                            }
                                            
                                            // If no class, try to get name from title
                                            if let Some(title) = window_title {
                                                let app_name = title.split(" - ")
                                                    .next()
                                                    .or_else(|| title.split(" — ").next())
                                                    .or_else(|| title.split(" | ").next())
                                                    .map(|s| s.trim().to_string());
                                                
                                                if let Some(name) = app_name {
                                                    if !name.is_empty() {
                                                        // Clean up the name
                                                        let clean_name = name.to_lowercase()
                                                            .replace("window", "")
                                                            .replace("browser", "")
                                                            .replace("client", "")
                                                            .trim()
                                                            .to_string();
                                                        
                                                        if !clean_name.is_empty() {
                                                            info!("Found window with title: {} and class: {:?}", clean_name, window_class);
                                                            return (Some(clean_name), Some(title));
                                                        }
                                                    }
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

// Helper function to find the focused window in the sway tree
fn find_focused_window(node: &serde_json::Value) -> Option<&serde_json::Value> {
    if let Some(focused) = node.get("focused") {
        if focused.as_bool().unwrap_or(false) {
            return Some(node);
        }
    }
    
    if let Some(nodes) = node.get("nodes") {
        if let Some(nodes) = nodes.as_array() {
            for node in nodes {
                if let Some(focused) = find_focused_window(node) {
                    return Some(focused);
                }
            }
        }
    }
    
    None
} 