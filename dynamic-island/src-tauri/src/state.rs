use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

pub const DB_FILE: &str = "dynamic-island.db3";
/// Folder Tauri calls `app_local_data_dir` for this bundle identifier.
const DATA_DIR_NAME: &str = "com.asrumon.pill";

pub struct AppState {
    pub db: Mutex<Connection>,
    /// On-disk location of the SQLite database (reported in Settings → Storage).
    pub db_path: std::path::PathBuf,
    /// Why the database had to be replaced at start-up. Empty on a normal run;
    /// surfaced in Settings → Storage so a silent rebuild is never silent.
    db_note: String,
    pub shelf: Mutex<Vec<ShelfEntry>>,
    clipboard_enabled: AtomicBool,
    island_hidden_by_user: AtomicBool,
    /// Bitmask of the click-through modifiers: 1 = Ctrl, 2 = Alt, 4 = Shift.
    click_through_mods: AtomicU8,
}

#[derive(Debug, Clone)]
pub struct ShelfEntry {
    pub id: String,
    /// `text` | `url` | `html` | `image` | `file` | `folder`
    pub kind: String,
    pub path: String,
    pub content: String,
    pub name: String,
    pub extension: String,
    pub mime: String,
    pub size_bytes: u64,
    pub thumb_path: String,
    /// "cover" for photo previews, "contain" for shell thumbnails/icons.
    pub thumb_kind: String,
    pub preview: String,
    pub assets: Vec<String>,
    pub pinned: bool,
    pub created_at: i64,
    /// Folder id when the item lives inside an organisational folder.
    pub parent_id: String,
    /// User supplied link/folder title. Kept separate from `name` so a later
    /// metadata refresh can never overwrite it.
    pub custom_title: String,
    pub meta_title: String,
    pub meta_description: String,
    pub meta_site: String,
    /// Local thumbnail/og:image path (never a remote URL).
    pub meta_image: String,
    pub meta_favicon: String,
    /// Unix ms of the last metadata fetch; `0` means never fetched.
    pub meta_fetched_at: i64,
}

impl AppState {
    /// Never fails: a database that cannot be opened, is truncated or holds
    /// garbage is moved aside and rebuilt (see [`open_database`]).
    pub fn new() -> Self {
        let OpenedDatabase { conn, path, note } = open_database();

        Self {
            db: Mutex::new(conn),
            db_path: path,
            db_note: note,
            shelf: Mutex::new(Vec::new()),
            clipboard_enabled: AtomicBool::new(true),
            island_hidden_by_user: AtomicBool::new(false),
            click_through_mods: AtomicU8::new(0),
        }
    }

    /// Human readable description of a start-up recovery, empty when the
    /// database opened cleanly.
    pub fn db_note(&self) -> &str {
        &self.db_note
    }

    pub fn click_through_mods(&self) -> u8 {
        self.click_through_mods.load(Ordering::Relaxed)
    }

    pub fn set_click_through_mods(&self, mods: u8) {
        self.click_through_mods.store(mods, Ordering::Relaxed);
    }

    pub fn clipboard_enabled(&self) -> bool {
        self.clipboard_enabled.load(Ordering::Relaxed)
    }

    pub fn set_clipboard_enabled(&self, enabled: bool) {
        self.clipboard_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn island_hidden_by_user(&self) -> bool {
        self.island_hidden_by_user.load(Ordering::Relaxed)
    }

    pub fn set_island_hidden_by_user(&self, hidden: bool) {
        self.island_hidden_by_user.store(hidden, Ordering::Relaxed);
    }
}

fn now_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

/// `%LOCALAPPDATA%\com.asrumon.pill` — the same folder Tauri uses for
/// `app_local_data_dir`, next to the shelf assets. Keeping the database here
/// (instead of `%TEMP%`) means Windows Storage Sense / "Disk Cleanup" cannot
/// quietly delete the user's tasks, settings and pinned items.
fn data_dir() -> PathBuf {
    match std::env::var_os("LOCALAPPDATA") {
        Some(base) => PathBuf::from(base).join(DATA_DIR_NAME),
        None => std::env::temp_dir(),
    }
}

/// Builds before 2.0.1 kept the database in `%TEMP%`.
fn legacy_db_path() -> PathBuf {
    std::env::temp_dir().join(DB_FILE)
}

struct OpenedDatabase {
    conn: Connection,
    path: PathBuf,
    note: String,
}

/// Opens the database, moving it out of `%TEMP%` once and healing it when the
/// file cannot be used.
///
/// Start-up used to abort with `expect(...)` when SQLite refused the file, which
/// left the app unlaunchable until the user deleted it by hand. Now a damaged
/// file is renamed to `dynamic-island.db3.corrupt-<timestamp>` (kept for
/// inspection), a clean database is created, and if even that fails the app
/// keeps running on an in-memory database.
fn open_database() -> OpenedDatabase {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    migrate_legacy(&dir.join(DB_FILE));
    open_database_at(&dir)
}

/// Moves a `<temp>\dynamic-island.db3` left by an older build into the data
/// folder. Never overwrites an existing database.
fn migrate_legacy(target: &Path) {
    let legacy = legacy_db_path();
    if target.exists() || !legacy.is_file() {
        return;
    }
    if std::fs::rename(&legacy, target).is_ok() {
        return;
    }
    // Different volume or a locked file: copy instead.
    if std::fs::copy(&legacy, target).is_ok() {
        let _ = std::fs::remove_file(&legacy);
    }
}

fn open_database_at(dir: &Path) -> OpenedDatabase {
    let path = dir.join(DB_FILE);
    let _ = std::fs::create_dir_all(dir);

    match try_open(&path) {
        Ok(conn) => OpenedDatabase {
            conn,
            path,
            note: String::new(),
        },
        Err(error) => {
            let note = quarantine(&path, &error);
            match try_open(&path) {
                Ok(conn) => OpenedDatabase { conn, path, note },
                Err(second) => {
                    let note = format!("{note} Running without a database ({second}).");
                    OpenedDatabase {
                        conn: memory_database(),
                        path,
                        note,
                    }
                }
            }
        }
    }
}

/// Opens the file and refuses to hand back a database whose pages do not check
/// out. `quick_check` is the cheap variant of `integrity_check`; it already
/// catches truncation, bad headers and unreadable pages.
fn try_open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("open failed: {e}"))?;
    let check: String = conn
        .query_row("PRAGMA quick_check(3)", [], |row| row.get(0))
        .map_err(|e| format!("integrity check failed: {e}"))?;
    if !check.eq_ignore_ascii_case("ok") {
        return Err(format!("integrity check failed: {check}"));
    }
    init_schema(&conn)?;
    Ok(conn)
}

/// Renames an unusable database aside instead of deleting it, then reports what
/// happened in one line for the Settings screen.
fn quarantine(path: &Path, error: &str) -> String {
    if !path.exists() {
        return format!("The database could not be created ({error}).");
    }

    let backup = path.with_extension(format!("db3.corrupt-{}", now_stamp()));
    match std::fs::rename(path, &backup) {
        Ok(()) => format!(
            "The database was damaged ({error}) and has been rebuilt. The old file is kept as {}.",
            backup
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default()
        ),
        Err(rename_error) => {
            // Locked or read-only: the only way to start is a fresh file.
            let removed = std::fs::remove_file(path).is_ok();
            format!(
                "The database was damaged ({error}){}. The old file could not be kept ({rename_error}).",
                if removed { " and has been rebuilt" } else { "" }
            )
        }
    }
}

fn memory_database() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory database");
    let _ = init_schema(&conn);
    conn
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS clipboard_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            preview TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            image_path TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS todo_items (
            id TEXT PRIMARY KEY,
            label TEXT NOT NULL,
            done INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS shelf_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            path TEXT NOT NULL,
            kind TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        INSERT OR IGNORE INTO settings (key, value) VALUES ('default_text_format', 'txt');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('default_image_format', 'png');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('minified_island', 'false');
        ",
    )
    .map_err(|e| format!("schema init failed: {e}"))?;

    migrate_shelf(conn)
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |_| Ok(()),
    )
    .is_ok()
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA table_info({table})")) else {
        return false;
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return false;
    };
    rows.flatten().any(|name| name == column)
}

/// Brings `shelf_items` up to the drop-inbox schema while keeping existing rows.
fn migrate_shelf(conn: &Connection) -> Result<(), String> {
    if table_exists(conn, "shelf_items") && !column_exists(conn, "shelf_items", "kind") {
        conn.execute_batch("ALTER TABLE shelf_items RENAME TO shelf_items_v0;")
            .map_err(|e| format!("shelf migration failed: {e}"))?;
    }
    let legacy = table_exists(conn, "shelf_items_v0");

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS shelf_items (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL DEFAULT 'file',
            path TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            name TEXT NOT NULL DEFAULT '',
            extension TEXT NOT NULL DEFAULT '',
            mime TEXT NOT NULL DEFAULT '',
            size_bytes INTEGER NOT NULL DEFAULT 0,
            thumb_path TEXT NOT NULL DEFAULT '',
            thumb_kind TEXT NOT NULL DEFAULT '',
            preview TEXT NOT NULL DEFAULT '',
            assets TEXT NOT NULL DEFAULT '[]',
            pinned INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT 0,
            parent_id TEXT NOT NULL DEFAULT '',
            custom_title TEXT NOT NULL DEFAULT '',
            meta_title TEXT NOT NULL DEFAULT '',
            meta_description TEXT NOT NULL DEFAULT '',
            meta_site TEXT NOT NULL DEFAULT '',
            meta_image TEXT NOT NULL DEFAULT '',
            meta_favicon TEXT NOT NULL DEFAULT '',
            meta_fetched_at INTEGER NOT NULL DEFAULT 0
        );
        ",
    )
    .map_err(|e| format!("shelf schema failed: {e}"))?;

    // Existing installs predate folders and link previews.
    for (column, declaration) in [
        ("parent_id", "TEXT NOT NULL DEFAULT ''"),
        ("custom_title", "TEXT NOT NULL DEFAULT ''"),
        ("meta_title", "TEXT NOT NULL DEFAULT ''"),
        ("meta_description", "TEXT NOT NULL DEFAULT ''"),
        ("meta_site", "TEXT NOT NULL DEFAULT ''"),
        ("meta_image", "TEXT NOT NULL DEFAULT ''"),
        ("meta_favicon", "TEXT NOT NULL DEFAULT ''"),
        ("meta_fetched_at", "INTEGER NOT NULL DEFAULT 0"),
    ] {
        if !column_exists(conn, "shelf_items", column) {
            conn.execute_batch(&format!(
                "ALTER TABLE shelf_items ADD COLUMN {column} {declaration};"
            ))
            .map_err(|e| format!("shelf migration failed: {e}"))?;
        }
    }

    if legacy {
        conn.execute_batch(
            "
            INSERT OR IGNORE INTO shelf_items
                (id, kind, path, name, extension, size_bytes, pinned, created_at)
            SELECT id, 'file', path, '', '', 0, pinned, created_at FROM shelf_items_v0;
            DROP TABLE shelf_items_v0;
            ",
        )
        .map_err(|e| format!("shelf migration copy failed: {e}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pill-db-{tag}-{}", now_stamp()));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn fresh_directory_creates_the_schema() {
        let dir = temp_dir("fresh");
        let opened = open_database_at(&dir);
        assert!(
            opened.note.is_empty(),
            "clean start must not report a recovery"
        );
        assert!(opened.path.exists());
        let tables: i64 = opened
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .expect("schema query");
        assert!(tables >= 5, "expected the five tables, found {tables}");
        drop(opened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_database_is_quarantined_and_rebuilt() {
        let dir = temp_dir("corrupt");
        let path = dir.join(DB_FILE);
        // Real header, garbage body: `open` succeeds, reading does not.
        std::fs::write(&path, b"SQLite format 3\0definitely not a database").expect("write junk");

        let opened = open_database_at(&dir);
        assert!(!opened.note.is_empty(), "a rebuild must be reported");
        assert!(
            opened
                .conn
                .query_row("SELECT COUNT(*) FROM settings", [], |row| row
                    .get::<_, i64>(0))
                .is_ok(),
            "the replacement database must be usable"
        );
        let backups = std::fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(backups, 1, "the damaged file must be kept, not deleted");
        drop(opened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_file_is_recovered_too() {
        let dir = temp_dir("empty");
        std::fs::write(dir.join(DB_FILE), b"").expect("write empty file");
        let opened = open_database_at(&dir);
        assert!(
            opened.note.is_empty(),
            "an empty file is a valid new database"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn existing_rows_survive_a_reopen() {
        let dir = temp_dir("keep");
        {
            let opened = open_database_at(&dir);
            opened
                .conn
                .execute(
                    "INSERT INTO settings (key, value) VALUES ('island_blur', '7')",
                    [],
                )
                .expect("seed row");
        }
        let reopened = open_database_at(&dir);
        assert!(reopened.note.is_empty());
        let blur: String = reopened
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'island_blur'",
                [],
                |row| row.get(0),
            )
            .expect("row still there");
        assert_eq!(blur, "7");
        drop(reopened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn data_dir_is_outside_the_temp_cleanup_path() {
        let dir = data_dir();
        assert!(dir.ends_with(DATA_DIR_NAME));
        assert!(
            !dir.starts_with(std::env::temp_dir()) || std::env::var_os("LOCALAPPDATA").is_none()
        );
    }
}
