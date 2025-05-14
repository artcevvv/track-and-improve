use anyhow::Result;
use chrono::{DateTime, Utc};
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sysinfo::{System, SystemExt};
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
    #[cfg(target_os = "linux")]
    x11_conn: Option<x11rb::rust_connection::RustConnection>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        let x11_conn = x11rb::connect(None).ok().map(|(conn, _)| conn);
        
        Self {
            sys: System::new_all(),
            active_apps: HashMap::new(),
            last_update: Utc::now(),
            current_focused: None,
            #[cfg(target_os = "linux")]
            x11_conn,
        }
    }

    pub fn update(&mut self) -> Result<()> {
        self.sys.refresh_all();
        let now = Utc::now();
        let elapsed = (now - self.last_update).num_seconds();
        
        // Get the currently focused window
        let (focused_app, window_title) = self.get_focused_app();
        debug!("Focused app: {:?}, Window title: {:?}", focused_app, window_title);
        
        // Update durations for all tracked apps
        for (name, info) in self.active_apps.iter_mut() {
            info.is_active = Some(name.clone()) == focused_app;
            if info.is_active {
                info.duration += elapsed;
                if let Some(title) = &window_title {
                    info.window_title = Some(title.clone());
                }
                debug!("Updated duration for {}: {} seconds", name, info.duration);
            }
        }
        
        // Add new app if it's not tracked yet
        if let Some(app_name) = &focused_app {
            if !self.active_apps.contains_key(app_name) {
                debug!("Adding new app to track: {}", app_name);
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
            }
        }
        
        self.current_focused = focused_app;
        self.last_update = now;
        Ok(())
    }

    fn get_focused_app(&self) -> (Option<String>, Option<String>) {
        #[cfg(target_os = "linux")]
        {
            if let Some(conn) = &self.x11_conn {
                if let Ok(setup) = conn.setup() {
                    let root = setup.default_screen().root;
                    let active_window = x11rb::protocol::xproto::get_input_focus(conn, ())
                        .ok()
                        .and_then(|reply| reply.reply().ok())
                        .map(|reply| reply.focus);

                    if let Some(window) = active_window {
                        // Get window title
                        let window_title = x11rb::protocol::xproto::get_property(
                            conn,
                            false,
                            window,
                            x11rb::protocol::xproto::ATOM_WM_NAME,
                            x11rb::protocol::xproto::ATOM_STRING,
                            0,
                            1024,
                        )
                        .ok()
                        .and_then(|reply| reply.reply().ok())
                        .and_then(|reply| String::from_utf8(reply.value).ok());

                        // Get window PID
                        if let Ok(pid) = Command::new("xprop")
                            .args(["-id", &format!("{}", window), "_NET_WM_PID"])
                            .output()
                        {
                            if let Ok(output) = String::from_utf8(pid.stdout) {
                                if let Some(pid_str) = output.split('=').nth(1) {
                                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                                        if let Some(process) = self.sys.process(sysinfo::Pid::from(pid as usize)) {
                                            let name = process.name().to_string();
                                            debug!("Found active process: {} with title: {:?}", name, window_title);
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
                        
                        debug!("Found active process on macOS: {} with title: {:?}", name, window_title);
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
                        
                        debug!("Found active process on Windows: {} with title: {:?}", name, window_title);
                        return (Some(name), window_title);
                    }
                }
            }
        }
        
        (None, None)
    }

    pub fn get_active_apps(&self) -> &HashMap<String, AppInfo> {
        &self.active_apps
    }
} 