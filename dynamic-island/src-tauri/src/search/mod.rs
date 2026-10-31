//! Search plumbing: voidtools Everything lookups plus browser launching.
//!
//! Everything is only reachable through one of two optional integrations:
//!   1. `es.exe` (the Everything command line tool)
//!   2. the Everything HTTP server (Tools -> Options -> HTTP Server)
//!
//! When neither is present the widget falls back to plain web searches, which
//! are just URLs opened in the browser the user picked in settings.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::settings::load_settings;
use crate::state::AppState;

const CONNECT_TIMEOUT_MS: u64 = 600;
const READ_TIMEOUT_MS: u64 = 2500;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchEngine {
    pub id: String,
    pub name: String,
    /// URL template; `{query}` is replaced with the encoded search term.
    pub url: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SearchResult {
    pub name: String,
    pub path: String,
    pub extension: String,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct BrowserOption {
    /// `default` or the absolute path of the browser executable.
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SearchStatus {
    pub everything: bool,
    /// `es` or `http` when Everything was located.
    pub everything_via: String,
    pub everything_detail: String,
    pub browsers: Vec<BrowserOption>,
}

// ---------------------------------------------------------------------------
// Everything
// ---------------------------------------------------------------------------

fn everything_exe_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for key in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Ok(root) = std::env::var(key) {
            candidates.push(Path::new(&root).join("Everything").join("Everything.exe"));
        }
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            Path::new(&local)
                .join("Programs")
                .join("Everything")
                .join("Everything.exe"),
        );
    }
    candidates
}

/// `es.exe` lives next to `Everything.exe` in portable installs and is
/// sometimes copied onto PATH. An explicit path from settings wins.
fn es_candidates(configured: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let configured = configured.trim();
    if !configured.is_empty() {
        candidates.push(PathBuf::from(configured));
    }
    for exe in everything_exe_candidates() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("es.exe"));
        }
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            Path::new(&local)
                .join("Programs")
                .join("Everything")
                .join("es.exe"),
        );
    }
    candidates
}

fn find_es(configured: &str) -> Option<PathBuf> {
    es_candidates(configured)
        .into_iter()
        .find(|path| path.is_file())
        .or_else(|| where_exe("es.exe"))
}

fn where_exe(name: &str) -> Option<PathBuf> {
    let output = hidden_command(Path::new("where")).arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next()?.trim().to_string();
    if first.is_empty() {
        None
    } else {
        Some(PathBuf::from(first))
    }
}

fn detect_everything(port: u16, es_path: &str) -> (bool, String, String) {
    if let Some(es) = find_es(es_path) {
        return (
            true,
            "es".to_string(),
            format!("es.exe at {}", es.display()),
        );
    }
    match http_get(port, "", 1) {
        Ok(_) => (
            true,
            "http".to_string(),
            format!("HTTP server on 127.0.0.1:{port}"),
        ),
        Err(error) => (false, String::new(), error),
    }
}

fn es_search(es: &Path, query: &str, limit: u32) -> Result<Vec<SearchResult>, String> {
    let output = hidden_command(es)
        .arg("-n")
        .arg(limit.to_string())
        .arg("-json")
        .arg(query)
        .output()
        .map_err(|e| format!("cannot run es.exe: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "es.exe returned an error".to_string()
        } else {
            stderr
        });
    }
    parse_results(&String::from_utf8_lossy(&output.stdout))
}

fn http_search(port: u16, query: &str, limit: u32) -> Result<Vec<SearchResult>, String> {
    parse_results(&http_get(port, query, limit)?)
}

fn http_get(port: u16, query: &str, limit: u32) -> Result<String, String> {
    let addr = ("127.0.0.1", port)
        .to_socket_addrs()
        .map_err(|e| format!("Everything HTTP server unreachable: {e}"))?
        .next()
        .ok_or_else(|| "Everything HTTP server unreachable".to_string())?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(CONNECT_TIMEOUT_MS))
        .map_err(|_| format!("Everything HTTP server is not running on 127.0.0.1:{port}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)));

    let uri = format!(
        "/?search={}&json=1&count={}&path_column=1&size_column=1&date_modified_column=1",
        percent_encode(query),
        limit
    );
    let request = format!(
        "GET {uri} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("Everything request failed: {e}"))?;

    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    if raw.is_empty() {
        return Err("Everything returned no data".to_string());
    }

    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "Everything returned a malformed HTTP response".to_string())?;
    let head = String::from_utf8_lossy(&raw[..split]).to_lowercase();
    if let Some(status) = head.lines().next() {
        if !status.contains(" 200") {
            return Err(format!("Everything HTTP error: {}", status.trim()));
        }
    }

    let body = &raw[split + 4..];
    let body = if head.contains("transfer-encoding: chunked") {
        decode_chunked(body).ok_or_else(|| "Everything returned broken chunked data".to_string())?
    } else {
        body.to_vec()
    };
    Ok(String::from_utf8_lossy(&body).into_owned())
}

fn decode_chunked(body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut index = 0usize;
    loop {
        let line_end = body[index..]
            .windows(2)
            .position(|window| window == b"\r\n")?
            + index;
        let size_line = std::str::from_utf8(&body[index..line_end]).ok()?;
        let size = usize::from_str_radix(size_line.split(';').next()?.trim(), 16).ok()?;
        index = line_end + 2;
        if size == 0 {
            return Some(out);
        }
        let end = index.checked_add(size)?;
        if end > body.len() {
            return None;
        }
        out.extend_from_slice(&body[index..end]);
        index = end + 2;
        if index >= body.len() {
            return Some(out);
        }
    }
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[derive(Deserialize)]
struct RawResult {
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    size: serde_json::Value,
    #[serde(default)]
    date_modified: serde_json::Value,
    #[serde(default)]
    extension: String,
}

fn parse_results(json: &str) -> Result<Vec<SearchResult>, String> {
    let value: serde_json::Value = serde_json::from_str(json.trim())
        .map_err(|e| format!("Everything returned invalid JSON: {e}"))?;
    // `es.exe -json` prints a bare array; the HTTP server wraps it in an object.
    let items = match value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Object(map) => map
            .get("results")
            .and_then(|results| results.as_array())
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    Ok(items
        .into_iter()
        .filter_map(|item| {
            let raw: RawResult = serde_json::from_value(item).ok()?;
            let path = full_path(&raw.path, &raw.name);
            if path.is_empty() {
                return None;
            }
            let name = if raw.name.is_empty() {
                file_name_of(&path)
            } else {
                raw.name
            };
            let extension = if raw.extension.is_empty() {
                extension_of(&path)
            } else {
                raw.extension.to_lowercase()
            };
            Some(SearchResult {
                name,
                path,
                extension,
                size: json_u64(&raw.size),
                modified: json_string(&raw.date_modified),
            })
        })
        .collect())
}

/// The HTTP server reports the containing folder, `es.exe` the full path.
fn full_path(path: &str, name: &str) -> String {
    if path.is_empty() {
        return name.to_string();
    }
    if name.is_empty() {
        return path.to_string();
    }
    if Path::new(path)
        .file_name()
        .map(|existing| existing.eq_ignore_ascii_case(name))
        .unwrap_or(false)
    {
        return path.to_string();
    }
    let separator = if path.ends_with('\\') || path.ends_with('/') {
        ""
    } else {
        "\\"
    };
    format!("{path}{separator}{name}")
}

fn file_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn extension_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn json_u64(value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::Number(number) => number.as_u64().unwrap_or(0),
        serde_json::Value::String(text) => text.trim().parse::<u64>().unwrap_or(0),
        _ => 0,
    }
}

fn json_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Browsers
// ---------------------------------------------------------------------------

fn env_path(key: &str, segments: &[&str]) -> Option<PathBuf> {
    let root = std::env::var(key).ok()?;
    Some(
        segments
            .iter()
            .fold(PathBuf::from(root), |acc, part| acc.join(part)),
    )
}

fn first_existing(candidates: Vec<Option<PathBuf>>) -> Option<PathBuf> {
    candidates
        .into_iter()
        .flatten()
        .find(|candidate| candidate.is_file())
}

fn detect_browsers() -> Vec<BrowserOption> {
    let candidates: Vec<(&str, Vec<Option<PathBuf>>)> = vec![
        (
            "Microsoft Edge",
            vec![
                env_path(
                    "ProgramFiles(x86)",
                    &["Microsoft", "Edge", "Application", "msedge.exe"],
                ),
                env_path(
                    "ProgramFiles",
                    &["Microsoft", "Edge", "Application", "msedge.exe"],
                ),
            ],
        ),
        (
            "Google Chrome",
            vec![
                env_path(
                    "ProgramFiles",
                    &["Google", "Chrome", "Application", "chrome.exe"],
                ),
                env_path(
                    "ProgramFiles(x86)",
                    &["Google", "Chrome", "Application", "chrome.exe"],
                ),
                env_path(
                    "LOCALAPPDATA",
                    &["Google", "Chrome", "Application", "chrome.exe"],
                ),
            ],
        ),
        (
            "Mozilla Firefox",
            vec![
                env_path("ProgramFiles", &["Mozilla Firefox", "firefox.exe"]),
                env_path("ProgramFiles(x86)", &["Mozilla Firefox", "firefox.exe"]),
            ],
        ),
        (
            "Brave",
            vec![
                env_path(
                    "ProgramFiles",
                    &["BraveSoftware", "Brave-Browser", "Application", "brave.exe"],
                ),
                env_path(
                    "LOCALAPPDATA",
                    &["BraveSoftware", "Brave-Browser", "Application", "brave.exe"],
                ),
            ],
        ),
        (
            "Vivaldi",
            vec![
                env_path("LOCALAPPDATA", &["Vivaldi", "Application", "vivaldi.exe"]),
                env_path(
                    "ProgramFiles",
                    &["Vivaldi", "Application", "vivaldi.exe"],
                ),
            ],
        ),
        (
            "Opera",
            vec![
                env_path("LOCALAPPDATA", &["Programs", "Opera", "opera.exe"]),
                env_path("ProgramFiles", &["Opera", "opera.exe"]),
            ],
        ),
        (
            "Opera GX",
            vec![
                env_path("LOCALAPPDATA", &["Programs", "Opera GX", "opera.exe"]),
                env_path("ProgramFiles", &["Opera GX", "opera.exe"]),
            ],
        ),
        (
            "Zen Browser",
            vec![env_path(
                "LOCALAPPDATA",
                &["Programs", "Zen Browser", "zen.exe"],
            )],
        ),
    ];

    let mut browsers = vec![BrowserOption {
        id: "default".to_string(),
        name: "System default".to_string(),
    }];
    let mut seen = std::collections::HashSet::new();
    for (name, options) in candidates {
        if let Some(path) = first_existing(options) {
            let id = path.to_string_lossy().into_owned();
            if seen.insert(id.to_lowercase()) {
                browsers.push(BrowserOption {
                    id,
                    name: name.to_string(),
                });
            }
        }
    }
    browsers
}

fn hidden_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Do not flash a console window for es.exe / browser launches.
        command.creation_flags(0x0800_0000);
    }
    command
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn live_settings(state: &AppState) -> Option<(u16, String, String)> {
    let db = state.db.lock().ok()?;
    let settings = load_settings(&db);
    Some((
        settings.search_everything_port.clamp(1, 65_535) as u16,
        settings.search_es_path,
        settings.search_browser,
    ))
}

#[tauri::command]
pub async fn search_status(state: State<'_, AppState>) -> Result<SearchStatus, String> {
    let (port, es_path, _) = live_settings(&state).ok_or_else(|| "db lock failed".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let (everything, via, detail) = detect_everything(port, &es_path);
        SearchStatus {
            everything,
            everything_via: via,
            everything_detail: detail,
            browsers: detect_browsers(),
        }
    })
    .await
    .map_err(|e| format!("search status failed: {e}"))
}

#[tauri::command]
pub async fn everything_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<SearchResult>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let (port, es_path, _) = live_settings(&state).ok_or_else(|| "db lock failed".to_string())?;
    let limit = limit.unwrap_or(40).clamp(1, 200);

    tauri::async_runtime::spawn_blocking(move || match find_es(&es_path) {
        Some(es) => es_search(&es, &query, limit),
        None => http_search(port, &query, limit),
    })
    .await
    .map_err(|e| format!("search failed: {e}"))?
}

#[tauri::command]
pub fn open_external(
    app: tauri::AppHandle,
    url: String,
    browser: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    let url = url.trim().to_string();

    // The update notice hands us either an https link or a Store deep link, so
    // both arrive through this one command.
    if url.starts_with("ms-windows-store://") {
        return open_system_uri(&url);
    }

    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("only http(s) links can be opened".to_string());
    }

    let chosen = browser
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "default")
        .or_else(|| {
            live_settings(&state)
                .map(|(_, _, saved)| saved)
                .filter(|value| !value.is_empty() && value != "default")
        });

    if let Some(executable) = chosen {
        let path = PathBuf::from(&executable);
        if path.is_file() {
            // Handing the URL to an already running instance opens a new tab
            // instead of starting a second browser process.
            if hidden_command(&path).arg(&url).spawn().is_ok() {
                return Ok(());
            }
        }
    }

    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("cannot open link: {e}"))
}

/// Hands a non-http URI (`ms-windows-store:`) to the shell, which routes it to
/// the Store app. The opener plugin only speaks URLs a browser can take.
fn open_system_uri(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let operation = HSTRING::from("open");
        let target = HSTRING::from(url);
        let result = unsafe {
            ShellExecuteW(
                None,
                &operation,
                &target,
                windows::core::PCWSTR::null(),
                windows::core::PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecuteW reports success as anything above 32.
        if result.0 as isize > 32 {
            return Ok(());
        }
        Err(format!("cannot open {url}"))
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("this link is only available on Windows".to_string())
    }
}

#[tauri::command]
pub fn open_search_result(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("nothing to open".to_string());
    }
    if !Path::new(trimmed).exists() {
        return Err("that file no longer exists".to_string());
    }
    app.opener()
        .open_path(trimmed, None::<&str>)
        .map_err(|e| format!("cannot open file: {e}"))
}
