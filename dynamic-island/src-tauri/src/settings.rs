use crate::search::SearchEngine;
use crate::startup;
use crate::state::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

/// Engine ids that ship with the app. Custom engines may not reuse them.
const BUILTIN_ENGINE_IDS: [&str; 3] = ["google", "youtube", "duckduckgo"];
const MAX_CUSTOM_ENGINES: usize = 20;
const MAX_ENGINE_NAME: usize = 32;

const TEXT_FORMATS: &[&str] = &[
    "txt", "md", "html", "json", "csv", "xml", "yml", "log", "ini", "css", "js", "ts", "py", "sql",
    "bat", "ps1", "sh",
];
const IMAGE_FORMATS: [&str; 3] = ["png", "jpg", "webp"];

/// Modifier combos that temporarily make the island click-through.
const CLICK_THROUGH_CHOICES: [&str; 5] = [
    "ctrl+alt",
    "ctrl+shift",
    "alt+shift",
    "ctrl+alt+shift",
    "off",
];

/// Island surface presets: the translucent glass look, pure black, or a
/// user-tuned surface.
const BACKGROUND_CHOICES: [&str; 3] = ["glass", "amoled", "custom"];
/// Custom surface opacity bounds, in percent.
const MIN_OPACITY: i64 = 10;
const MAX_OPACITY: i64 = 100;
/// Custom surface backdrop blur bounds, in CSS pixels.
const MAX_BLUR: i64 = 40;

/// Maps a shortcut choice to its modifier bitmask: 1 = Ctrl, 2 = Alt, 4 = Shift.
pub fn shortcut_mask(shortcut: &str) -> u8 {
    let mut mask = 0u8;
    if shortcut.contains("ctrl") {
        mask |= 1;
    }
    if shortcut.contains("alt") {
        mask |= 2;
    }
    if shortcut.contains("shift") {
        mask |= 4;
    }
    mask
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub default_text_format: String,
    pub default_image_format: String,
    pub minified_island: bool,
    /// Hours before unpinned shelf items expire. `0` means never.
    pub shelf_retention_hours: i64,
    /// `ctrl+alt`, `ctrl+shift`, `alt+shift`, `ctrl+alt+shift` or `off`.
    pub click_through_shortcut: String,
    /// Active web search engine: a builtin id or one of `search_engines`.
    #[serde(default)]
    pub search_engine: String,
    /// User-defined engines. Builtins are hard coded on the frontend.
    #[serde(default)]
    pub search_engines: Vec<SearchEngine>,
    /// `default` or the absolute path of the browser executable.
    #[serde(default)]
    pub search_browser: String,
    /// Everything HTTP server port.
    #[serde(default = "default_everything_port")]
    pub search_everything_port: i64,
    /// Optional absolute path to `es.exe`. Empty means auto-detect.
    #[serde(default)]
    pub search_es_path: String,
    /// Island surface: `glass`, `amoled` or `custom`.
    #[serde(default = "default_island_background")]
    pub island_background: String,
    /// Custom surface opacity, percent (10-100).
    #[serde(default = "default_island_opacity")]
    pub island_opacity: i64,
    /// Custom surface backdrop blur radius in px (0-40).
    #[serde(default = "default_island_blur")]
    pub island_blur: i64,
}

fn default_everything_port() -> i64 {
    80
}

fn default_island_background() -> String {
    "amoled".to_string()
}

fn default_island_opacity() -> i64 {
    92
}

fn default_island_blur() -> i64 {
    24
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_text_format: "txt".to_string(),
            default_image_format: "png".to_string(),
            minified_island: false,
            shelf_retention_hours: 24,
            click_through_shortcut: "ctrl+alt".to_string(),
            search_engine: "google".to_string(),
            search_engines: Vec::new(),
            search_browser: "default".to_string(),
            search_everything_port: default_everything_port(),
            search_es_path: String::new(),
            island_background: default_island_background(),
            island_opacity: default_island_opacity(),
            island_blur: default_island_blur(),
        }
    }
}

pub(crate) fn read_setting(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

pub(crate) fn write_setting(
    conn: &rusqlite::Connection,
    key: &str,
    value: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn load_settings(conn: &rusqlite::Connection) -> Settings {
    let fallback = Settings::default();

    Settings {
        default_text_format: read_setting(conn, "default_text_format")
            .filter(|value| TEXT_FORMATS.contains(&value.as_str()))
            .unwrap_or(fallback.default_text_format),
        default_image_format: read_setting(conn, "default_image_format")
            .filter(|value| IMAGE_FORMATS.contains(&value.as_str()))
            .unwrap_or(fallback.default_image_format),
        minified_island: read_setting(conn, "minified_island")
            .map(|value| value == "true")
            .unwrap_or(false),
        shelf_retention_hours: read_setting(conn, "shelf_retention_hours")
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|hours| (0..=720).contains(hours))
            .unwrap_or(fallback.shelf_retention_hours),
        click_through_shortcut: read_setting(conn, "click_through_shortcut")
            .filter(|value| CLICK_THROUGH_CHOICES.contains(&value.as_str()))
            .unwrap_or(fallback.click_through_shortcut),
        search_engine: read_setting(conn, "search_engine")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback.search_engine),
        search_engines: read_setting(conn, "search_engines")
            .and_then(|value| serde_json::from_str::<Vec<SearchEngine>>(&value).ok())
            .map(sanitize_engines)
            .unwrap_or_default(),
        search_browser: read_setting(conn, "search_browser")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback.search_browser),
        search_everything_port: read_setting(conn, "search_everything_port")
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|port| (1..=65_535).contains(port))
            .unwrap_or(fallback.search_everything_port),
        search_es_path: read_setting(conn, "search_es_path").unwrap_or_default(),
        island_background: read_setting(conn, "island_background")
            .filter(|value| BACKGROUND_CHOICES.contains(&value.as_str()))
            .unwrap_or_else(|| fallback.island_background.clone()),
        island_opacity: read_setting(conn, "island_opacity")
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| (MIN_OPACITY..=MAX_OPACITY).contains(value))
            .unwrap_or(fallback.island_opacity),
        island_blur: read_setting(conn, "island_blur")
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| (0..=MAX_BLUR).contains(value))
            .unwrap_or(fallback.island_blur),
    }
}

/// Keeps the stored engine list predictable: unique ids, http(s) templates that
/// actually carry a `{query}` placeholder, and no shadowing of builtins.
fn sanitize_engines(engines: Vec<SearchEngine>) -> Vec<SearchEngine> {
    let mut seen: Vec<String> = Vec::new();
    engines
        .into_iter()
        .filter_map(|engine| {
            let id = engine.id.trim().to_lowercase();
            let name = engine
                .name
                .trim()
                .chars()
                .take(MAX_ENGINE_NAME)
                .collect::<String>();
            let url = engine.url.trim().to_string();
            if id.is_empty()
                || name.is_empty()
                || BUILTIN_ENGINE_IDS.contains(&id.as_str())
                || seen.iter().any(|existing| existing == &id)
                || !(url.starts_with("http://") || url.starts_with("https://"))
                || !url.contains("{query}")
            {
                return None;
            }
            seen.push(id.clone());
            Some(SearchEngine { id, name, url })
        })
        .take(MAX_CUSTOM_ENGINES)
        .collect()
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<Settings, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    Ok(load_settings(&db))
}

#[tauri::command]
pub fn set_settings(
    app: tauri::AppHandle,
    settings: Settings,
    state: State<AppState>,
) -> Result<Settings, String> {
    let text_format = if TEXT_FORMATS.contains(&settings.default_text_format.as_str()) {
        settings.default_text_format
    } else {
        "txt".to_string()
    };

    let image_format = if IMAGE_FORMATS.contains(&settings.default_image_format.as_str()) {
        settings.default_image_format
    } else {
        "png".to_string()
    };

    let shortcut = if CLICK_THROUGH_CHOICES.contains(&settings.click_through_shortcut.as_str()) {
        settings.click_through_shortcut
    } else {
        "ctrl+alt".to_string()
    };

    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;

    write_setting(&db, "default_text_format", &text_format)
        .map_err(|e| format!("settings write failed: {e}"))?;
    write_setting(&db, "default_image_format", &image_format)
        .map_err(|e| format!("settings write failed: {e}"))?;
    write_setting(
        &db,
        "minified_island",
        if settings.minified_island {
            "true"
        } else {
            "false"
        },
    )
    .map_err(|e| format!("settings write failed: {e}"))?;

    let retention = settings.shelf_retention_hours.clamp(0, 720);
    write_setting(&db, "shelf_retention_hours", &retention.to_string())
        .map_err(|e| format!("settings write failed: {e}"))?;
    write_setting(&db, "click_through_shortcut", &shortcut)
        .map_err(|e| format!("settings write failed: {e}"))?;

    let engines = sanitize_engines(settings.search_engines);
    let engine_ids: Vec<String> = BUILTIN_ENGINE_IDS
        .iter()
        .map(|id| id.to_string())
        .chain(engines.iter().map(|engine| engine.id.clone()))
        .collect();
    let engine = if engine_ids.contains(&settings.search_engine) {
        settings.search_engine
    } else {
        "google".to_string()
    };
    let browser = if settings.search_browser.trim().is_empty() {
        "default".to_string()
    } else {
        settings.search_browser.trim().to_string()
    };
    let port = settings.search_everything_port.clamp(1, 65_535);
    let es_path = settings.search_es_path.trim().to_string();

    let background = if BACKGROUND_CHOICES.contains(&settings.island_background.as_str()) {
        settings.island_background
    } else {
        default_island_background()
    };
    let opacity = settings.island_opacity.clamp(MIN_OPACITY, MAX_OPACITY);
    let blur = settings.island_blur.clamp(0, MAX_BLUR);

    write_setting(&db, "island_background", &background)
        .map_err(|e| format!("settings write failed: {e}"))?;
    write_setting(&db, "island_opacity", &opacity.to_string())
        .map_err(|e| format!("settings write failed: {e}"))?;
    write_setting(&db, "island_blur", &blur.to_string())
        .map_err(|e| format!("settings write failed: {e}"))?;

    write_setting(&db, "search_engine", &engine)
        .map_err(|e| format!("settings write failed: {e}"))?;
    write_setting(
        &db,
        "search_engines",
        &serde_json::to_string(&engines).unwrap_or_else(|_| "[]".to_string()),
    )
    .map_err(|e| format!("settings write failed: {e}"))?;
    write_setting(&db, "search_browser", &browser)
        .map_err(|e| format!("settings write failed: {e}"))?;
    write_setting(&db, "search_everything_port", &port.to_string())
        .map_err(|e| format!("settings write failed: {e}"))?;
    write_setting(&db, "search_es_path", &es_path)
        .map_err(|e| format!("settings write failed: {e}"))?;

    let updated = Settings {
        default_text_format: text_format,
        default_image_format: image_format,
        minified_island: settings.minified_island,
        shelf_retention_hours: retention,
        click_through_shortcut: shortcut,
        search_engine: engine,
        search_engines: engines,
        search_browser: browser,
        search_everything_port: port,
        search_es_path: es_path,
        island_background: background,
        island_opacity: opacity,
        island_blur: blur,
    };

    // The watcher thread reads this mask on every tick.
    state.set_click_through_mods(shortcut_mask(&updated.click_through_shortcut));

    drop(db);
    let _ = app.emit("settings-changed", updated.clone());

    Ok(updated)
}

#[tauri::command]
pub fn set_minified_island(
    app: tauri::AppHandle,
    enabled: bool,
    state: State<AppState>,
) -> Result<(), String> {
    persist_minified_island(&state, enabled)?;
    let _ = app.emit("minified-island-changed", enabled);
    Ok(())
}

// ------------------------------------------------------------- autostart ---

/// Whether Windows launches the island when the user signs in. Read straight
/// from the `Run` registry entry, so it also reflects changes made in Task
/// Manager's Startup apps list.
#[tauri::command]
pub fn get_autostart() -> Result<bool, String> {
    Ok(startup::is_enabled())
}

#[tauri::command]
pub fn set_autostart(enabled: bool, state: State<AppState>) -> Result<bool, String> {
    startup::set_enabled(&state, enabled)
}

/// Switches the web search engine from the Search widget. Emits its own event
/// so widgets that watch `settings-changed` (format pickers) are left alone.
#[tauri::command]
pub fn set_search_engine(
    app: tauri::AppHandle,
    engine_id: String,
    state: State<AppState>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    let settings = load_settings(&db);
    let known = BUILTIN_ENGINE_IDS.contains(&engine_id.as_str())
        || settings
            .search_engines
            .iter()
            .any(|engine| engine.id == engine_id);
    let engine_id = if known {
        engine_id
    } else {
        "google".to_string()
    };
    write_setting(&db, "search_engine", &engine_id)
        .map_err(|e| format!("settings write failed: {e}"))?;
    drop(db);

    let _ = app.emit("search-engine-changed", engine_id.clone());
    Ok(engine_id)
}

pub fn persist_minified_island(state: &AppState, enabled: bool) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    write_setting(
        &db,
        "minified_island",
        if enabled { "true" } else { "false" },
    )
    .map_err(|e| format!("settings write failed: {e}"))
}
