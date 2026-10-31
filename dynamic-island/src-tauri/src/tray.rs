use crate::settings::{load_settings, persist_minified_island};
use crate::state::AppState;
use crate::utils::open_settings_window;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Emitter, Manager,
};

pub fn create_tray(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Pill", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let toggle_capture = MenuItem::with_id(
        app,
        "toggle_clipboard",
        "Toggle Clipboard Capture",
        true,
        None::<&str>,
    )?;
    let minified_enabled = app
        .state::<AppState>()
        .db
        .lock()
        .map(|db| load_settings(&db).minified_island)
        .unwrap_or(false);
    let minified = CheckMenuItem::with_id(
        app,
        "minified_island",
        "Minified Pill",
        true,
        minified_enabled,
        None::<&str>,
    )?;
    let restart = MenuItem::with_id(app, "restart", "Restart Application", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let separator_three = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &show,
            &settings,
            &separator_one,
            &toggle_capture,
            &minified,
            &separator_two,
            &restart,
            &separator_three,
            &quit,
        ],
    )?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".to_string()))?;

    TrayIconBuilder::with_id("island-tray")
        .icon(icon)
        .icon_as_template(false)
        .tooltip("Pill")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event({
            let minified = minified.clone();
            move |app, event| match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        app.state::<AppState>().set_island_hidden_by_user(false);
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "settings" => {
                    if let Err(error) = open_settings_window(app.clone()) {
                        eprintln!("failed to open settings: {error}");
                    }
                }
                "toggle_clipboard" => {
                    let state = app.state::<AppState>();
                    let next = !state.clipboard_enabled();
                    state.set_clipboard_enabled(next);
                    let _ = app.emit("clipboard-watch-changed", next);
                }
                "minified_island" => {
                    let state = app.state::<AppState>();
                    let current = state
                        .db
                        .lock()
                        .map(|db| load_settings(&db).minified_island)
                        .unwrap_or_else(|_| minified.is_checked().unwrap_or(false));
                    let next = !current;
                    if let Err(error) = persist_minified_island(&state, next) {
                        eprintln!("failed to save Minified Island setting: {error}");
                    } else {
                        let _ = minified.set_checked(next);
                        let _ = app.emit("minified-island-changed", next);
                    }
                }
                "restart" => app.restart(),
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    if visible {
                        app.state::<AppState>().set_island_hidden_by_user(true);
                        let _ = window.hide();
                    } else {
                        app.state::<AppState>().set_island_hidden_by_user(false);
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
