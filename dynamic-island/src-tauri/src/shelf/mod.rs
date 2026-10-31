//! Drop Inbox: everything dragged into the island becomes one record that can
//! be previewed and exported back out as a real file.
//!
//! Content kinds:
//! - `text`  — stored inline in SQLite, previewed as text
//! - `url`   — stored inline, previewed as a link
//! - `html`  — stored inline, embedded `data:` images split into assets
//! - `image` — bytes live in `<app data>/assets`, previewed from a thumbnail
//! - `file`  — the original path is referenced, never copied

use crate::state::{AppState, ShelfEntry};
use base64::Engine;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

#[cfg(target_os = "windows")]
mod drop_target;
mod linkmeta;
#[cfg(target_os = "windows")]
mod shell;

pub const KIND_TEXT: &str = "text";
pub const KIND_URL: &str = "url";
pub const KIND_HTML: &str = "html";
pub const KIND_IMAGE: &str = "image";
pub const KIND_FILE: &str = "file";
pub const KIND_FOLDER: &str = "folder";

const IMAGE_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"];
const MAX_THUMB_SOURCE_BYTES: u64 = 40 * 1024 * 1024;
const MAX_INLINE_IMAGE_BYTES: usize = 24 * 1024 * 1024;

const SELECT_SHELF: &str = "SELECT id, kind, path, content, name, extension, mime, size_bytes, \
     thumb_path, thumb_kind, preview, assets, pinned, created_at, parent_id, custom_title, \
     meta_title, meta_description, meta_site, meta_image, meta_favicon, meta_fetched_at \
     FROM shelf_items";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShelfItem {
    pub id: String,
    pub kind: String,
    /// Abs path for `file` / `image`, empty for inline kinds.
    pub path: String,
    /// Inline payload for `text`, `url` and `html`.
    pub content: String,
    pub name: String,
    pub extension: String,
    pub mime: String,
    pub size_bytes: u64,
    pub thumb_path: String,
    pub thumb_kind: String,
    pub preview: String,
    pub assets: Vec<String>,
    pub pinned: bool,
    pub created_at: i64,
    /// Folder id when the item sits inside an organisational folder.
    pub parent_id: String,
    /// Title the user typed; empty when the title is automatic.
    pub custom_title: String,
    pub meta_title: String,
    pub meta_description: String,
    pub meta_site: String,
    pub meta_image: String,
    pub meta_favicon: String,
    pub meta_fetched_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShelfHistoryEntry {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExportedFile {
    pub id: String,
    pub path: String,
    pub name: String,
}

/// One drag operation, normalised by the drop target or the frontend.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DropPayload {
    /// Real file paths (Explorer, or browsers that hand over temp files).
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// base64 image (data URL or raw) from the drag/clipboard.
    #[serde(default)]
    pub image_base64: Option<String>,
    #[serde(default)]
    pub image_mime: Option<String>,
}

impl From<ShelfEntry> for ShelfItem {
    fn from(entry: ShelfEntry) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind,
            path: entry.path,
            content: entry.content,
            name: entry.name,
            extension: entry.extension,
            mime: entry.mime,
            size_bytes: entry.size_bytes,
            thumb_path: entry.thumb_path,
            thumb_kind: entry.thumb_kind,
            preview: entry.preview,
            assets: entry.assets,
            pinned: entry.pinned,
            created_at: entry.created_at,
            parent_id: entry.parent_id,
            custom_title: entry.custom_title,
            meta_title: entry.meta_title,
            meta_description: entry.meta_description,
            meta_site: entry.meta_site,
            meta_image: entry.meta_image,
            meta_favicon: entry.meta_favicon,
            meta_fetched_at: entry.meta_fetched_at,
        }
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ShelfEntry> {
    let assets: String = row.get(11)?;
    Ok(ShelfEntry {
        id: row.get(0)?,
        kind: row.get(1)?,
        path: row.get(2)?,
        content: row.get(3)?,
        name: row.get(4)?,
        extension: row.get(5)?,
        mime: row.get(6)?,
        size_bytes: row.get::<_, i64>(7)? as u64,
        thumb_path: row.get(8)?,
        thumb_kind: row.get(9)?,
        preview: row.get(10)?,
        assets: serde_json::from_str(&assets).unwrap_or_default(),
        pinned: row.get::<_, i32>(12)? != 0,
        created_at: row.get(13)?,
        parent_id: row.get(14)?,
        custom_title: row.get(15)?,
        meta_title: row.get(16)?,
        meta_description: row.get(17)?,
        meta_site: row.get(18)?,
        meta_image: row.get(19)?,
        meta_favicon: row.get(20)?,
        meta_fetched_at: row.get(21)?,
    })
}

pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn new_id(kind: &str, stamp: i64) -> String {
    format!("{kind}-{stamp}-{}", now_nanos() % 1_000_000)
}

fn preview_dir() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push("dynamic-island-thumbs");
    dir
}

pub(crate) fn assets_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("app data folder unavailable: {e}"))?;
    dir.push("assets");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create assets folder: {e}"))?;
    Ok(dir)
}

fn export_root() -> Result<PathBuf, String> {
    let mut dir = std::env::temp_dir();
    dir.push("dynamic-island-export");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create export folder: {e}"))?;
    Ok(dir)
}

fn extension_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn mime_of(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// Pinned first, then newest-to-oldest so the latest drop sits on top.
pub(crate) fn sort_shelf(shelf: &mut [ShelfEntry]) {
    shelf.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
}

fn sanitize_name(input: &str, fallback: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = trimmed.trim_matches('.').trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.chars().take(60).collect()
    }
}

fn text_preview(text: &str, limit: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(limit).collect()
}

fn looks_like_url(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && !trimmed.contains('\n')
        && !trimmed.contains(char::is_whitespace)
        && (trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("www."))
}

fn host_of(url: &str) -> String {
    let without_scheme = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url);
    without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(without_scheme)
        .trim_start_matches("www.")
        .to_string()
}

fn unique_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{stem}.{extension}"));
    let mut index = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({index}).{extension}"));
        index += 1;
    }
    candidate
}

/// Same idea for directories, which have no extension to append.
fn unique_dir(dir: &Path, stem: &str) -> PathBuf {
    let mut candidate = dir.join(stem);
    let mut index = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({index})"));
        index += 1;
    }
    candidate
}

/// An organisational folder: a shelf item that groups other items without
/// touching the filesystem.
fn folder_entry(name: &str, now: i64) -> ShelfEntry {
    ShelfEntry {
        id: new_id(KIND_FOLDER, now),
        kind: KIND_FOLDER.to_string(),
        path: String::new(),
        content: String::new(),
        name: sanitize_name(name, "New folder"),
        extension: String::new(),
        mime: "inode/directory".to_string(),
        size_bytes: 0,
        thumb_path: String::new(),
        thumb_kind: String::new(),
        preview: String::new(),
        assets: Vec::new(),
        pinned: false,
        created_at: now,
        parent_id: String::new(),
        custom_title: String::new(),
        meta_title: String::new(),
        meta_description: String::new(),
        meta_site: String::new(),
        meta_image: String::new(),
        meta_favicon: String::new(),
        meta_fetched_at: 0,
    }
}

fn next_folder_name(shelf: &[ShelfEntry]) -> String {
    const BASE: &str = "New folder";
    if !shelf
        .iter()
        .any(|entry| entry.kind == KIND_FOLDER && entry.name == BASE)
    {
        return BASE.to_string();
    }
    for index in 2..10_000 {
        let candidate = format!("{BASE} {index}");
        if !shelf
            .iter()
            .any(|entry| entry.kind == KIND_FOLDER && entry.name == candidate)
        {
            return candidate;
        }
    }
    BASE.to_string()
}

/// Recursively pulls every file out of a directory that was dropped on the
/// island. Depth and count are capped so a huge tree cannot stall the app.
fn collect_directory(
    path: &Path,
    now: i64,
    depth: usize,
    budget: &mut usize,
    out: &mut Vec<ShelfEntry>,
) {
    if depth > 4 || *budget == 0 {
        return;
    }
    let Ok(read) = std::fs::read_dir(path) else {
        return;
    };
    for entry in read.flatten() {
        if *budget == 0 {
            return;
        }
        let child = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            collect_directory(&child, now, depth + 1, budget, out);
        } else if metadata.is_file() {
            if let Ok(item) = file_entry(&child.to_string_lossy(), now) {
                out.push(item);
                *budget -= 1;
            }
        }
    }
}

/// Photos get resized locally; everything else asks Explorer for its real
/// thumbnail, then for its real icon.
fn make_preview(path: &str, extension: &str, size_bytes: u64, stamp: u128) -> (String, String) {
    if IMAGE_EXTENSIONS.contains(&extension) && size_bytes <= MAX_THUMB_SOURCE_BYTES {
        if let Ok(image) = image::open(path) {
            let dir = preview_dir();
            if std::fs::create_dir_all(&dir).is_ok() {
                let target = dir.join(format!("thumb-{stamp}.png"));
                if image
                    .thumbnail(256, 256)
                    .save_with_format(&target, image::ImageFormat::Png)
                    .is_ok()
                {
                    return (target.to_string_lossy().to_string(), "cover".to_string());
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    if let Some(preview) = shell::preview(path, extension, stamp) {
        return (preview.path, preview.kind.to_string());
    }

    (String::new(), String::new())
}

// ---------------------------------------------------------------- entries ---

fn file_entry(path: &str, now: i64) -> Result<ShelfEntry, String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("cannot read file: {e}"))?;
    if !metadata.is_file() {
        return Err("Only files can be added to the inbox.".to_string());
    }

    let extension = extension_of(path);
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());
    let kind = if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        KIND_IMAGE
    } else {
        KIND_FILE
    };
    let (thumb_path, thumb_kind) = make_preview(path, &extension, metadata.len(), now_nanos());

    Ok(ShelfEntry {
        id: new_id(kind, now),
        kind: kind.to_string(),
        path: path.to_string(),
        content: String::new(),
        name,
        extension: extension.clone(),
        mime: mime_of(&extension).to_string(),
        size_bytes: metadata.len(),
        thumb_path,
        thumb_kind,
        preview: String::new(),
        assets: Vec::new(),
        pinned: false,
        created_at: now,
        parent_id: String::new(),
        custom_title: String::new(),
        meta_title: String::new(),
        meta_description: String::new(),
        meta_site: String::new(),
        meta_image: String::new(),
        meta_favicon: String::new(),
        meta_fetched_at: 0,
    })
}

fn text_entry(text: &str, now: i64) -> ShelfEntry {
    let stem = text
        .lines()
        .next()
        .map(|line| sanitize_name(line, "note"))
        .unwrap_or_else(|| "note".to_string());

    ShelfEntry {
        id: new_id(KIND_TEXT, now),
        kind: KIND_TEXT.to_string(),
        path: String::new(),
        content: text.to_string(),
        name: stem,
        extension: "txt".to_string(),
        mime: "text/plain".to_string(),
        size_bytes: text.len() as u64,
        thumb_path: String::new(),
        thumb_kind: String::new(),
        preview: text_preview(text, 160),
        assets: Vec::new(),
        pinned: false,
        created_at: now,
        parent_id: String::new(),
        custom_title: String::new(),
        meta_title: String::new(),
        meta_description: String::new(),
        meta_site: String::new(),
        meta_image: String::new(),
        meta_favicon: String::new(),
        meta_fetched_at: 0,
    }
}

fn url_entry(url: &str, title: Option<&str>, now: i64) -> ShelfEntry {
    let url = url.trim();
    let host = host_of(url);
    let name = title
        .map(|value| sanitize_name(value, &host))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| host.clone());

    ShelfEntry {
        id: new_id(KIND_URL, now),
        kind: KIND_URL.to_string(),
        path: String::new(),
        content: url.to_string(),
        name,
        extension: "url".to_string(),
        mime: "text/uri-list".to_string(),
        size_bytes: url.len() as u64,
        thumb_path: String::new(),
        thumb_kind: String::new(),
        preview: host,
        assets: Vec::new(),
        pinned: false,
        created_at: now,
        parent_id: String::new(),
        custom_title: String::new(),
        meta_title: String::new(),
        meta_description: String::new(),
        meta_site: String::new(),
        meta_image: String::new(),
        meta_favicon: String::new(),
        meta_fetched_at: 0,
    }
}

/// Pulls `data:image/...;base64,...` payloads out of HTML into real asset files
/// and rewrites the references so the document stays valid after export.
fn extract_data_images(html: &str, assets: &Path, stem: &str) -> (String, Vec<String>) {
    const MARKER: &str = "data:image/";
    let mut output = String::with_capacity(html.len());
    let mut saved = Vec::new();
    let bytes = html.as_bytes();
    let mut cursor = 0;

    while let Some(found) = html[cursor..].find(MARKER) {
        let start = cursor + found;
        let mut end = start;
        while end < bytes.len()
            && !matches!(bytes[end], b'"' | b'\'' | b')' | b' ' | b'\n' | b'\r' | b'>')
        {
            end += 1;
        }

        let uri = &html[start..end];
        let meta_end = uri.find(',').unwrap_or(0);
        let meta = &uri[..meta_end];
        let extension = if meta.contains("png") {
            "png"
        } else if meta.contains("jpeg") || meta.contains("jpg") {
            "jpg"
        } else if meta.contains("webp") {
            "webp"
        } else if meta.contains("gif") {
            "gif"
        } else {
            ""
        };

        let mut written = false;
        if !extension.is_empty() && meta.contains("base64") && meta_end > 0 {
            if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(&uri[meta_end + 1..]) {
                let name = format!("{stem}-{}.{extension}", saved.len() + 1);
                let file = assets.join(&name);
                if std::fs::write(&file, data).is_ok() {
                    output.push_str(&html[cursor..start]);
                    output.push_str(&format!("./assets/{name}"));
                    saved.push(file.to_string_lossy().to_string());
                    written = true;
                }
            }
        }

        if !written {
            output.push_str(&html[cursor..end]);
        }
        cursor = end;
    }

    output.push_str(&html[cursor..]);
    (output, saved)
}

fn html_entry(assets: &Path, html: &str, text: Option<&str>, now: i64) -> Result<ShelfEntry, String> {
    let stem = sanitize_name(
        text.and_then(|value| value.lines().next()).unwrap_or("page"),
        &format!("page-{now}"),
    );

    let (rewritten, asset_paths) = extract_data_images(html, assets, &stem);

    Ok(ShelfEntry {
        id: new_id(KIND_HTML, now),
        kind: KIND_HTML.to_string(),
        path: String::new(),
        content: rewritten.clone(),
        name: stem,
        extension: "html".to_string(),
        mime: "text/html".to_string(),
        size_bytes: rewritten.len() as u64,
        thumb_path: asset_paths
            .first()
            .map(|first| {
                let extension = extension_of(first);
                make_preview(first, &extension, 0, now_nanos()).0
            })
            .unwrap_or_default(),
        thumb_kind: if asset_paths.is_empty() {
            String::new()
        } else {
            "cover".to_string()
        },
        preview: text_preview(text.unwrap_or(&rewritten), 160),
        assets: asset_paths,
        pinned: false,
        created_at: now,
        parent_id: String::new(),
        custom_title: String::new(),
        meta_title: String::new(),
        meta_description: String::new(),
        meta_site: String::new(),
        meta_image: String::new(),
        meta_favicon: String::new(),
        meta_fetched_at: 0,
    })
}

fn image_entry(
    assets: &Path,
    payload: &str,
    mime: Option<&str>,
    title: Option<&str>,
    now: i64,
) -> Result<ShelfEntry, String> {
    let encoded = payload
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(payload)
        .trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("cannot decode image: {e}"))?;
    if bytes.len() > MAX_INLINE_IMAGE_BYTES {
        return Err("Dropped image is too large.".to_string());
    }

    let extension = match mime.unwrap_or("") {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ if bytes.starts_with(&[0x89, b'P', b'N', b'G']) => "png",
        _ if bytes.starts_with(&[0xFF, 0xD8]) => "jpg",
        _ => "png",
    };

    let id = new_id(KIND_IMAGE, now);
    let file = assets.join(format!("{id}.{extension}"));
    std::fs::write(&file, &bytes).map_err(|e| format!("cannot store image: {e}"))?;

    let path = file.to_string_lossy().to_string();
    let (thumb_path, thumb_kind) = make_preview(&path, extension, bytes.len() as u64, now_nanos());

    Ok(ShelfEntry {
        id,
        kind: KIND_IMAGE.to_string(),
        path,
        content: String::new(),
        name: sanitize_name(title.unwrap_or("image"), "image"),
        extension: extension.to_string(),
        mime: format!("image/{extension}"),
        size_bytes: bytes.len() as u64,
        thumb_path,
        thumb_kind,
        preview: String::new(),
        assets: Vec::new(),
        pinned: false,
        created_at: now,
        parent_id: String::new(),
        custom_title: String::new(),
        meta_title: String::new(),
        meta_description: String::new(),
        meta_site: String::new(),
        meta_image: String::new(),
        meta_favicon: String::new(),
        meta_fetched_at: 0,
    })
}

/// Converts one drag/clipboard payload into inbox records.
pub fn ingest_payload(assets: &Path, payload: DropPayload) -> Result<Vec<ShelfEntry>, String> {
    let now = now_millis();
    let mut entries = Vec::new();

    for path in payload.files.iter() {
        match std::fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => {
                // A dropped folder contributes every file it contains.
                let mut budget = 200usize;
                collect_directory(Path::new(path), now, 0, &mut budget, &mut entries);
            }
            _ => {
                if let Ok(entry) = file_entry(path, now) {
                    entries.push(entry);
                }
            }
        }
    }
    if !entries.is_empty() {
        // A real file drop wins: browsers often also expose text/HTML for it.
        return Ok(entries);
    }

    if let Some(image) = payload.image_base64.as_deref().filter(|data| !data.is_empty()) {
        if let Ok(entry) = image_entry(assets, image, payload.image_mime.as_deref(), payload.text.as_deref(), now) {
            return Ok(vec![entry]);
        }
    }

    if let Some(url) = payload.url.as_deref().map(str::trim).filter(|url| !url.is_empty()) {
        let title = payload.text.as_deref().filter(|text| text.trim() != url);
        return Ok(vec![url_entry(url, title, now)]);
    }

    if let Some(html) = payload.html.as_deref().filter(|html| !html.trim().is_empty()) {
        return Ok(vec![html_entry(assets, html, payload.text.as_deref(), now)?]);
    }

    if let Some(text) = payload.text.as_deref().filter(|text| !text.trim().is_empty()) {
        if looks_like_url(text) {
            return Ok(vec![url_entry(text, None, now)]);
        }
        return Ok(vec![text_entry(text, now)]);
    }

    Ok(entries)
}

// ------------------------------------------------------------------ store ---

pub(crate) fn store_insert(state: &AppState, entry: &ShelfEntry) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    db.execute(
        "INSERT OR REPLACE INTO shelf_items
         (id, kind, path, content, name, extension, mime, size_bytes, thumb_path, thumb_kind, preview, assets, pinned, created_at,
          parent_id, custom_title, meta_title, meta_description, meta_site, meta_image, meta_favicon, meta_fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
        params![
            entry.id,
            entry.kind,
            entry.path,
            entry.content,
            entry.name,
            entry.extension,
            entry.mime,
            entry.size_bytes as i64,
            entry.thumb_path,
            entry.thumb_kind,
            entry.preview,
            serde_json::to_string(&entry.assets).unwrap_or_else(|_| "[]".to_string()),
            entry.pinned as i32,
            entry.created_at,
            entry.parent_id,
            entry.custom_title,
            entry.meta_title,
            entry.meta_description,
            entry.meta_site,
            entry.meta_image,
            entry.meta_favicon,
            entry.meta_fetched_at,
        ],
    )
    .map(|_| ())
    .map_err(|e| format!("inbox save failed: {e}"))
}

fn store_delete(state: &AppState, id: &str) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    db.execute("DELETE FROM shelf_items WHERE id = ?1", params![id])
        .map(|_| ())
        .map_err(|e| format!("inbox delete failed: {e}"))
}

/// Appends to the drop log that survives the item itself being cleared.
fn store_history(state: &AppState, entry: &ShelfEntry) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    db.execute(
        "INSERT INTO shelf_history (name, path, kind, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![entry.name, entry.path, entry.kind, entry.created_at],
    )
    .map(|_| ())
    .map_err(|e| format!("history write failed: {e}"))
}

fn prune_expired(state: &AppState) {
    let hours = {
        let Ok(db) = state.db.lock() else { return };
        crate::settings::load_settings(&db).shelf_retention_hours
    };
    if hours <= 0 {
        return;
    }

    let cutoff = now_millis() - hours * 3_600_000;
    if let Ok(db) = state.db.lock() {
        // Items inside a pinned folder belong to that folder and stay.
        let _ = db.execute(
            "DELETE FROM shelf_items WHERE pinned = 0 AND created_at < ?1
             AND (parent_id = '' OR parent_id NOT IN (SELECT id FROM shelf_items WHERE pinned = 1))",
            params![cutoff],
        );
    }
}

/// Deletes the files an entry generated: its inline image, HTML assets, the
/// per-item thumbnail and the cached link preview. Shared Explorer icon files
/// (`icon-png.png`) are left alone because other tiles reuse them.
pub(crate) fn remove_generated_files(entry: &ShelfEntry) {
    if entry.kind == KIND_IMAGE && !entry.path.is_empty() {
        let _ = std::fs::remove_file(&entry.path);
    }
    for asset in &entry.assets {
        let _ = std::fs::remove_file(asset);
    }
    for path in [
        entry.thumb_path.as_str(),
        entry.meta_image.as_str(),
        entry.meta_favicon.as_str(),
    ] {
        if path.is_empty() {
            continue;
        }
        let Some(name) = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
        else {
            continue;
        };
        if name.starts_with("thumb-") || name.starts_with("link-") {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Removes an entry and any assets it owns.
fn remove_entry(state: &AppState, entry: &ShelfEntry) {
    remove_generated_files(entry);
    let _ = store_delete(state, &entry.id);
}

fn insert_entry(state: &AppState, entry: ShelfEntry) -> Result<ShelfItem, String> {
    {
        let mut shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;

        let duplicate = shelf.iter().find(|item| {
            if !entry.path.is_empty() && !item.path.is_empty() {
                item.path == entry.path
            } else {
                item.kind == entry.kind && item.content == entry.content
            }
        });

        if let Some(existing) = duplicate {
            return Ok(existing.clone().into());
        }

        shelf.push(entry.clone());
        sort_shelf(&mut shelf);
    }

    store_insert(state, &entry)?;
    let _ = store_history(state, &entry);
    Ok(entry.into())
}

// ---------------------------------------------------------------- export ---

/// Writes a real file for every item, in a throwaway export directory.
///
/// `all` is the whole shelf so a folder can resolve the items it contains.
pub fn export_entries(entries: &[ShelfEntry], all: &[ShelfEntry]) -> Result<Vec<ExportedFile>, String> {
    let dir = export_root()?.join(format!("export-{}", now_millis()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create export folder: {e}"))?;
    export_into(entries, all, &dir, false)
}

/// `copy_files` is false at the top level (dropped files keep referencing the
/// original path) and true inside a folder export, where the dropped folder is
/// expected to contain real copies.
fn export_into(
    entries: &[ShelfEntry],
    all: &[ShelfEntry],
    dir: &Path,
    copy_files: bool,
) -> Result<Vec<ExportedFile>, String> {
    let mut exported = Vec::new();
    for entry in entries {
        let stem = sanitize_name(&entry.name, &entry.id);
        match entry.kind.as_str() {
            KIND_FOLDER => {
                let folder_dir = unique_dir(dir, &stem);
                std::fs::create_dir_all(&folder_dir)
                    .map_err(|e| format!("cannot export folder: {e}"))?;
                let children: Vec<ShelfEntry> = all
                    .iter()
                    .filter(|child| child.parent_id == entry.id)
                    .cloned()
                    .collect();
                export_into(&children, all, &folder_dir, true)?;
                exported.push(ExportedFile {
                    id: entry.id.clone(),
                    path: folder_dir.to_string_lossy().to_string(),
                    name: stem,
                });
            }
            // Reference the original file instead of duplicating it.
            KIND_FILE if !copy_files => exported.push(ExportedFile {
                id: entry.id.clone(),
                path: entry.path.clone(),
                name: entry.name.clone(),
            }),
            KIND_FILE => {
                let extension = extension_of(&entry.path);
                let extension = if extension.is_empty() {
                    "bin".to_string()
                } else {
                    extension
                };
                let target = unique_path(dir, &stem, &extension);
                std::fs::copy(&entry.path, &target)
                    .map_err(|e| format!("cannot export file: {e}"))?;
                exported.push(ExportedFile {
                    id: entry.id.clone(),
                    path: target.to_string_lossy().to_string(),
                    name: format!("{stem}.{extension}"),
                });
            }
            KIND_IMAGE => {
                let extension = if entry.extension.is_empty() {
                    "png"
                } else {
                    entry.extension.as_str()
                };
                let target = unique_path(dir, &stem, extension);
                std::fs::copy(&entry.path, &target)
                    .map_err(|e| format!("cannot export image: {e}"))?;
                exported.push(ExportedFile {
                    id: entry.id.clone(),
                    path: target.to_string_lossy().to_string(),
                    name: format!("{stem}.{extension}"),
                });
            }
            KIND_TEXT => {
                let target = unique_path(dir, &stem, "txt");
                std::fs::write(&target, &entry.content)
                    .map_err(|e| format!("cannot export text: {e}"))?;
                exported.push(ExportedFile {
                    id: entry.id.clone(),
                    path: target.to_string_lossy().to_string(),
                    name: format!("{stem}.txt"),
                });
            }
            KIND_URL => {
                let target = unique_path(dir, &stem, "url");
                let body = format!("[InternetShortcut]\r\nURL={}\r\n", entry.content.trim());
                std::fs::write(&target, body)
                    .map_err(|e| format!("cannot export link: {e}"))?;
                exported.push(ExportedFile {
                    id: entry.id.clone(),
                    path: target.to_string_lossy().to_string(),
                    name: format!("{stem}.url"),
                });
            }
            KIND_HTML => {
                // Keep the document next to its images so relative links hold.
                let bundle = unique_path(dir, &stem, "html")
                    .parent()
                    .map(|parent| parent.join(format!("{stem}-files")))
                    .unwrap_or_else(|| dir.join(format!("{stem}-files")));
                std::fs::create_dir_all(&bundle).map_err(|e| format!("cannot export html: {e}"))?;
                for asset in &entry.assets {
                    if let Some(file_name) = Path::new(asset).file_name() {
                        let _ = std::fs::copy(asset, bundle.join(file_name));
                    }
                }
                let target = bundle.join(format!("{stem}.html"));
                std::fs::write(&target, &entry.content)
                    .map_err(|e| format!("cannot export html: {e}"))?;
                exported.push(ExportedFile {
                    id: entry.id.clone(),
                    path: target.to_string_lossy().to_string(),
                    name: format!("{stem}.html"),
                });
            }
            _ => {}
        }
    }

    Ok(exported)
}

// -------------------------------------------------------------- commands ---

#[tauri::command]
pub fn get_shelf_items(state: State<AppState>) -> Result<Vec<ShelfItem>, String> {
    let shelf = state
        .shelf
        .lock()
        .map_err(|_| "inbox lock failed".to_string())?;
    Ok(shelf.iter().cloned().map(ShelfItem::from).collect())
}

/// Turns one or more drops/pastes into inbox items.
#[tauri::command]
pub async fn ingest_drop(
    app: AppHandle,
    payloads: Vec<DropPayload>,
    state: State<'_, AppState>,
) -> Result<Vec<ShelfItem>, String> {
    let worker_assets = assets_dir(&app)?;
    let entries = tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::new();
        let mut errors = Vec::new();
        for payload in payloads {
            match ingest_payload(&worker_assets, payload) {
                Ok(mut items) => out.append(&mut items),
                Err(e) => errors.push(e),
            }
        }
        if out.is_empty() && !errors.is_empty() {
            return Err(errors.join("; "));
        }
        Ok(out)
    })
    .await
    .map_err(|e| format!("ingest task failed: {e}"))??;

    let mut saved = Vec::new();
    for entry in entries {
        saved.push(insert_entry(&state, entry)?);
    }
    if !saved.is_empty() {
        let _ = app.emit("shelf-changed", ());
    }

    // Link previews are fetched off the hot path; each item repaints itself
    // when its metadata lands.
    let link_ids: Vec<String> = saved
        .iter()
        .filter(|item| item.kind == KIND_URL)
        .map(|item| item.id.clone())
        .collect();
    if !link_ids.is_empty() {
        let handle = app.clone();
        std::thread::spawn(move || {
            let state = handle.state::<AppState>();
            for id in link_ids {
                let _ = linkmeta::refresh(&handle, state.inner(), &id, false);
            }
        });
    }

    Ok(saved)
}

#[tauri::command]
pub fn export_shelf_items(ids: Vec<String>, state: State<AppState>) -> Result<Vec<ExportedFile>, String> {
    let (entries, all): (Vec<ShelfEntry>, Vec<ShelfEntry>) = {
        let shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        (
            shelf
                .iter()
                .filter(|entry| ids.contains(&entry.id))
                .cloned()
                .collect(),
            shelf.clone(),
        )
    };

    if entries.is_empty() {
        return Err("Nothing to export.".to_string());
    }
    export_entries(&entries, &all)
}

#[tauri::command]
pub fn save_shelf_item(path: String, target: String) -> Result<String, String> {
    if path == target {
        return Ok(target);
    }
    std::fs::copy(&path, &target).map_err(|e| format!("cannot save file: {e}"))?;
    Ok(target)
}

#[tauri::command]
pub fn remove_shelf_item(id: String, state: State<AppState>) -> Result<bool, String> {
    let removed = {
        let mut shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        let position = shelf.iter().position(|entry| entry.id == id);
        position.map(|index| shelf.remove(index))
    };

    match removed {
        Some(entry) if entry.kind == KIND_FOLDER => {
            // Deleting a folder only removes the container: its contents go
            // back to the main list and the files themselves are untouched.
            let promoted: Vec<ShelfEntry> = {
                let Ok(mut shelf) = state.shelf.lock() else {
                    return Ok(false);
                };
                let mut promoted = Vec::new();
                for child in shelf.iter_mut().filter(|child| child.parent_id == entry.id) {
                    child.parent_id.clear();
                    promoted.push(child.clone());
                }
                promoted
            };
            for child in promoted {
                let _ = store_insert(&state, &child);
            }
            remove_entry(&state, &entry);
            Ok(true)
        }
        Some(entry) => {
            remove_entry(&state, &entry);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Clears the scratch area but keeps pinned items, which are meant to stay.
/// Returns how many entries were removed (files and attachments included).
pub(crate) fn clear_unpinned_shelf(state: &AppState) -> Result<usize, String> {
    let dropped: Vec<ShelfEntry> = {
        let mut shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        let pinned_ids: Vec<String> = shelf
            .iter()
            .filter(|entry| entry.pinned)
            .map(|entry| entry.id.clone())
            .collect();
        let keep: Vec<ShelfEntry> = shelf
            .iter()
            .filter(|entry| entry.pinned || pinned_ids.contains(&entry.parent_id))
            .cloned()
            .collect();
        // Items sitting inside a pinned folder belong to it and survive too.
        let dropped: Vec<ShelfEntry> = shelf
            .iter()
            .filter(|entry| !entry.pinned && !pinned_ids.contains(&entry.parent_id))
            .cloned()
            .collect();
        *shelf = keep;
        dropped
    };

    for entry in &dropped {
        remove_generated_files(entry);
    }

    if !dropped.is_empty() {
        let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
        db.execute(
            "DELETE FROM shelf_items WHERE pinned = 0
             AND (parent_id = '' OR parent_id NOT IN (SELECT id FROM shelf_items WHERE pinned = 1))",
            params![],
        )
        .map_err(|e| format!("inbox clear failed: {e}"))?;
    }
    Ok(dropped.len())
}

#[tauri::command]
pub fn clear_shelf_items(state: State<AppState>) -> Result<bool, String> {
    Ok(clear_unpinned_shelf(&state)? > 0)
}

#[tauri::command]
pub fn set_shelf_pinned(
    id: String,
    pinned: bool,
    state: State<AppState>,
) -> Result<ShelfItem, String> {
    let updated = {
        let mut shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        let entry = shelf
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| "inbox item not found".to_string())?;
        entry.pinned = pinned;
        let snapshot = entry.clone();
        sort_shelf(&mut shelf);
        snapshot
    };

    store_insert(&state, &updated)?;
    Ok(updated.into())
}

/// Display name an item falls back to when it has no custom title.
fn auto_name(entry: &ShelfEntry) -> String {
    if !entry.meta_title.trim().is_empty() {
        return sanitize_name(&entry.meta_title, &host_of(&entry.content));
    }
    if entry.kind == KIND_URL && !entry.meta_site.trim().is_empty() {
        return entry.meta_site.trim().to_string();
    }
    match entry.kind.as_str() {
        KIND_URL => host_of(&entry.content),
        KIND_FILE | KIND_IMAGE => Path::new(&entry.path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| entry.name.clone()),
        _ => entry.name.clone(),
    }
}

fn move_into_folder(
    shelf: &mut [ShelfEntry],
    changed: &mut Vec<ShelfEntry>,
    id: &str,
    folder_id: &str,
) {
    if id == folder_id {
        return;
    }
    let Some(entry) = shelf.iter_mut().find(|entry| entry.id == id) else {
        return;
    };
    // Folders stay flat: nesting is not part of the model.
    if entry.kind == KIND_FOLDER || entry.parent_id == folder_id {
        return;
    }
    entry.parent_id = folder_id.to_string();
    changed.push(entry.clone());
}

fn persist_changed(state: &AppState, changed: &[ShelfEntry]) {
    let mut seen: Vec<String> = Vec::new();
    for entry in changed {
        if seen.contains(&entry.id) {
            continue;
        }
        seen.push(entry.id.clone());
        let _ = store_insert(state, entry);
    }
}

/// Groups items into an organisational folder.
///
/// * `target_id` is the tile the drag ended on. When it is a folder the items
///   are moved into it; otherwise a new folder is created holding the target
///   and the dragged items. An empty target creates an empty folder.
#[tauri::command]
pub fn create_shelf_folder(
    app: AppHandle,
    target_id: String,
    source_ids: Vec<String>,
    state: State<AppState>,
) -> Result<Vec<ShelfItem>, String> {
    let changed = {
        let mut shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        let now = now_millis();
        let target = if target_id.is_empty() {
            None
        } else {
            Some(
                shelf
                    .iter()
                    .find(|entry| entry.id == target_id)
                    .cloned()
                    .ok_or_else(|| "shelf item not found".to_string())?,
            )
        };

        let mut changed: Vec<ShelfEntry> = Vec::new();
        let folder = match &target {
            Some(entry) if entry.kind == KIND_FOLDER => entry.clone(),
            other => {
                let entry = folder_entry(&next_folder_name(&shelf), now);
                shelf.push(entry.clone());
                changed.push(entry.clone());
                if let Some(target) = other {
                    move_into_folder(&mut shelf, &mut changed, &target.id, &entry.id);
                }
                entry
            }
        };

        for id in &source_ids {
            move_into_folder(&mut shelf, &mut changed, id, &folder.id);
        }

        sort_shelf(&mut shelf);
        if !changed.iter().any(|entry| entry.id == folder.id) {
            changed.push(folder);
        }
        changed
    };

    persist_changed(&state, &changed);
    let _ = app.emit("shelf-changed", ());
    Ok(changed.into_iter().map(ShelfItem::from).collect())
}

/// Moves one item in or out of a folder (`parent_id` empty = main list).
#[tauri::command]
pub fn move_shelf_item(
    app: AppHandle,
    id: String,
    parent_id: String,
    state: State<AppState>,
) -> Result<ShelfItem, String> {
    let updated = {
        let mut shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        if !parent_id.is_empty()
            && !shelf
                .iter()
                .any(|entry| entry.id == parent_id && entry.kind == KIND_FOLDER)
        {
            return Err("folder not found".to_string());
        }
        let entry = shelf
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| "shelf item not found".to_string())?;
        if entry.kind == KIND_FOLDER {
            return Err("folders cannot be nested".to_string());
        }
        entry.parent_id = parent_id;
        let snapshot = entry.clone();
        sort_shelf(&mut shelf);
        snapshot
    };

    store_insert(&state, &updated)?;
    let _ = app.emit("shelf-changed", ());
    Ok(updated.into())
}

/// Renames a link, folder or file tile. An empty title restores the automatic
/// name, and a later metadata refresh never overwrites a custom title.
#[tauri::command]
pub fn rename_shelf_item(
    app: AppHandle,
    id: String,
    title: String,
    state: State<AppState>,
) -> Result<ShelfItem, String> {
    let updated = {
        let mut shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        let entry = shelf
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| "inbox item not found".to_string())?;
        let trimmed: String = title.trim().chars().take(80).collect();
        let fallback = auto_name(entry);
        entry.custom_title = trimmed.clone();
        entry.name = if trimmed.is_empty() {
            fallback.clone()
        } else {
            sanitize_name(&trimmed, &fallback)
        };
        let snapshot = entry.clone();
        sort_shelf(&mut shelf);
        snapshot
    };

    store_insert(&state, &updated)?;
    let _ = app.emit("shelf-changed", ());
    Ok(updated.into())
}

/// Copies the given items to the clipboard as a real file list.
#[tauri::command]
pub fn copy_shelf_items(ids: Vec<String>, state: State<AppState>) -> Result<usize, String> {
    let (entries, all): (Vec<ShelfEntry>, Vec<ShelfEntry>) = {
        let shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        (
            shelf
                .iter()
                .filter(|entry| ids.contains(&entry.id))
                .cloned()
                .collect(),
            shelf.clone(),
        )
    };

    let exported = export_entries(&entries, &all)?;
    let paths: Vec<String> = exported.into_iter().map(|file| file.path).collect();

    #[cfg(target_os = "windows")]
    {
        shell::copy_files(&paths)?;
        Ok(paths.len())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = paths;
        Err("Copying files is only supported on Windows.".to_string())
    }
}

#[tauri::command]
pub fn open_shelf_item(app: AppHandle, id: String, state: State<AppState>) -> Result<(), String> {
    let (entry, all) = {
        let shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        let entry = shelf
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
            .ok_or_else(|| "inbox item not found".to_string())?;
        (entry, shelf.clone())
    };

    match entry.kind.as_str() {
        KIND_URL => app
            .opener()
            .open_url(entry.content.trim(), None::<&str>)
            .map_err(|e| format!("cannot open link: {e}")),
        // Opening a folder shows its contents in Explorer by materialising it
        // in a throwaway export directory.
        KIND_FOLDER => {
            let exported = export_entries(std::slice::from_ref(&entry), &all)?;
            let path = exported
                .first()
                .map(|file| file.path.clone())
                .ok_or_else(|| "nothing to open".to_string())?;
            app.opener()
                .open_path(path, None::<&str>)
                .map_err(|e| format!("cannot open folder: {e}"))
        }
        KIND_TEXT | KIND_HTML => {
            let exported = export_entries(std::slice::from_ref(&entry), &all)?;
            let path = exported
                .first()
                .map(|file| file.path.clone())
                .ok_or_else(|| "nothing to open".to_string())?;
            app.opener()
                .open_path(path, None::<&str>)
                .map_err(|e| format!("cannot open item: {e}"))
        }
        _ => app
            .opener()
            .open_path(entry.path, None::<&str>)
            .map_err(|e| format!("cannot open file: {e}")),
    }
}

/// Re-reads a link's title, description, image and favicon.
#[tauri::command]
pub async fn refresh_link_metadata(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> Result<ShelfItem, String> {
    let _ = &state;
    let handle = app.clone();
    let entry = tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        linkmeta::refresh(&handle, state.inner(), &id, true)
    })
    .await
    .map_err(|error| format!("metadata task failed: {error}"))??;
    Ok(entry.into())
}

#[tauri::command]
pub fn list_shelf_history(state: State<AppState>) -> Result<Vec<ShelfHistoryEntry>, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    let mut stmt = db
        .prepare(
            "SELECT id, name, path, kind, created_at FROM shelf_history ORDER BY id DESC LIMIT 60",
        )
        .map_err(|e| format!("history prepare failed: {e}"))?;
    let rows = stmt
        .query_map(params![], |row| {
            Ok(ShelfHistoryEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                kind: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| format!("history query failed: {e}"))?;

    Ok(rows.flatten().collect())
}

/// Reloads stored items (and rebuilds missing previews) without blocking start-up.
pub fn restore_shelf(app: AppHandle) {
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        prune_expired(&state);

        let mut restored: Vec<ShelfEntry> = {
            let Ok(db) = state.db.lock() else { return };
            let Ok(mut stmt) = db.prepare(SELECT_SHELF) else {
                return;
            };
            let mapped = stmt.query_map(params![], map_row);
            match mapped {
                Ok(rows) => rows.flatten().collect(),
                Err(_) => Vec::new(),
            }
        };

        // Files that vanished since the last run are dropped from the inbox.
        // A pinned item whose file is gone is reported once and then removed —
        // keeping a tile that cannot open anything is worse than losing it.
        let (present, missing): (Vec<ShelfEntry>, Vec<ShelfEntry>) = restored
            .into_iter()
            .partition(|entry| entry.path.is_empty() || Path::new(&entry.path).exists());
        for entry in &missing {
            let _ = store_delete(&state, &entry.id);
        }
        restored = present;

        // Thumbnails and cached link images live in the temp folder, so they can
        // be cleared by the OS between runs. Forget the dead paths so the tiles
        // fall back to their glyph and missing previews are rebuilt below.
        for entry in restored.iter_mut() {
            if !entry.thumb_path.is_empty() && !Path::new(&entry.thumb_path).exists() {
                entry.thumb_path.clear();
                entry.thumb_kind.clear();
            }
            if !entry.meta_image.is_empty() && !Path::new(&entry.meta_image).exists() {
                entry.meta_image.clear();
            }
            if !entry.meta_favicon.is_empty() && !Path::new(&entry.meta_favicon).exists() {
                entry.meta_favicon.clear();
            }
        }

        let pending: Vec<(String, String, String, u64)> = {
            let Ok(mut shelf) = state.shelf.lock() else {
                return;
            };
            *shelf = restored;
            sort_shelf(&mut shelf);
            shelf
                .iter()
                .filter(|entry| entry.thumb_path.is_empty() && !entry.path.is_empty())
                .map(|entry| {
                    (
                        entry.id.clone(),
                        entry.path.clone(),
                        entry.extension.clone(),
                        entry.size_bytes,
                    )
                })
                .collect()
        };
        let _ = app.emit("shelf-changed", ());

        for (id, path, extension, size) in pending {
            let (thumb_path, thumb_kind) = make_preview(&path, &extension, size, now_nanos());
            if thumb_path.is_empty() {
                continue;
            }
            let updated = {
                let Ok(mut shelf) = state.shelf.lock() else {
                    return;
                };
                shelf
                    .iter_mut()
                    .find(|entry| entry.id == id)
                    .map(|entry| {
                        entry.thumb_path = thumb_path;
                        entry.thumb_kind = thumb_kind;
                        entry.clone()
                    })
            };
            if let Some(entry) = updated {
                let _ = store_insert(&state, &entry);
            }
            let _ = app.emit("shelf-changed", ());
        }
    });
}

/// Installs the native drop target so text/HTML/image drags reach the inbox.
pub fn start_drop_target(app: AppHandle) {
    #[cfg(target_os = "windows")]
    drop_target::install(app);
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_1PX: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    fn test_assets() -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("di-inbox-test-{}", now_nanos()));
        std::fs::create_dir_all(&dir).expect("assets dir");
        dir
    }

    fn temp_file(name: &str, bytes: &[u8]) -> String {
        let mut path = std::env::temp_dir();
        path.push(format!("di-inbox-{}-{name}", now_nanos()));
        std::fs::write(&path, bytes).expect("write temp file");
        path.to_string_lossy().to_string()
    }

    #[test]
    fn dropping_a_directory_expands_every_file_inside() {
        let source = test_assets();
        std::fs::write(source.join("one.txt"), b"one").expect("write");
        std::fs::write(source.join("two.txt"), b"two").expect("write");

        let payload = DropPayload {
            files: vec![source.to_string_lossy().to_string()],
            ..Default::default()
        };

        let items = ingest_payload(&test_assets(), payload).expect("ingest");
        assert_eq!(items.len(), 2, "both files inside the folder are captured");
        assert!(items.iter().all(|item| item.kind == KIND_FILE));
    }

    #[test]
    fn exporting_a_folder_materialises_its_children() {
        let assets = test_assets();
        let first = temp_file("bundle-a.txt", b"alpha");
        let second = temp_file("bundle-b.txt", b"beta");

        let folder = folder_entry("Bundle", now_millis());
        let mut child_a = file_entry(&first, now_millis()).expect("entry");
        child_a.parent_id = folder.id.clone();
        let mut child_b = file_entry(&second, now_millis()).expect("entry");
        child_b.parent_id = folder.id.clone();
        let _ = &assets;

        let all = vec![folder.clone(), child_a, child_b];
        let exported = export_entries(std::slice::from_ref(&folder), &all).expect("export");
        assert_eq!(exported.len(), 1);

        let folder_dir = PathBuf::from(&exported[0].path);
        assert!(folder_dir.is_dir(), "a real directory is produced");
        assert_eq!(
            std::fs::read_dir(&folder_dir).expect("read export").count(),
            2,
            "both files are copied into the exported folder"
        );
    }

    #[test]
    fn captures_plain_text_inline() {
        let assets = test_assets();
        let payload = DropPayload {
            text: Some("hello\nworld".to_string()),
            ..Default::default()
        };

        let items = ingest_payload(&assets, payload).expect("ingest");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, KIND_TEXT);
        assert_eq!(items[0].content, "hello\nworld");
        assert!(items[0].path.is_empty(), "text must not create a file");
        assert!(!items[0].preview.is_empty());
        assert_eq!(items[0].mime, "text/plain");
    }

    #[test]
    fn captures_url_with_host_preview() {
        let assets = test_assets();
        let payload = DropPayload {
            url: Some("https://news.ycombinator.com/item?id=1".to_string()),
            ..Default::default()
        };

        let items = ingest_payload(&assets, payload).expect("ingest");
        assert_eq!(items[0].kind, KIND_URL);
        assert_eq!(items[0].content, "https://news.ycombinator.com/item?id=1");
        assert_eq!(items[0].preview, "news.ycombinator.com");
        assert!(items[0].path.is_empty());
    }

    #[test]
    fn bare_url_text_becomes_a_link() {
        let assets = test_assets();
        let payload = DropPayload {
            text: Some("https://example.com/page".to_string()),
            ..Default::default()
        };

        let items = ingest_payload(&assets, payload).expect("ingest");
        assert_eq!(items[0].kind, KIND_URL);
    }

    #[test]
    fn captures_image_into_assets() {
        let assets = test_assets();
        let payload = DropPayload {
            image_base64: Some(PNG_1PX.to_string()),
            image_mime: Some("image/png".to_string()),
            ..Default::default()
        };

        let items = ingest_payload(&assets, payload).expect("ingest");
        assert_eq!(items[0].kind, KIND_IMAGE);
        let path = Path::new(&items[0].path);
        assert!(path.exists(), "image bytes must be written to assets");
        assert!(
            Path::new(&items[0].path).starts_with(&assets),
            "image must live in the managed assets directory"
        );
        assert_eq!(items[0].extension, "png");
        assert_eq!(items[0].size_bytes, 70);
        assert!(!items[0].thumb_path.is_empty(), "image needs a thumbnail");
    }

    #[test]
    fn captures_html_and_extracts_embedded_images() {
        let assets = test_assets();
        let html = format!(
            "<p>rich <b>text</b></p><img src=\"data:image/png;base64,{PNG_1PX}\">"
        );
        let payload = DropPayload {
            html: Some(html),
            text: Some("rich text".to_string()),
            ..Default::default()
        };

        let items = ingest_payload(&assets, payload).expect("ingest");
        assert_eq!(items[0].kind, KIND_HTML);
        assert_eq!(items[0].assets.len(), 1, "embedded image becomes an asset");
        assert!(items[0].content.contains("./assets/"), "src must be rewritten");
        assert!(!items[0].content.contains("data:image"));
        assert!(Path::new(&items[0].assets[0]).exists());
    }

    #[test]
    fn captures_multiple_files_by_reference() {
        let assets = test_assets();
        let first = temp_file("note.txt", b"hello");
        let second = temp_file("picture.png", &[0x89, b'P', b'N', b'G']);

        let payload = DropPayload {
            files: vec![first.clone(), second.clone()],
            ..Default::default()
        };

        let items = ingest_payload(&assets, payload).expect("ingest");
        assert_eq!(items.len(), 2, "both files must land in the inbox");
        assert_eq!(items[0].path, first, "original file is referenced, not copied");
        assert_eq!(items[0].kind, KIND_FILE);
        assert_eq!(items[1].kind, KIND_IMAGE);
        assert!(Path::new(&first).exists());

        let _ = std::fs::remove_file(&first);
        let _ = std::fs::remove_file(&second);
    }

    #[test]
    fn exports_every_kind_to_a_real_file() {
        let assets = test_assets();
        let mut entries = Vec::new();
        entries.extend(
            ingest_payload(
                &assets,
                DropPayload {
                    text: Some("export me".to_string()),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        entries.extend(
            ingest_payload(
                &assets,
                DropPayload {
                    url: Some("https://example.com".to_string()),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        entries.extend(
            ingest_payload(
                &assets,
                DropPayload {
                    image_base64: Some(PNG_1PX.to_string()),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        entries.extend(
            ingest_payload(
                &assets,
                DropPayload {
                    html: Some(format!("<img src=\"data:image/png;base64,{PNG_1PX}\">")),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

        let exported = export_entries(&entries, &entries).expect("export");
        assert_eq!(exported.len(), 4);
        for file in &exported {
            assert!(Path::new(&file.path).exists(), "{} missing", file.name);
        }
        assert!(exported[0].name.ends_with(".txt"));
        assert!(exported[1].name.ends_with(".url"));
        assert!(exported[2].name.ends_with(".png"));
        assert!(exported[3].name.ends_with(".html"));
    }

    /// Exercises the real Explorer icon/thumbnail pipeline through COM + GDI.
    #[test]
    fn extracts_a_readable_shell_preview() {
        let path = temp_file("shell-test.txt", b"hello from the island");

        let (thumb, kind) = make_preview(
            &path,
            "txt",
            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            now_nanos(),
        );

        assert!(!thumb.is_empty(), "no preview was produced for a .txt file");
        assert_eq!(kind, "contain", "shell previews must render contained");

        let image = image::open(&thumb).expect("preview file is not a readable image");
        assert!(image.width() > 0 && image.height() > 0);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&thumb);
    }
}
