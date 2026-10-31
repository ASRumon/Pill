//! Native Windows drop target.
//!
//! Tauri's built-in handler replaces WebView2's drop target and only reports
//! file paths, so browser text/link/image drags never reach the frontend.
//! Registering our own `IDropTarget` lets us read the raw drag object instead:
//! `CF_HDROP`, `CF_UNICODETEXT`, `HTML Format` and `PNG`, all in one drop.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use windows::core::Result as WinResult;
use windows::Win32::Foundation::{HWND, LPARAM, POINTL};
use windows::Win32::System::Com::{
    IDataObject, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{
    IDropTarget, IDropTarget_Impl, OleInitialize, RegisterDragDrop, RevokeDragDrop, DROPEFFECT,
    DROPEFFECT_COPY, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GW_CHILD, GetWindow};

use super::DropPayload;

const CF_HDROP: u32 = 15;
const CF_UNICODETEXT: u32 = 13;

#[windows::core::implement(IDropTarget)]
struct IslandDropTarget {
    app: AppHandle,
}

impl IslandDropTarget {
    fn payload(&self, data: Option<&IDataObject>) -> Option<DropPayload> {
        let data = data?;
        let files = unsafe { read_files(data) };
        let text = unsafe { read_text(data, CF_UNICODETEXT) };
        let html = unsafe { read_html(data) };
        let url = text
            .as_deref()
            .filter(|value| looks_like_url(value))
            .map(|value| value.trim().to_string());
        let image = unsafe { read_png(data) };

        let has_something = !files.is_empty()
            || text.as_deref().map(|value| !value.trim().is_empty()).unwrap_or(false)
            || html.is_some()
            || image.is_some();

        if !has_something {
            return None;
        }

        Some(DropPayload {
            files,
            text,
            html,
            url,
            image_base64: image,
            image_mime: Some("image/png".to_string()),
        })
    }
}

fn looks_like_url(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.contains(char::is_whitespace)
        && (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
}

impl IDropTarget_Impl for IslandDropTarget_Impl {
    fn DragEnter(
        &self,
        pdataobj: windows::core::Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        let payload = self.payload(pdataobj.ok().ok());
        let _ = self.app.emit("drop-hover", true);
        let _ = payload;
        unsafe {
            if !effect.is_null() {
                *effect = DROPEFFECT_COPY;
            }
        }
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        unsafe {
            if !effect.is_null() {
                *effect = DROPEFFECT_COPY;
            }
        }
        Ok(())
    }

    fn DragLeave(&self) -> WinResult<()> {
        let _ = self.app.emit("drop-hover", false);
        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: windows::core::Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        let _ = self.app.emit("drop-hover", false);

        match self.payload(pdataobj.ok().ok()) {
            Some(payload) => {
                // The frontend ingests this through the normal command, so the
                // UI can expand and report progress.
                let _ = self.app.emit("drop-payload", payload);
                unsafe {
                    if !effect.is_null() {
                        *effect = DROPEFFECT_COPY;
                    }
                }
            }
            None => unsafe {
                if !effect.is_null() {
                    *effect = DROPEFFECT_NONE;
                }
            },
        }
        Ok(())
    }
}

unsafe fn fetch(data: &IDataObject, format: u32) -> Option<STGMEDIUM> {
    let spec = FORMATETC {
        cfFormat: format as u16,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    unsafe { data.GetData(&spec).ok() }
}

unsafe fn read_files(data: &IDataObject) -> Vec<String> {
    let mut files = Vec::new();
    let Some(medium) = (unsafe { fetch(data, CF_HDROP) }) else {
        return files;
    };

    let handle = unsafe { medium.u.hGlobal };
    let pointer = unsafe { GlobalLock(handle) };
    if !pointer.is_null() {
        let drop = HDROP(pointer);
        let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
        for index in 0..count {
            let length = unsafe { DragQueryFileW(drop, index, None) };
            if length == 0 {
                continue;
            }
            let mut buffer = vec![0u16; length as usize + 1];
            let written = unsafe { DragQueryFileW(drop, index, Some(&mut buffer)) };
            if written > 0 {
                files.push(String::from_utf16_lossy(&buffer[..written as usize]));
            }
        }
        let _ = unsafe { GlobalUnlock(handle) };
    }

    files
}

unsafe fn read_bytes(data: &IDataObject, format: u32) -> Option<Vec<u8>> {
    let medium = unsafe { fetch(data, format) }?;
    let handle = unsafe { medium.u.hGlobal };
    let pointer = unsafe { GlobalLock(handle) };
    if pointer.is_null() {
        return None;
    }
    let size = unsafe { GlobalSize(handle) };
    let bytes = if size == 0 {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(pointer as *const u8, size) }.to_vec())
    };
    let _ = unsafe { GlobalUnlock(handle) };
    bytes
}

unsafe fn read_text(data: &IDataObject, format: u32) -> Option<String> {
    let bytes = unsafe { read_bytes(data, format) }?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    if units.is_empty() {
        return None;
    }
    Some(String::from_utf16_lossy(&units))
}

/// `HTML Format` is a UTF-8 document preceded by a `StartHTML:` offset header.
unsafe fn read_html(data: &IDataObject) -> Option<String> {
    let format = unsafe { RegisterClipboardFormatW(&windows::core::HSTRING::from("HTML Format")) };
    if format == 0 {
        return None;
    }
    let bytes = unsafe { read_bytes(data, format) }?;
    let text = String::from_utf8_lossy(&bytes).to_string();

    let start = text
        .lines()
        .find_map(|line| line.strip_prefix("StartHTML:"))
        .and_then(|value| value.trim().parse::<usize>().ok());

    let html = match start {
        Some(offset) if offset < bytes.len() => String::from_utf8_lossy(&bytes[offset..]).to_string(),
        _ => text,
    };
    let trimmed = html.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// `PNG` is a registered clipboard format holding raw PNG bytes.
unsafe fn read_png(data: &IDataObject) -> Option<String> {
    use base64::Engine;
    let format = unsafe { RegisterClipboardFormatW(&windows::core::HSTRING::from("PNG")) };
    if format == 0 {
        return None;
    }
    let bytes = unsafe { read_bytes(data, format) }?;
    if bytes.len() < 8 || !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return None;
    }
    Some(base64::engine::general_purpose::STANDARD.encode(bytes))
}

unsafe extern "system" fn collect_child(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let list = unsafe { &mut *(lparam.0 as *mut Vec<isize>) };
    list.push(hwnd.0 as isize);
    true.into()
}

unsafe fn register(app: &AppHandle) {
    let _ = unsafe { OleInitialize(None) };

    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(hwnd) = window.hwnd() else {
        return;
    };

    let target: IDropTarget = IslandDropTarget { app: app.clone() }.into();

    let mut handles: Vec<isize> = vec![hwnd.0 as isize];
    let mut descendants: Vec<isize> = Vec::new();
    let _ = unsafe {
        EnumChildWindows(
            Some(hwnd),
            Some(collect_child),
            LPARAM(&mut descendants as *mut Vec<isize> as isize),
        )
    };

    // WebView2 usually hosts the drop target below the top-level window.
    let mut child = unsafe { GetWindow(hwnd, GW_CHILD) };
    while let Ok(current) = child {
        if current.0.is_null() {
            break;
        }
        handles.push(current.0 as isize);
        child = unsafe {
            GetWindow(
                current,
                windows::Win32::UI::WindowsAndMessaging::GW_HWNDNEXT,
            )
        };
    }
    handles.extend(descendants);
    handles.sort_unstable();
    handles.dedup();

    let mut registered = 0;
    for raw in &handles {
        let handle = HWND(*raw as *mut std::ffi::c_void);
        // Take the slot from WebView2 so our handler sees the raw data object.
        let _ = unsafe { RevokeDragDrop(handle) };
        if unsafe { RegisterDragDrop(handle, &target) }.is_ok() {
            registered += 1;
        }
    }
    eprintln!(
        "[drop-target] registered on {registered}/{} window(s)",
        handles.len()
    );

    // The registration owns a reference; keep ours alive for the app lifetime.
    std::mem::forget(target);
}

pub fn install(app: AppHandle) {
    std::thread::spawn(move || {
        // The webview creates its child windows asynchronously; try twice so a
        // slow first paint cannot leave the island without a drop target.
        for delay in [2500u64, 6000] {
            std::thread::sleep(Duration::from_millis(delay));
            let main = app.clone();
            let _ = app.run_on_main_thread(move || unsafe { register(&main) });
        }
    });
}
