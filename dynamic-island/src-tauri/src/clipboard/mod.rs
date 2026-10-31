use crate::state::AppState;
use arboard::Clipboard;
use base64::Engine;
use image::ImageFormat;
use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager, State};

/// Text clips are plain text regardless of extension, so any of these is a
/// valid drag-out target.
const TEXT_FORMATS: &[&str] = &[
    "txt", "md", "html", "json", "csv", "xml", "yml", "log", "ini", "css", "js", "ts", "py",
    "sql", "bat", "ps1", "sh",
];
const IMAGE_FORMATS: [&str; 3] = ["png", "jpg", "webp"];

const SELECT_ENTRY: &str =
    "SELECT id, kind, preview, content, image_path, created_at FROM clipboard_items";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClipboardEntry {
    pub id: i64,
    pub kind: String,
    pub preview: String,
    pub content: String,
    pub image_path: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SynthesizedFile {
    pub path: String,
    pub extension: String,
    pub kind: String,
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn map_entry(row: &Row<'_>) -> rusqlite::Result<ClipboardEntry> {
    Ok(ClipboardEntry {
        id: row.get(0)?,
        kind: row.get(1)?,
        preview: row.get(2)?,
        content: row.get(3)?,
        image_path: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn insert_entry(
    conn: &Connection,
    kind: &str,
    preview: &str,
    content: &str,
    image_path: &str,
) -> Result<ClipboardEntry, String> {
    // The watcher can observe one clipboard payload several times (multiple
    // formats, polls while the source app still owns the clipboard). Reuse the
    // existing row — and bump it to the top — instead of stacking duplicates.
    // For images `content` carries the payload hash.
    if !content.is_empty() {
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM clipboard_items WHERE kind = ?1 AND content = ?2 ORDER BY id DESC LIMIT 1",
                params![kind, content],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            let _ = conn.execute(
                "UPDATE clipboard_items SET created_at = CURRENT_TIMESTAMP WHERE id = ?1",
                params![id],
            );
            return conn
                .query_row(
                    &format!("{SELECT_ENTRY} WHERE id = ?1"),
                    params![id],
                    map_entry,
                )
                .map_err(|e| format!("clipboard reload failed: {e}"));
        }
    }

    conn.execute(
        "INSERT INTO clipboard_items (kind, preview, content, image_path) VALUES (?1, ?2, ?3, ?4)",
        params![kind, preview, content, image_path],
    )
    .map_err(|e| format!("clipboard insert failed: {e}"))?;

    let id = conn.last_insert_rowid();
    conn.query_row(
        &format!("{SELECT_ENTRY} WHERE id = ?1"),
        params![id],
        map_entry,
    )
    .map_err(|e| format!("clipboard reload failed: {e}"))
}

fn preview_of(content: &str) -> String {
    let collapsed: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(72).collect()
}

#[tauri::command]
pub fn list_clipboard_entries(state: State<AppState>) -> Result<Vec<ClipboardEntry>, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    let mut stmt = db
        .prepare(&format!("{SELECT_ENTRY} ORDER BY id DESC LIMIT 60"))
        .map_err(|e| format!("clipboard prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![], map_entry)
        .map_err(|e| format!("clipboard query failed: {e}"))?;

    // A single unreadable row (hand-edited or half-recovered database) must not
    // blank the whole panel, so bad rows are skipped instead of propagated.
    Ok(rows.flatten().collect())
}

#[tauri::command]
pub fn get_clipboard_entry(id: i64, state: State<AppState>) -> Result<ClipboardEntry, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    db.query_row(
        &format!("{SELECT_ENTRY} WHERE id = ?1"),
        params![id],
        map_entry,
    )
    .map_err(|e| format!("clipboard entry {id} not found: {e}"))
}

/// Puts a stored clip back on the system clipboard.
#[tauri::command]
pub fn copy_clipboard_entry(id: i64, state: State<AppState>) -> Result<(), String> {
    let entry: ClipboardEntry = {
        let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
        db.query_row(
            &format!("{SELECT_ENTRY} WHERE id = ?1"),
            params![id],
            map_entry,
        )
        .map_err(|e| format!("clipboard entry {id} not found: {e}"))?
    };

    let mut clipboard = Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;

    if entry.kind == "image" && !entry.image_path.is_empty() {
        let image = image::open(&entry.image_path)
            .map_err(|e| format!("cannot read clipboard image: {e}"))?
            .to_rgba8();
        let (width, height) = image.dimensions();
        clipboard
            .set_image(arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: image.into_raw().into(),
            })
            .map_err(|e| format!("clipboard write failed: {e}"))?;
    } else {
        clipboard
            .set_text(entry.content)
            .map_err(|e| format!("clipboard write failed: {e}"))?;
    }

    Ok(())
}

/// Saves edits as a brand new clip so the original capture is preserved.
#[tauri::command]
pub fn update_clipboard_entry(
    app: tauri::AppHandle,
    id: i64,
    content: String,
    state: State<AppState>,
) -> Result<ClipboardEntry, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;

    // Fail loudly if the source clip vanished.
    let _source: ClipboardEntry = db
        .query_row(
            &format!("{SELECT_ENTRY} WHERE id = ?1"),
            params![id],
            map_entry,
        )
        .map_err(|e| format!("clipboard entry {id} not found: {e}"))?;

    let preview = preview_of(&content);
    let entry = insert_entry(&db, "text", &preview, &content, "")?;
    drop(db);

    let _ = app.emit("clipboard-changed", entry.clone());
    Ok(entry)
}

#[tauri::command]
pub fn delete_clipboard_entry(id: i64, state: State<AppState>) -> Result<bool, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    let affected = db
        .execute("DELETE FROM clipboard_items WHERE id = ?1", params![id])
        .map_err(|e| format!("clipboard delete failed: {e}"))?;
    Ok(affected > 0)
}

/// Deletes every stored clip together with the image files they reference.
pub(crate) fn clear_clipboard_history(state: &AppState) -> Result<usize, String> {
    let paths: Vec<String> = {
        let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
        let Ok(mut stmt) =
            db.prepare("SELECT image_path FROM clipboard_items WHERE image_path <> ''")
        else {
            return Err("clipboard clear failed: cannot list images".to_string());
        };
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("clipboard clear failed: {e}"))?;
        rows.flatten().collect()
    };

    let affected = {
        let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
        db.execute("DELETE FROM clipboard_items", [])
            .map_err(|e| format!("clipboard clear failed: {e}"))?
    };

    for path in paths {
        let _ = fs::remove_file(path);
    }
    Ok(affected)
}

#[tauri::command]
pub fn clear_clipboard_entries(state: State<AppState>) -> Result<bool, String> {
    Ok(clear_clipboard_history(&state)? > 0)
}

fn export_dir() -> Result<PathBuf, String> {
    let mut dir = std::env::temp_dir();
    dir.push("dynamic-island-exports");
    fs::create_dir_all(&dir).map_err(|e| format!("export dir failed: {e}"))?;
    Ok(dir)
}

fn escape_html(content: &str) -> String {
    content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn text_payload(content: &str, ext: &str) -> String {
    if ext == "html" {
        format!(
            "<!doctype html>\n<html>\n<head><meta charset=\"utf-8\"><title>Clip</title></head>\n<body>\n<pre>{}</pre>\n</body>\n</html>\n",
            escape_html(content)
        )
    } else {
        content.to_string()
    }
}

/// Image clips dragged as a text format still need meaningful content, so they
/// become a path reference (or an embed) instead of an empty file.
fn image_text_payload(image_path: &str, ext: &str) -> String {
    let normalized = image_path.replace('\\', "/");
    match ext {
        "md" => format!("![clip]({normalized})\n"),
        "html" => format!(
            "<!doctype html>\n<html>\n<head><meta charset=\"utf-8\"><title>Clip</title></head>\n<body>\n<img src=\"file:///{}\" alt=\"clip\">\n</body>\n</html>\n",
            escape_html(&normalized)
        ),
        _ => format!("{image_path}\n"),
    }
}

fn export_text(content: &str, format: &str, index: usize) -> Result<SynthesizedFile, String> {
    let ext = if TEXT_FORMATS.contains(&format) {
        format
    } else {
        "txt"
    };

    export_text_payload(text_payload(content, ext), ext, index)
}

fn export_text_payload(
    payload: String,
    ext: &str,
    index: usize,
) -> Result<SynthesizedFile, String> {
    let path = export_dir()?.join(format!("clip-{}-{}.{}", now_nanos(), index, ext));
    fs::write(&path, payload).map_err(|e| format!("export write failed: {e}"))?;

    Ok(SynthesizedFile {
        path: path.to_string_lossy().to_string(),
        extension: ext.to_string(),
        kind: "text".to_string(),
    })
}

fn export_image(source: &Path, format: &str, index: usize) -> Result<SynthesizedFile, String> {
    let (ext, image_format) = match format {
        "jpg" | "jpeg" => ("jpg", ImageFormat::Jpeg),
        "webp" => ("webp", ImageFormat::WebP),
        _ => ("png", ImageFormat::Png),
    };

    let image = image::open(source).map_err(|e| format!("cannot read image: {e}"))?;

    // JPEG has no alpha channel, so flatten before encoding.
    let image = if image_format == ImageFormat::Jpeg {
        image::DynamicImage::ImageRgb8(image.to_rgb8())
    } else {
        image
    };

    let path = export_dir()?.join(format!("clip-{}-{}.{}", now_nanos(), index, ext));
    image
        .save_with_format(&path, image_format)
        .map_err(|e| format!("image export failed: {e}"))?;

    Ok(SynthesizedFile {
        path: path.to_string_lossy().to_string(),
        extension: ext.to_string(),
        kind: "image".to_string(),
    })
}

/// Writes one temporary file per clip so the whole selection can be dragged out.
#[tauri::command]
pub fn export_clipboard_entries(
    ids: Vec<i64>,
    format: String,
    state: State<AppState>,
) -> Result<Vec<SynthesizedFile>, String> {
    if ids.is_empty() {
        return Err("Select at least one clip first.".to_string());
    }

    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    let mut files = Vec::with_capacity(ids.len());

    for (index, id) in ids.iter().enumerate() {
        let entry: ClipboardEntry = db
            .query_row(
                &format!("{SELECT_ENTRY} WHERE id = ?1"),
                params![id],
                map_entry,
            )
            .map_err(|e| format!("clipboard entry {id} not found: {e}"))?;

        let file = if entry.kind == "image" && !entry.image_path.is_empty() {
            if TEXT_FORMATS.contains(&format.as_str()) {
                // A text format was requested for an image clip: emit a path or
                // embed reference rather than an empty file.
                let payload = image_text_payload(&entry.image_path, &format);
                export_text_payload(payload, &format, index)?
            } else {
                let image_format = if IMAGE_FORMATS.contains(&format.as_str()) {
                    format.as_str()
                } else {
                    "png"
                };
                export_image(Path::new(&entry.image_path), image_format, index)?
            }
        } else {
            export_text(&entry.content, &format, index)?
        };

        files.push(file);
    }

    Ok(files)
}

/// Stores a text clip rendered by the WebView canvas as a PNG/JPEG/WebP file.
/// The frontend does the text layout so Windows fonts and Unicode render well.
#[tauri::command]
pub fn save_converted_text_image(
    data_base64: String,
    format: String,
) -> Result<SynthesizedFile, String> {
    let (extension, image_format) = match format.as_str() {
        "jpg" => ("jpg", ImageFormat::Jpeg),
        "webp" => ("webp", ImageFormat::WebP),
        "png" => ("png", ImageFormat::Png),
        _ => return Err(format!("unsupported image format: {format}")),
    };
    let encoded = data_base64
        .strip_prefix("data:image/")
        .and_then(|payload| payload.split_once(",").map(|(_, data)| data))
        .ok_or_else(|| "invalid rendered image data".to_string())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("invalid rendered image data: {e}"))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|e| format!("cannot decode rendered text image: {e}"))?;
    let image = if image_format == ImageFormat::Jpeg {
        image::DynamicImage::ImageRgb8(image.to_rgb8())
    } else {
        image
    };
    let path = export_dir()?.join(format!("clip-{}.{}", now_nanos(), extension));
    image
        .save_with_format(&path, image_format)
        .map_err(|e| format!("converted image write failed: {e}"))?;

    Ok(SynthesizedFile {
        path: path.to_string_lossy().to_string(),
        extension: extension.to_string(),
        kind: "image".to_string(),
    })
}

#[tauri::command]
pub fn get_clipboard_watch_enabled(state: State<AppState>) -> bool {
    state.clipboard_enabled()
}

#[tauri::command]
pub fn set_clipboard_watch_enabled(enabled: bool, state: State<AppState>) -> bool {
    state.set_clipboard_enabled(enabled);
    enabled
}

fn save_image_capture(width: u32, height: u32, rgba: &[u8]) -> Option<(String, String)> {
    let mut dir = std::env::temp_dir();
    dir.push("dynamic-island-images");
    fs::create_dir_all(&dir).ok()?;

    let path = dir.join(format!("capture-{}.png", now_nanos()));
    let buffer = image::RgbaImage::from_raw(width, height, rgba.to_vec())?;
    buffer.save_with_format(&path, ImageFormat::Png).ok()?;

    Some((
        path.to_string_lossy().to_string(),
        format!("Image {width}x{height}"),
    ))
}

/// Collapses duplicate captures. The watcher can observe a single copy several
/// times (multiple clipboard formats, repeated polls), and older rows stored no
/// content token at all, so image rows are re-hashed from their files.
fn dedupe_clipboard(conn: &Connection) {
    // Text and modern image rows carry a token in `content`; keep the newest.
    let _ = conn.execute(
        "DELETE FROM clipboard_items
         WHERE content <> ''
           AND id NOT IN (
               SELECT MAX(id) FROM clipboard_items WHERE content <> '' GROUP BY kind, content
           )",
        [],
    );

    // Legacy image rows: hash the stored files, backfill the token, drop repeats.
    let rows: Vec<(i64, String)> = {
        let Ok(mut stmt) =
            conn.prepare("SELECT id, image_path FROM clipboard_items WHERE kind = 'image' AND content = ''")
        else {
            return;
        };
        let Ok(mapped) = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?))) else {
            return;
        };
        mapped.flatten().collect()
    };

    let mut seen: HashMap<u64, i64> = HashMap::new();
    for (id, path) in rows {
        let Ok(bytes) = fs::read(&path) else { continue };
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        let hash = hasher.finish();

        if seen.contains_key(&hash) {
            let _ = conn.execute("DELETE FROM clipboard_items WHERE id = ?1", params![id]);
            let _ = fs::remove_file(&path);
        } else {
            seen.insert(hash, id);
            let _ = conn.execute(
                "UPDATE clipboard_items SET content = ?1 WHERE id = ?2",
                params![format!("image-{hash:016x}"), id],
            );
        }
    }
}

pub fn start_clipboard_watcher(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(_) => return,
        };

        let state = app.state::<AppState>();

        // Collapse any duplicates captured before de-duplication existed.
        if let Ok(db) = state.db.lock() {
            dedupe_clipboard(&db);
        }
        let mut last_text_hash: u64 = 0;
        let mut last_image_hash: u64 = 0;
        let mut tick: u64 = 0;

        loop {
            tick = tick.wrapping_add(1);

            if state.clipboard_enabled() {
                // Windows can expose both bitmap and text/file-list formats for
                // one clipboard operation. Prefer the bitmap so copied images
                // are stored as image clips instead of their path/description.
                let has_image = match clipboard.get_image() {
                    Ok(image) => {
                        // Hash the whole payload (not a prefix) so one copied
                        // image can never be captured twice.
                        let mut hasher = DefaultHasher::new();
                        image.width.hash(&mut hasher);
                        image.height.hash(&mut hasher);
                        image.bytes.as_ref().hash(&mut hasher);
                        let hash = hasher.finish();
                        let token = format!("image-{hash:016x}");

                        if hash != last_image_hash {
                            last_image_hash = hash;

                            let already_stored = match state.db.lock() {
                                Ok(db) => db
                                    .query_row(
                                        "SELECT id FROM clipboard_items WHERE kind = 'image' AND content = ?1 LIMIT 1",
                                        params![token],
                                        |row| row.get::<_, i64>(0),
                                    )
                                    .ok()
                                    .is_some(),
                                Err(_) => false,
                            };

                            if !already_stored {
                                if let Some((path, preview)) = save_image_capture(
                                    image.width as u32,
                                    image.height as u32,
                                    image.bytes.as_ref(),
                                ) {
                                    let inserted = match state.db.lock() {
                                        Ok(db) => {
                                            insert_entry(&db, "image", &preview, &token, &path).ok()
                                        }
                                        Err(_) => None,
                                    };

                                    if let Some(entry) = inserted {
                                        let _ = app.emit("clipboard-changed", entry);
                                    }
                                }
                            }
                        }
                        true
                    }
                    Err(_) => false,
                };

                if !has_image {
                    let text = clipboard
                        .get_text()
                        .ok()
                        .filter(|value| !value.trim().is_empty());

                    if let Some(text) = text {
                        let mut hasher = DefaultHasher::new();
                        text.trim().hash(&mut hasher);
                        let hash = hasher.finish();

                        if hash != last_text_hash {
                            last_text_hash = hash;
                            let preview = preview_of(&text);

                            let inserted = match state.db.lock() {
                                Ok(db) => insert_entry(&db, "text", &preview, &text, "").ok(),
                                Err(_) => None,
                            };

                            if let Some(entry) = inserted {
                                let _ = app.emit("clipboard-changed", entry);
                            }
                        }
                    }
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(650));
        }
    });
}
