mod clipboard;
mod http;
mod media;
mod search;
mod settings;
mod shelf;
mod startup;
mod state;
mod storage;
mod timer;
mod todo;
mod tray;
mod updates;
mod utils;

use clipboard::{
    clear_clipboard_entries, copy_clipboard_entry, delete_clipboard_entry,
    export_clipboard_entries, get_clipboard_entry, get_clipboard_watch_enabled,
    list_clipboard_entries, save_converted_text_image, set_clipboard_watch_enabled,
    start_clipboard_watcher, update_clipboard_entry,
};
use media::{get_media_state, get_media_timeline, media_action};
use search::{everything_search, open_external, open_search_result, search_status};
use settings::{
    get_autostart, get_settings, set_autostart, set_minified_island, set_search_engine,
    set_settings,
};
use shelf::{
    clear_shelf_items, copy_shelf_items, create_shelf_folder, export_shelf_items, get_shelf_items,
    ingest_drop, list_shelf_history, move_shelf_item, open_shelf_item, refresh_link_metadata,
    remove_shelf_item, rename_shelf_item, restore_shelf, save_shelf_item, set_shelf_pinned,
    start_drop_target,
};
use state::AppState;
use storage::{clear_storage, storage_usage};
use timer::{pause_timer, reset_timer, start_timer};
use todo::{add_todo, delete_todo, list_todos, set_todo_done};
use updates::{check_for_update, update_channel};
use utils::{
    get_drag_icon, open_clip_editor, open_settings_window, set_island_window_bounds,
    set_island_window_region, start_click_through_watcher, start_fullscreen_watcher,
};

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `AppState::new` heals a missing/corrupt database instead of aborting, so
    // a damaged file can never leave the app unlaunchable.
    let state = AppState::new();
    if !state.db_note().is_empty() {
        eprintln!("[db] {}", state.db_note());
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
                // Frameless, shadowless and transparent: the CSS island is the
                // only painted surface. Re-assert here so a restored window
                // can never come back with native chrome.
                let _ = window.set_decorations(false);
                let _ = window.set_shadow(false);
                let _ = window.set_skip_taskbar(true);
                // Keep native window fully transparent. Applying Mica at the
                // whole-window level causes a transient rectangular backdrop
                // during bounds morphs (pill -> expanded). The glass effect is
                // rendered only on the island surface via CSS backdrop blur.
            }

            let loaded = app
                .state::<AppState>()
                .db
                .lock()
                .map(|db| settings::load_settings(&db))
                .unwrap_or_default();
            app.state::<AppState>()
                .set_click_through_mods(settings::shortcut_mask(&loaded.click_through_shortcut));
            // Keep the Windows logon entry pointing at this executable after
            // an update moved the install.
            startup::sync_on_launch(&app.state::<AppState>());
            let minified = loaded.minified_island;
            let _ = set_island_window_bounds(app.handle().clone(), false, minified, false);
            // The window is created hidden so it cannot flash at the screen
            // center before it is anchored to the top edge.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
            start_fullscreen_watcher(app.handle().clone());
            start_click_through_watcher(app.handle().clone());

            start_clipboard_watcher(app.handle().clone());
            restore_shelf(app.handle().clone());
            start_drop_target(app.handle().clone());
            tray::create_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_clipboard_entries,
            get_clipboard_entry,
            update_clipboard_entry,
            delete_clipboard_entry,
            clear_clipboard_entries,
            export_clipboard_entries,
            save_converted_text_image,
            get_settings,
            set_settings,
            set_minified_island,
            set_search_engine,
            get_autostart,
            set_autostart,
            open_clip_editor,
            open_settings_window,
            ingest_drop,
            export_shelf_items,
            remove_shelf_item,
            clear_shelf_items,
            get_shelf_items,
            open_shelf_item,
            set_shelf_pinned,
            copy_shelf_items,
            create_shelf_folder,
            move_shelf_item,
            rename_shelf_item,
            refresh_link_metadata,
            save_shelf_item,
            list_shelf_history,
            copy_clipboard_entry,
            get_media_state,
            get_media_timeline,
            media_action,
            start_timer,
            pause_timer,
            reset_timer,
            list_todos,
            add_todo,
            set_todo_done,
            delete_todo,
            get_drag_icon,
            set_island_window_bounds,
            set_island_window_region,
            get_clipboard_watch_enabled,
            set_clipboard_watch_enabled,
            search_status,
            everything_search,
            open_external,
            open_search_result,
            storage_usage,
            clear_storage,
            check_for_update,
            update_channel
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
