use crate::state::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TodoItem {
    pub id: String,
    pub label: String,
    pub done: bool,
}

#[tauri::command]
pub fn list_todos(state: State<AppState>) -> Result<Vec<TodoItem>, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    let mut stmt = db
        .prepare("SELECT id, label, done FROM todo_items ORDER BY created_at DESC")
        .map_err(|e| format!("todo query prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![], |row| {
            Ok(TodoItem {
                id: row.get(0)?,
                label: row.get(1)?,
                done: row.get::<_, i64>(2)? == 1,
            })
        })
        .map_err(|e| format!("todo query failed: {e}"))?;

    // One damaged row must not hide the rest of the list.
    Ok(rows.flatten().collect())
}

#[tauri::command]
pub fn add_todo(label: String, state: State<AppState>) -> Result<TodoItem, String> {
    let id = format!(
        "todo-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("clock failed: {e}"))?
            .as_nanos()
    );

    let item = TodoItem {
        id: id.clone(),
        label,
        done: false,
    };

    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    db.execute(
        "INSERT INTO todo_items (id, label, done) VALUES (?1, ?2, 0)",
        params![item.id, item.label],
    )
    .map_err(|e| format!("todo insert failed: {e}"))?;

    Ok(item)
}

#[tauri::command]
pub fn set_todo_done(id: String, done: bool, state: State<AppState>) -> Result<TodoItem, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;

    db.execute(
        "UPDATE todo_items SET done = ?2 WHERE id = ?1",
        params![id, if done { 1 } else { 0 }],
    )
    .map_err(|e| format!("todo update failed: {e}"))?;

    let mut stmt = db
        .prepare("SELECT id, label, done FROM todo_items WHERE id = ?1")
        .map_err(|e| format!("todo fetch prepare failed: {e}"))?;

    stmt.query_row(params![id], |row| {
        Ok(TodoItem {
            id: row.get(0)?,
            label: row.get(1)?,
            done: row.get::<_, i64>(2)? == 1,
        })
    })
    .map_err(|e| format!("todo fetch failed: {e}"))
}

#[tauri::command]
pub fn delete_todo(id: String, state: State<AppState>) -> Result<bool, String> {
    let db = state.db.lock().map_err(|_| "db lock failed".to_string())?;
    let affected = db
        .execute("DELETE FROM todo_items WHERE id = ?1", params![id])
        .map_err(|e| format!("todo delete failed: {e}"))?;
    Ok(affected > 0)
}
