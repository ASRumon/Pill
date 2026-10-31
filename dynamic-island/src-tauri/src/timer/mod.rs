use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimerState {
    pub mode: String,
    pub seconds_left: u32,
    pub running: bool,
}

#[tauri::command]
pub fn start_timer(mode: String) -> TimerState {
    let seconds_left = if mode == "break" { 5 * 60 } else { 25 * 60 };
    TimerState {
        mode,
        seconds_left,
        running: true,
    }
}

#[tauri::command]
pub fn pause_timer(mode: String, seconds_left: u32) -> TimerState {
    TimerState {
        mode,
        seconds_left,
        running: false,
    }
}

#[tauri::command]
pub fn reset_timer(mode: String) -> TimerState {
    let seconds_left = if mode == "break" { 5 * 60 } else { 25 * 60 };
    TimerState {
        mode,
        seconds_left,
        running: false,
    }
}
