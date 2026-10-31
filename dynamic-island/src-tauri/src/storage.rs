//! Storage reporting and cleanup for Settings → Storage.
//!
//! Everything the app caches on disk is counted separately so the numbers are
//! explainable: the SQLite database, clipboard image captures, shelf
//! attachments and generated previews. Clearing removes history and unpinned
//! shelf items, deletes any cache file no row refers to any more, then
//! compacts the database so the reported size actually drops. Pinned items,
//! everything inside a pinned folder, tasks and settings are never touched.

use crate::state::AppState;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize)]
pub struct StorageUsage {
    /// SQLite file size.
    pub database_bytes: u64,
    /// Clipboard image captures (`%TEMP%/dynamic-island-images`).
    pub clipboard_images_bytes: u64,
    /// Shelf attachments: inline images and HTML assets.
    pub shelf_files_bytes: u64,
    /// Generated thumbnails and cached link previews.
    pub previews_bytes: u64,
    pub total_bytes: u64,
    pub clipboard_items: i64,
    pub shelf_items: i64,
    pub pinned_items: i64,
    /// Set when the database had to be rebuilt at start-up (state::open_database).
    pub database_note: String,
}

fn temp_dir(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(name);
    dir
}

/// Recursive folder size with a depth guard; unreadable entries count as zero.
fn dir_size(path: &Path, depth: usize) -> u64 {
    if depth > 3 {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            total += dir_size(&entry.path(), depth + 1);
        } else {
            total += metadata.len();
        }
    }
    total
}

fn count(state: &AppState, sql: &str) -> i64 {
    state
        .db
        .lock()
        .ok()
        .and_then(|db| db.query_row(sql, [], |row| row.get::<_, i64>(0)).ok())
        .unwrap_or(0)
}

fn read_usage(app: &AppHandle, state: &AppState) -> StorageUsage {
    let database_bytes = std::fs::metadata(&state.db_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let clipboard_images_bytes = dir_size(&temp_dir("dynamic-island-images"), 0);
    let shelf_files_bytes = crate::shelf::assets_dir(app)
        .ok()
        .map(|assets| dir_size(&assets, 0))
        .unwrap_or(0);
    let previews_bytes = dir_size(&temp_dir("dynamic-island-thumbs"), 0);

    StorageUsage {
        database_bytes,
        clipboard_images_bytes,
        shelf_files_bytes,
        previews_bytes,
        total_bytes: database_bytes + clipboard_images_bytes + shelf_files_bytes + previews_bytes,
        clipboard_items: count(state, "SELECT COUNT(*) FROM clipboard_items"),
        shelf_items: count(state, "SELECT COUNT(*) FROM shelf_items"),
        pinned_items: count(state, "SELECT COUNT(*) FROM shelf_items WHERE pinned = 1"),
        database_note: state.db_note().to_string(),
    }
}

/// Every file the database still points at. Paths are normalised so the sweep
/// compares like with like regardless of slash direction or case.
fn referenced_paths(state: &AppState) -> HashSet<String> {
    let mut referenced = HashSet::new();
    let Ok(db) = state.db.lock() else {
        return referenced;
    };

    if let Ok(mut stmt) = db.prepare("SELECT image_path FROM clipboard_items WHERE image_path <> ''")
    {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for path in rows.flatten() {
                referenced.insert(normalize(&path));
            }
        }
    }

    if let Ok(mut stmt) =
        db.prepare("SELECT path, thumb_path, meta_image, meta_favicon, assets FROM shelf_items")
    {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        }) {
            for (path, thumb, image, favicon, assets) in rows.flatten() {
                for value in [path, thumb, image, favicon] {
                    if !value.is_empty() {
                        referenced.insert(normalize(&value));
                    }
                }
                if let Ok(list) = serde_json::from_str::<Vec<String>>(&assets) {
                    for value in list {
                        referenced.insert(normalize(&value));
                    }
                }
            }
        }
    }

    referenced
}

fn normalize(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

/// Deletes files in a cache folder that no row references any more (captures
/// from cleared histories, superseded previews, …). Returns the bytes freed.
fn sweep(dir: &Path, referenced: &HashSet<String>) -> u64 {
    let mut freed = 0;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let path = entry.path();
        if referenced.contains(&normalize(&path.to_string_lossy())) {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            freed += metadata.len();
        }
    }
    freed
}

fn sweep_caches(app: &AppHandle, state: &AppState) {
    let referenced = referenced_paths(state);
    sweep(&temp_dir("dynamic-island-images"), &referenced);
    sweep(&temp_dir("dynamic-island-thumbs"), &referenced);
    if let Ok(assets) = crate::shelf::assets_dir(app) {
        sweep(&assets, &referenced);
    }
}

#[tauri::command]
pub fn storage_usage(app: AppHandle, state: State<AppState>) -> Result<StorageUsage, String> {
    Ok(read_usage(&app, &state))
}

#[tauri::command]
pub async fn clear_storage(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StorageUsage, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let state = handle.state::<AppState>();
        crate::clipboard::clear_clipboard_history(&state)?;
        crate::shelf::clear_unpinned_shelf(&state)?;
        {
            let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
            let _ = db.execute("DELETE FROM shelf_history", []);
            // Reclaims the pages freed above; without it the file keeps its size.
            let _ = db.execute_batch("VACUUM;");
        }
        // Leftovers from earlier clears or deduped captures.
        sweep_caches(&handle, &state);
        Ok(())
    })
    .await
    .map_err(|error| format!("storage clear failed: {error}"))??;

    let _ = app.emit("shelf-changed", ());
    let _ = app.emit("clipboard-changed", ());
    Ok(read_usage(&app, &state))
}
