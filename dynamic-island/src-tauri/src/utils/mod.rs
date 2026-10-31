use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow};

/// The native window keeps the expanded size for its whole life. Only the
/// region changes shape, so the island can never jump or re-centre while the
/// shell animates — resizing/moving the window mid-hover did exactly that.
const ISLAND_WIDTH: f64 = 540.0;
const ISLAND_HEIGHT: f64 = 430.0;

fn collapsed_size(minified: bool, timer_active: bool) -> (f64, f64) {
    if minified && !timer_active {
        // The tiny dot only exists while no session is running.
        (92.0, 10.0)
    } else if timer_active {
        // A running session brings the normal bar back so the timer chip fits.
        (288.0, 36.0)
    } else {
        (246.0, 36.0)
    }
}

/// Pins the window to the top-centre of the primary monitor. Idempotent: the
/// size never changes, so calling it again cannot move the island.
fn anchor_window(window: &WebviewWindow) -> Result<(), String> {
    let scale = window
        .scale_factor()
        .map_err(|e| format!("scale factor lookup failed: {e}"))?;
    let monitor = window
        .primary_monitor()
        .map_err(|e| format!("monitor lookup failed: {e}"))?
        .ok_or_else(|| "no primary monitor available".to_string())?;
    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();
    let x = monitor_pos.x
        + ((monitor_size.width as i64 - (ISLAND_WIDTH * scale).round() as i64) / 2) as i32;

    // Only touch the native window when something actually differs. An
    // unhidden `SetWindowPos` per expand repainted the frame and could make
    // the caption flicker back over the island.
    let target_width = (ISLAND_WIDTH * scale).round() as u32;
    let target_height = (ISLAND_HEIGHT * scale).round() as u32;
    let current = window.outer_size().ok();
    let needs_resize = current
        .map(|size| size.width != target_width || size.height != target_height)
        .unwrap_or(true);
    if needs_resize {
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                ISLAND_WIDTH,
                ISLAND_HEIGHT,
            )))
            .map_err(|e| format!("window resize failed: {e}"))?;
    }

    let position = window.outer_position().ok();
    let needs_move = position
        .map(|current| current.x != x || current.y != monitor_pos.y)
        .unwrap_or(true);
    if needs_move {
        window
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                x,
                monitor_pos.y,
            )))
            .map_err(|e| format!("window positioning failed: {e}"))?;
    }
    Ok(())
}

/// Re-shapes the island. The window itself never resizes, so this only has to
/// keep the hit-test region in sync with the collapsed or expanded state.
#[tauri::command]
pub fn set_island_window_bounds(
    app: AppHandle,
    expanded: bool,
    minified: bool,
    timer_active: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?;

    // The island is frameless by design. A window that picked its native
    // caption back up (after a DWM/composition change) is repaired here so an
    // expand never shows a title bar over the panel.
    if window.is_decorated().unwrap_or(false) {
        let _ = window.set_decorations(false);
        let _ = window.set_shadow(false);
    }

    anchor_window(&window)?;

    // Growth is driven per frame from the frontend through
    // `set_island_window_region`; only a fully collapsed island needs the
    // region restored here.
    if !expanded {
        let (width, height) = collapsed_size(minified, timer_active);
        set_native_region(&window, width, height, minified, Some(ISLAND_WIDTH))?;
    }

    Ok(())
}

#[tauri::command]
pub fn set_island_window_region(
    app: AppHandle,
    width: f64,
    height: f64,
    pill: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?;
    set_native_region(&window, width, height, pill, None)
}

/// Keeps the island off the primary display while another app owns a
/// borderless full-screen window. Its own hide/show choice remains respected.
/// Lets the user reach whatever sits behind the island.
///
/// While the configured modifiers are held (Ctrl+Alt by default, changeable in
/// settings) the native window ignores the mouse entirely, so clicks land on
/// the window underneath. Releasing restores normal interaction.
pub fn start_click_through_watcher(app: AppHandle) {
    #[cfg(target_os = "windows")]
    std::thread::spawn(move || {
        use std::time::Duration;
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
        };

        let mut ignoring = false;
        loop {
            let mask = app.state::<crate::state::AppState>().click_through_mods();

            let held = mask != 0
                && unsafe {
                    let control = GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000 != 0;
                    let alt = GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000 != 0;
                    let shift = GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000 != 0;
                    (mask & 1 == 0 || control) && (mask & 2 == 0 || alt) && (mask & 4 == 0 || shift)
                };

            if held != ignoring {
                ignoring = held;
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.set_ignore_cursor_events(ignoring);
                    }
                });
                let _ = app.emit("click-through", ignoring);
            }

            std::thread::sleep(Duration::from_millis(90));
        }
    });
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

pub fn start_fullscreen_watcher(app: AppHandle) {
    #[cfg(target_os = "windows")]
    std::thread::spawn(move || {
        use std::time::Duration;
        let mut hidden_for_fullscreen = false;
        loop {
            if let Some(window) = app.get_webview_window("main") {
                let island_hwnd = window.hwnd().ok().map(|hwnd| hwnd.0);
                let fullscreen = island_hwnd.is_some_and(is_primary_fullscreen_foreground);
                let visible = window.is_visible().unwrap_or(false);

                if fullscreen {
                    if visible {
                        let _ = window.hide();
                    }
                    let other_app_window_open =
                        app.webview_windows().iter().any(|(label, other)| {
                            label != "main" && other.is_visible().unwrap_or(false)
                        });
                    if !app
                        .state::<crate::state::AppState>()
                        .island_hidden_by_user()
                        && !other_app_window_open
                    {
                        hidden_for_fullscreen = true;
                    }
                } else if !fullscreen && hidden_for_fullscreen {
                    let other_app_window_open =
                        app.webview_windows().iter().any(|(label, other)| {
                            label != "main" && other.is_visible().unwrap_or(false)
                        });
                    if !app
                        .state::<crate::state::AppState>()
                        .island_hidden_by_user()
                        && !other_app_window_open
                    {
                        let _ = window.show();
                    }
                    hidden_for_fullscreen = false;
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

#[cfg(target_os = "windows")]
fn is_primary_fullscreen_foreground(island_hwnd: *mut std::ffi::c_void) -> bool {
    use windows_sys::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::{
            GetClassNameW, GetDesktopWindow, GetForegroundWindow, GetShellWindow, GetWindowRect,
            IsWindowVisible, MONITORINFOF_PRIMARY,
        },
    };

    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null()
            || foreground == island_hwnd
            || foreground == GetDesktopWindow()
            || foreground == GetShellWindow()
            || IsWindowVisible(foreground) == 0
        {
            return false;
        }

        // The desktop wallpaper is hosted in full-monitor shell windows too;
        // it is not an app occupying the screen and must not suppress the island.
        let mut class_name = [0u16; 128];
        let class_len = GetClassNameW(foreground, class_name.as_mut_ptr(), class_name.len() as i32);
        if class_len > 0 {
            let class_name = String::from_utf16_lossy(&class_name[..class_len as usize]);
            if [
                "Progman",
                "WorkerW",
                "Shell_TrayWnd",
                "Shell_SecondaryTrayWnd",
            ]
            .iter()
            .any(|shell_class| class_name.eq_ignore_ascii_case(shell_class))
            {
                return false;
            }
        }

        let monitor = MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return false;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info) == 0 || info.dwFlags & MONITORINFOF_PRIMARY == 0 {
            return false;
        }

        let mut rect = RECT::default();
        if GetWindowRect(foreground, &mut rect) == 0 {
            return false;
        }

        let bounds = info.rcMonitor;
        // A two-pixel tolerance covers DPI rounding while excluding maximized
        // windows that stop at the taskbar's work-area edge.
        rect.left <= bounds.left + 2
            && rect.top <= bounds.top + 2
            && rect.right >= bounds.right - 2
            && rect.bottom >= bounds.bottom - 2
    }
}

fn set_native_region(
    window: &WebviewWindow,
    logical_width: f64,
    logical_height: f64,
    pill: bool,
    known_window_width: Option<f64>,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Graphics::Gdi::{
            CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, RGN_OR,
        };

        let hwnd = window
            .hwnd()
            .map_err(|e| format!("window handle lookup failed: {e}"))?;
        let scale = window.scale_factor().unwrap_or(1.0);
        let width = (logical_width * scale).round() as i32;
        let height = (logical_height * scale).round() as i32;
        let available = match known_window_width {
            // A resize may not have landed yet, so trust the target width.
            Some(value) => (value * scale).round() as i32,
            None => {
                window
                    .outer_size()
                    .map_err(|e| format!("window size lookup failed: {e}"))?
                    .width as i32
            }
        };
        let left = ((available - width) / 2).max(0);
        let radius = ((if pill {
            logical_height / 2.0
        } else if logical_height > 36.0 {
            30.0
        } else {
            20.0
        }) * scale)
            .round() as i32;

        unsafe {
            let region = if pill {
                CreateRoundRectRgn(left, 0, left + width, height, height, height)
            } else {
                // Keep the top edge flush with the monitor while rounding the lower corners.
                let rounded =
                    CreateRoundRectRgn(left, 0, left + width, height, radius * 2, radius * 2);
                let top = CreateRectRgn(left, 0, left + width, radius);
                if rounded.is_null() || top.is_null() {
                    return Err("failed to create native island region".to_string());
                }
                if CombineRgn(rounded, rounded, top, RGN_OR) == 0 {
                    return Err("failed to combine native island region".to_string());
                }
                DeleteObject(top);
                rounded
            };

            if region.is_null() {
                return Err("failed to create native island region".to_string());
            }
            // Windows takes ownership of the region handle after a successful call.
            if SetWindowRgn(hwnd.0, region, 1) == 0 {
                return Err("failed to set native island hit-test region".to_string());
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (window, logical_width, logical_height, pill);
    Ok(())
}

/// Creates (once) a small circular PNG used as the native drag preview icon.
#[tauri::command]
pub fn get_drag_icon() -> Result<String, String> {
    let mut path = std::env::temp_dir();
    path.push("pill-drag-icon.png");

    if !path.exists() {
        let mut image = image::RgbaImage::new(64, 64);

        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let dx = x as i32 - 32;
            let dy = y as i32 - 32;

            if dx * dx + dy * dy <= 30 * 30 {
                let fade = (64 - (x + y) / 2) as u8;
                *pixel = image::Rgba([69, 214, 255, fade.max(90)]);
            }
        }

        image
            .save_with_format(&path, image::ImageFormat::Png)
            .map_err(|e| format!("drag icon write failed: {e}"))?;
    }

    Ok(path.to_string_lossy().to_string())
}

fn focus_or_build(
    app: &AppHandle,
    label: &str,
    route: String,
    title: &str,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let app = app.clone();
    let label = label.to_string();
    let title = title.to_string();

    // Window/webview creation must happen on a fresh turn of the event loop.
    // When it runs inline inside an IPC callback (which already executes on the
    // main thread, inside a WebView2 message handler) WebView2 deadlocks while
    // creating the controller and the new window stays blank forever. Bouncing
    // through a worker thread makes Tauri queue the build instead.
    std::thread::spawn(move || {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.show();
            let _ = window.set_focus();
            return;
        }

        let builder_app = app.clone();
        let builder_label = label.clone();
        let main_window = app.get_webview_window("main");

        let dispatched = app.run_on_main_thread(move || {
            // Another request may have built this window while we were queued.
            if builder_app.get_webview_window(&builder_label).is_some() {
                return;
            }

            if let Some(main_window) = &main_window {
                let _ = main_window.hide();
            }

            let built = tauri::WebviewWindowBuilder::new(
                &builder_app,
                &builder_label,
                WebviewUrl::App(route.into()),
            )
            .title(title)
            .inner_size(width, height)
            .min_inner_size(420.0, 320.0)
            .resizable(true)
            .focused(true)
            .always_on_top(true)
            .skip_taskbar(true)
            // Undecorated on purpose: the native caption (title, close and
            // minimise buttons, painted frame) showed up below the island and
            // looked like a second, unrelated app. These windows draw their
            // own header instead, which also drags via data-tauri-drag-region.
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .center()
            .build();

            match built {
                Ok(window) => {
                    let app = builder_app.clone();
                    let _ = window.on_window_event(move |event| {
                        if matches!(event, tauri::WindowEvent::Destroyed) {
                            if let Some(main) = app.get_webview_window("main") {
                                let _ = main.show();
                                let _ = main.set_focus();
                            }
                        }
                    });
                }
                Err(error) => {
                    if let Some(main) = builder_app.get_webview_window("main") {
                        let _ = main.show();
                    }
                    eprintln!("window {builder_label} create failed: {error}");
                }
            }
        });

        if let Err(error) = dispatched {
            eprintln!("window {label} dispatch failed: {error}");
        }
    });

    Ok(())
}

/// Opens (or focuses) the standalone editor window for a single clip.
#[tauri::command]
pub fn open_clip_editor(app: AppHandle, id: i64) -> Result<(), String> {
    focus_or_build(
        &app,
        &format!("editor-{id}"),
        "index.html".to_string(),
        "Edit Clip",
        620.0,
        520.0,
    )
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) -> Result<(), String> {
    focus_or_build(
        &app,
        "settings",
        "index.html".to_string(),
        "Island Settings",
        480.0,
        560.0,
    )
}
