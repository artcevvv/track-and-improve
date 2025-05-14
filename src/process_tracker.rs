use anyhow::Result;
use log::{info, warn, error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sysinfo::System;
use std::process::Command;
use serde_json;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, Duration};

// Helper function to find focused window in Wayland tree
fn find_focused_window(json: &serde_json::Value) -> Option<&serde_json::Value> {
    if let Some(focused) = json.get("focused") {
        if focused.as_bool().unwrap_or(false) {
            return Some(json);
        }
    }
    
    if let Some(nodes) = json.get("nodes") {
        if let Some(nodes) = nodes.as_array() {
            for node in nodes {
                if let Some(found) = find_focused_window(node) {
                    return Some(found);
                }
            }
        }
    }
    
    None
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppInfo {
    pub name: String,
    pub window_title: String,
    pub duration: Duration,
    pub is_active: bool,
    pub is_minimized: bool,
    pub is_maximized: bool,
    pub is_fullscreen: bool,
    pub workspace: Option<i32>,
    pub monitor: Option<i32>,
    pub is_urgent: bool,
    pub is_skip_taskbar: bool,
    pub app_id: Option<String>,
    pub stable_sequence: Option<i32>,
    pub last_updated: SystemTime,
}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub class: String,
    pub pid: Option<i32>,
    pub is_minimized: bool,
    pub is_maximized: bool,
    pub is_fullscreen: bool,
    pub workspace: Option<i32>,
    pub monitor: Option<i32>,
    pub is_urgent: bool,
    pub is_skip_taskbar: bool,
    pub app_id: Option<String>,
    pub app_name: Option<String>,
    pub stable_sequence: Option<i32>,
}

pub trait WindowDetector: Send + Sync {
    fn get_active_window(&self) -> Result<Option<WindowInfo>>;
    fn get_all_windows(&self) -> Result<Vec<WindowInfo>>;
}

pub struct X11Detector;

impl WindowDetector for X11Detector {
    fn get_active_window(&self) -> Result<Option<WindowInfo>> {
        // Try to get window info using gdbus first
        let gdbus_cmd = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest", "org.gnome.Shell",
                "--object-path", "/org/gnome/Shell",
                "--method", "org.gnome.Shell.Eval",
                "let windows = global.get_window_actors(); let focused = windows.find(w => w.meta_window.has_focus()); if (focused) { let window = focused.meta_window; window.get_wm_class() + '|' + window.get_title() + '|' + window.get_pid(); } else { ''; }"
            ])
            .output();

        match gdbus_cmd {
            Ok(output) => {
                if !output.status.success() {
                    error!("gdbus command failed: {}", String::from_utf8_lossy(&output.stderr));
                } else if let Ok(output_str) = String::from_utf8(output.stdout) {
                    info!("GNOME Shell response: {}", output_str);
                    
                    // Parse the response
                    if let Some(data_str) = output_str.split("'").nth(1) {
                        info!("Extracted data string: {}", data_str);
                        if !data_str.is_empty() {
                            let parts: Vec<&str> = data_str.split('|').collect();
                            if parts.len() >= 3 {
                                let class = parts[0].to_string();
                                let title = parts[1].to_string();
                                if let Ok(pid) = parts[2].parse::<i32>() {
                                    info!("Found active window: class={}, title={}, pid={}", class, title, pid);
                                    return Ok(Some(WindowInfo {
                                        id: class.clone(),
                                        title,
                                        class,
                                        pid: Some(pid),
                                        is_minimized: false,
                                        is_maximized: false,
                                        is_fullscreen: false,
                                        workspace: None,
                                        monitor: None,
                                        is_urgent: false,
                                        is_skip_taskbar: false,
                                        app_id: None,
                                        app_name: None,
                                        stable_sequence: None,
                                    }));
                                }
                            }
                        }
                    }
                }
            },
            Err(e) => error!("Failed to execute gdbus command: {}", e),
        }
        
        // Fallback to xprop if gdbus fails
        let xprop_cmd = Command::new("xprop")
            .args(["-root", "_NET_ACTIVE_WINDOW"])
            .output();

        match xprop_cmd {
            Ok(output) => {
                if !output.status.success() {
                    error!("xprop command failed: {}", String::from_utf8_lossy(&output.stderr));
                } else if let Ok(output_str) = String::from_utf8(output.stdout) {
                    info!("xprop response: {}", output_str);
                    if let Some(window_id) = output_str.split_whitespace().last() {
                        info!("Found active window ID: {}", window_id);
                        
                        // Get window properties
                        let xprop_info_cmd = Command::new("xprop")
                            .args(["-id", window_id])
                            .output();

                        match xprop_info_cmd {
                            Ok(xprop_output) => {
                                if !xprop_output.status.success() {
                                    error!("xprop info command failed: {}", String::from_utf8_lossy(&xprop_output.stderr));
                                } else if let Ok(xprop_str) = String::from_utf8(xprop_output.stdout) {
                                    info!("Window properties: {}", xprop_str);
                                    
                                    // Get window title
                                    let window_title = xprop_str.lines()
                                        .find(|line| line.contains("WM_NAME"))
                                        .and_then(|line| line.split('"').nth(1))
                                        .map(|s| s.trim().to_string())
                                        .unwrap_or_default();
                                    
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
                                        })
                                        .unwrap_or_default();
                                    
                                    info!("Found window: title={}, class={}", window_title, window_class);
                                    
                                    return Ok(Some(WindowInfo {
                                        id: window_id.to_string(),
                                        title: window_title,
                                        class: window_class,
                                        pid: None,
                                        is_minimized: false,
                                        is_maximized: false,
                                        is_fullscreen: false,
                                        workspace: None,
                                        monitor: None,
                                        is_urgent: false,
                                        is_skip_taskbar: false,
                                        app_id: None,
                                        app_name: None,
                                        stable_sequence: None,
                                    }));
                                }
                            },
                            Err(e) => error!("Failed to execute xprop info command: {}", e),
                        }
                    }
                }
            },
            Err(e) => error!("Failed to execute xprop command: {}", e),
        }
        
        warn!("No active window found");
        Ok(None)
    }
    
    fn get_all_windows(&self) -> Result<Vec<WindowInfo>> {
        let mut windows = Vec::new();
        
        // Try to get all windows using gdbus
        let gdbus_cmd = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest", "org.gnome.Shell",
                "--object-path", "/org/gnome/Shell",
                "--method", "org.gnome.Shell.Eval",
                "let windows = global.get_window_actors(); windows.map(w => { let window = w.meta_window; window.get_wm_class() + '|' + window.get_title() + '|' + window.get_pid(); }).join('\\n');"
            ])
            .output();

        match gdbus_cmd {
            Ok(output) => {
                if !output.status.success() {
                    error!("gdbus command failed: {}", String::from_utf8_lossy(&output.stderr));
                } else if let Ok(output_str) = String::from_utf8(output.stdout) {
                    info!("GNOME Shell response for all windows: {}", output_str);
                    
                    // Parse the response
                    if let Some(data_str) = output_str.split("'").nth(1) {
                        info!("Extracted data string: {}", data_str);
                        for line in data_str.lines() {
                            let parts: Vec<&str> = line.split('|').collect();
                            if parts.len() >= 3 {
                                let class = parts[0].to_string();
                                let title = parts[1].to_string();
                                if let Ok(pid) = parts[2].parse::<i32>() {
                                    info!("Found window: class={}, title={}, pid={}", class, title, pid);
                                    windows.push(WindowInfo {
                                        id: class.clone(),
                                        title,
                                        class,
                                        pid: Some(pid),
                                        is_minimized: false,
                                        is_maximized: false,
                                        is_fullscreen: false,
                                        workspace: None,
                                        monitor: None,
                                        is_urgent: false,
                                        is_skip_taskbar: false,
                                        app_id: None,
                                        app_name: None,
                                        stable_sequence: None,
                                    });
                                }
                            }
                        }
                    }
                }
            },
            Err(e) => error!("Failed to execute gdbus command: {}", e),
        }
        
        Ok(windows)
    }
}

pub struct WaylandDetector;

impl WindowDetector for WaylandDetector {
    fn get_active_window(&self) -> Result<Option<WindowInfo>> {
        // Use gdbus to get window info from GNOME Shell
        let gdbus_cmd = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest", "org.gnome.Shell",
                "--object-path", "/org/gnome/Shell",
                "--method", "org.gnome.Shell.Eval",
                "try {
                    let focused = global.get_window_actors().find(w => w.meta_window.has_focus());
                    if (focused) {
                        let window = focused.meta_window;
                        let app = Shell.WindowTracker.get_default().get_window_app(window);
                        JSON.stringify({
                            class: window.get_wm_class(),
                            title: window.get_title(),
                            pid: window.get_pid(),
                            minimized: window.minimized,
                            maximized: window.get_maximized() === Meta.MaximizeFlags.BOTH,
                            fullscreen: window.fullscreen,
                            workspace: window.get_workspace() ? window.get_workspace().index() : null,
                            monitor: window.get_monitor(),
                            urgent: window.urgent,
                            skip_taskbar: window.skip_taskbar,
                            app_id: app ? app.get_id() : null,
                            app_name: app ? app.get_name() : null,
                            stable_sequence: window.get_stable_sequence()
                        });
                    } else {
                        'null'
                    }
                } catch(e) {
                    'null'
                }"
            ])
            .output();

        match gdbus_cmd {
            Ok(output) => {
                if !output.status.success() {
                    error!("gdbus command failed: {}", String::from_utf8_lossy(&output.stderr));
                } else if let Ok(output_str) = String::from_utf8(output.stdout) {
                    info!("GNOME Shell response: {}", output_str);
                    
                    // Parse the response
                    if let Some(json_str) = output_str.split("'").nth(1) {
                        info!("Extracted JSON string: {}", json_str);
                        if json_str == "null" {
                            return Ok(None);
                        }
                        match serde_json::from_str::<serde_json::Value>(json_str) {
                            Ok(json) => {
                                info!("Parsed JSON: {:?}", json);
                                if let (Some(class), Some(title), Some(pid)) = (
                                    json.get("class").and_then(|v| v.as_str()),
                                    json.get("title").and_then(|v| v.as_str()),
                                    json.get("pid").and_then(|v| v.as_i64())
                                ) {
                                    info!("Found active window: class={}, title={}, pid={}", class, title, pid);
                                    return Ok(Some(WindowInfo {
                                        id: class.to_string(),
                                        title: title.to_string(),
                                        class: class.to_string(),
                                        pid: Some(pid as i32),
                                        is_minimized: json.get("minimized").and_then(|v| v.as_bool()).unwrap_or(false),
                                        is_maximized: json.get("maximized").and_then(|v| v.as_bool()).unwrap_or(false),
                                        is_fullscreen: json.get("fullscreen").and_then(|v| v.as_bool()).unwrap_or(false),
                                        workspace: json.get("workspace").and_then(|v| v.as_i64()).map(|v| v as i32),
                                        monitor: json.get("monitor").and_then(|v| v.as_i64()).map(|v| v as i32),
                                        is_urgent: json.get("urgent").and_then(|v| v.as_bool()).unwrap_or(false),
                                        is_skip_taskbar: json.get("skip_taskbar").and_then(|v| v.as_bool()).unwrap_or(false),
                                        app_id: json.get("app_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                        app_name: json.get("app_name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                        stable_sequence: json.get("stable_sequence").and_then(|v| v.as_i64()).map(|v| v as i32),
                                    }));
                                } else {
                                    error!("Missing required fields in JSON response");
                                }
                            },
                            Err(e) => error!("Failed to parse JSON: {}", e),
                        }
                    } else {
                        error!("Failed to extract JSON string from response");
                    }
                } else {
                    error!("Failed to decode gdbus output as UTF-8");
                }
            },
            Err(e) => error!("Failed to execute gdbus command: {}", e),
        }
        
        warn!("No active window found");
        Ok(None)
    }
    
    fn get_all_windows(&self) -> Result<Vec<WindowInfo>> {
        let mut windows = Vec::new();
        
        // Use gdbus to get all windows from GNOME Shell
        let gdbus_cmd = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest", "org.gnome.Shell",
                "--object-path", "/org/gnome/Shell",
                "--method", "org.gnome.Shell.Eval",
                "try {
                    let windows = global.get_window_actors();
                    let tracker = Shell.WindowTracker.get_default();
                    JSON.stringify(windows.map(w => {
                        let window = w.meta_window;
                        let app = tracker.get_window_app(window);
                        return {
                            class: window.get_wm_class(),
                            title: window.get_title(),
                            pid: window.get_pid(),
                            minimized: window.minimized,
                            maximized: window.get_maximized() === Meta.MaximizeFlags.BOTH,
                            fullscreen: window.fullscreen,
                            workspace: window.get_workspace() ? window.get_workspace().index() : null,
                            monitor: window.get_monitor(),
                            urgent: window.urgent,
                            skip_taskbar: window.skip_taskbar,
                            app_id: app ? app.get_id() : null,
                            app_name: app ? app.get_name() : null,
                            stable_sequence: window.get_stable_sequence()
                        };
                    }));
                } catch(e) {
                    '[]'
                }"
            ])
            .output();

        match gdbus_cmd {
            Ok(output) => {
                if !output.status.success() {
                    error!("gdbus command failed: {}", String::from_utf8_lossy(&output.stderr));
                } else if let Ok(output_str) = String::from_utf8(output.stdout) {
                    info!("GNOME Shell response for all windows: {}", output_str);
                    
                    // Parse the response
                    if let Some(json_str) = output_str.split("'").nth(1) {
                        info!("Extracted JSON string: {}", json_str);
                        match serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
                            Ok(json_array) => {
                                info!("Parsed JSON array: {:?}", json_array);
                                for window in json_array {
                                    if let (Some(class), Some(title), Some(pid)) = (
                                        window.get("class").and_then(|v| v.as_str()),
                                        window.get("title").and_then(|v| v.as_str()),
                                        window.get("pid").and_then(|v| v.as_i64())
                                    ) {
                                        info!("Found window: class={}, title={}, pid={}", class, title, pid);
                                        windows.push(WindowInfo {
                                            id: class.to_string(),
                                            title: title.to_string(),
                                            class: class.to_string(),
                                            pid: Some(pid as i32),
                                            is_minimized: window.get("minimized").and_then(|v| v.as_bool()).unwrap_or(false),
                                            is_maximized: window.get("maximized").and_then(|v| v.as_bool()).unwrap_or(false),
                                            is_fullscreen: window.get("fullscreen").and_then(|v| v.as_bool()).unwrap_or(false),
                                            workspace: window.get("workspace").and_then(|v| v.as_i64()).map(|v| v as i32),
                                            monitor: window.get("monitor").and_then(|v| v.as_i64()).map(|v| v as i32),
                                            is_urgent: window.get("urgent").and_then(|v| v.as_bool()).unwrap_or(false),
                                            is_skip_taskbar: window.get("skip_taskbar").and_then(|v| v.as_bool()).unwrap_or(false),
                                            app_id: window.get("app_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                            app_name: window.get("app_name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                            stable_sequence: window.get("stable_sequence").and_then(|v| v.as_i64()).map(|v| v as i32),
                                        });
                                    }
                                }
                            },
                            Err(e) => error!("Failed to parse JSON array: {}", e),
                        }
                    } else {
                        error!("Failed to extract JSON string from response");
                    }
                } else {
                    error!("Failed to decode gdbus output as UTF-8");
                }
            },
            Err(e) => error!("Failed to execute gdbus command: {}", e),
        }
        
        Ok(windows)
    }
}

pub struct ProcessTracker {
    sys: System,
    active_apps: HashMap<String, AppInfo>,
    last_update: SystemTime,
    current_focused: Option<String>,
    window_detector: Arc<Mutex<Box<dyn WindowDetector>>>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        info!("Initializing ProcessTracker");
        
        let detector: Box<dyn WindowDetector> = if cfg!(target_os = "linux") {
            if let Ok(display) = std::env::var("WAYLAND_DISPLAY") {
                if !display.is_empty() {
                    info!("Using Wayland detector");
                    Box::new(WaylandDetector)
                } else {
                    info!("Using X11 detector");
                    Box::new(X11Detector)
                }
            } else {
                info!("Using X11 detector");
                Box::new(X11Detector)
            }
        } else {
            info!("Using X11 detector");
            Box::new(X11Detector)
        };
        
        Self {
            sys: System::new_all(),
            active_apps: HashMap::new(),
            last_update: SystemTime::now(),
            current_focused: None,
            window_detector: Arc::new(Mutex::new(detector)),
        }
    }

    pub fn update(&mut self) -> Result<()> {
        info!("Updating process tracker");
        self.sys.refresh_all();
        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_update)?;
        self.last_update = now;
        
        // Get the currently focused window
        if let Ok(detector) = self.window_detector.lock() {
            if let Ok(Some(window_info)) = detector.get_active_window() {
                info!("Current focused window: {:?}", window_info);
                
                // Update durations for all tracked apps
                for (name, info) in self.active_apps.iter_mut() {
                    let was_active = info.is_active;
                    info.is_active = Some(name.clone()) == Some(window_info.class.clone());
                    if info.is_active {
                        info.duration += elapsed;
                        info.window_title = window_info.title.clone();
                        info.is_minimized = window_info.is_minimized;
                        info.is_maximized = window_info.is_maximized;
                        info.is_fullscreen = window_info.is_fullscreen;
                        info.workspace = window_info.workspace;
                        info.monitor = window_info.monitor;
                        info.is_urgent = window_info.is_urgent;
                        info.is_skip_taskbar = window_info.is_skip_taskbar;
                        info!("Updated duration for {}: {} seconds (was active: {})", name, info.duration.as_secs(), was_active);
                    }
                }
                
                // Add new app if it's not tracked yet
                if !self.active_apps.contains_key(&window_info.class) {
                    info!("Adding new app to track: {} with title: {}", window_info.class, window_info.title);
                    self.active_apps.insert(
                        window_info.class.clone(),
                        AppInfo {
                            name: window_info.class.clone(),
                            window_title: window_info.title.clone(),
                            duration: Duration::from_secs(0),
                            is_active: true,
                            is_minimized: window_info.is_minimized,
                            is_maximized: window_info.is_maximized,
                            is_fullscreen: window_info.is_fullscreen,
                            workspace: window_info.workspace,
                            monitor: window_info.monitor,
                            is_urgent: window_info.is_urgent,
                            is_skip_taskbar: window_info.is_skip_taskbar,
                            app_id: window_info.app_id.clone(),
                            stable_sequence: window_info.stable_sequence,
                            last_updated: now,
                        },
                    );
                }
                
                self.current_focused = Some(window_info.class.clone());
            }
        }
        
        Ok(())
    }

    pub fn get_active_apps(&self) -> &HashMap<String, AppInfo> {
        &self.active_apps
    }
} 