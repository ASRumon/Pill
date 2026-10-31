//! Explorer-quality previews (thumbnails first, then the real shell icon) and
//! OS "copy files" support, both on Windows only.
//!
//! Everything here runs on a worker thread (`spawn_blocking`), so blocking on
//! the shell is safe; COM is initialised per call because shell thumbnail
//! extraction is only reliable in a single-threaded apartment.

use std::ffi::c_void;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{HANDLE, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, ReleaseDC, BITMAPINFO, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, DROPFILES, SIIGBF, SIIGBF_BIGGERSIZEOK,
    SIIGBF_ICONONLY, SIIGBF_THUMBNAILONLY,
};

const EDGE: i32 = 256;

/// `CF_HDROP`: the clipboard format used for lists of dropped files.
const CF_HDROP_FORMAT: u32 = 15;

/// Extensions whose icons are per-file (embedded in the file), not per-type.
const PER_FILE_ICONS: [&str; 5] = ["exe", "ico", "lnk", "msi", "png"];

pub struct Preview {
    pub path: String,
    /// `cover` for real photos, `contain` for shell art with transparent edges.
    pub kind: &'static str,
}

struct ComApartment(bool);

impl ComApartment {
    fn init() -> Self {
        // S_FALSE (already initialised) is also fine; RPC_E_CHANGED_MODE means
        // the thread already lives in an MTA, which the shell APIs tolerate.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self(hr.is_ok())
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

/// Explorer's image factory is happiest when only one caller uses it at a time.
static SHELL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Asks Explorer for the file's thumbnail, then falls back to its real icon.
pub fn preview(path: &str, extension: &str, stamp: u128) -> Option<Preview> {
    let _guard = SHELL_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _apartment = ComApartment::init();

    let ext = if extension.is_empty() {
        "file"
    } else {
        extension
    };
    let icon_name = format!("icon-{ext}.png");
    let cache_icon = !PER_FILE_ICONS.contains(&extension);

    if cache_icon {
        let cached = super::preview_dir().join(&icon_name);
        if cached.is_file() {
            return Some(Preview {
                path: cached.to_string_lossy().to_string(),
                kind: "contain",
            });
        }
    }

    unsafe {
        if let Some(bitmap) = shell_bitmap(path, SIIGBF_BIGGERSIZEOK | SIIGBF_THUMBNAILONLY) {
            let image = bitmap_to_rgba(bitmap);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            if let Some(image) = image {
                if let Some(saved) = save_png(&image, &format!("thumb-{stamp}.png")) {
                    return Some(Preview {
                        path: saved,
                        kind: "contain",
                    });
                }
            }
        }

        if let Some(bitmap) = shell_bitmap(path, SIIGBF_BIGGERSIZEOK | SIIGBF_ICONONLY) {
            let image = bitmap_to_rgba(bitmap);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            if let Some(image) = image {
                let name = if cache_icon {
                    icon_name.clone()
                } else {
                    format!("icon-{stamp}-{ext}.png")
                };
                if let Some(saved) = save_png(&image, &name) {
                    return Some(Preview {
                        path: saved,
                        kind: "contain",
                    });
                }
            }
        }
    }

    None
}

unsafe fn shell_bitmap(
    path: &str,
    flags: SIIGBF,
) -> Option<windows::Win32::Graphics::Gdi::HBITMAP> {
    unsafe {
        let wide = HSTRING::from(path);
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;
        factory.GetImage(SIZE { cx: EDGE, cy: EDGE }, flags).ok()
    }
}

fn save_png(image: &image::RgbaImage, name: &str) -> Option<String> {
    let dir = super::preview_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let target = dir.join(name);
    image
        .save_with_format(&target, image::ImageFormat::Png)
        .ok()?;
    Some(target.to_string_lossy().to_string())
}

unsafe fn bitmap_to_rgba(
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Option<image::RgbaImage> {
    unsafe {
        let hdc = GetDC(None);
        if hdc.0.is_null() {
            return None;
        }

        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize =
            std::mem::size_of::<windows::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32;
        if GetDIBits(hdc, bitmap, 0, 0, None, &mut info, DIB_RGB_COLORS) == 0 {
            ReleaseDC(None, hdc);
            return None;
        }

        let width = info.bmiHeader.biWidth.max(1) as u32;
        let height = info.bmiHeader.biHeight.unsigned_abs().max(1);
        info.bmiHeader.biHeight = -(height as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;

        let mut buffer = vec![0u8; (width as usize) * (height as usize) * 4];
        let lines = GetDIBits(
            hdc,
            bitmap,
            0,
            height,
            Some(buffer.as_mut_ptr() as *mut c_void),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        if lines == 0 {
            return None;
        }

        // Shell bitmaps are BGRA with premultiplied alpha; PNG needs straight alpha.
        for pixel in buffer.chunks_exact_mut(4) {
            let alpha = pixel[3];
            if alpha == 0 {
                pixel[0] = 0;
                pixel[1] = 0;
                pixel[2] = 0;
            } else if alpha != 255 {
                pixel[0] = unpremultiply(pixel[0], alpha);
                pixel[1] = unpremultiply(pixel[1], alpha);
                pixel[2] = unpremultiply(pixel[2], alpha);
            }
            pixel.swap(0, 2);
        }

        image::RgbaImage::from_raw(width, height, buffer)
    }
}

fn unpremultiply(channel: u8, alpha: u8) -> u8 {
    ((channel as u32 * 255 + alpha as u32 / 2) / alpha as u32).min(255) as u8
}

/// Puts the given paths on the clipboard as a real file list (CF_HDROP), so
/// Ctrl+V into Explorer/Finder-like apps pastes the actual files.
pub fn copy_files(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("Nothing to copy.".to_string());
    }

    let mut payload: Vec<u16> = Vec::new();
    for path in paths {
        payload.extend(path.encode_utf16());
        payload.push(0);
    }
    payload.push(0);

    let header = std::mem::size_of::<DROPFILES>();
    let total = header + payload.len() * 2;

    unsafe {
        let block = GlobalAlloc(GMEM_MOVEABLE, total)
            .map_err(|e| format!("clipboard alloc failed: {e}"))?;
        let target = GlobalLock(block);
        if target.is_null() {
            return Err("clipboard lock failed".to_string());
        }

        let drop = DROPFILES {
            pFiles: header as u32,
            pt: POINT { x: 0, y: 0 },
            fNC: false.into(),
            fWide: true.into(),
        };
        std::ptr::copy_nonoverlapping(&drop as *const _ as *const u8, target as *mut u8, header);
        std::ptr::copy_nonoverlapping(
            payload.as_ptr() as *const u8,
            (target as *mut u8).add(header),
            payload.len() * 2,
        );
        let _ = GlobalUnlock(block);

        OpenClipboard(None).map_err(|e| format!("clipboard open failed: {e}"))?;
        let result = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|e| format!("clipboard clear failed: {e}"))?;
            SetClipboardData(CF_HDROP_FORMAT, Some(HANDLE(block.0)))
                .map_err(|e| format!("clipboard write failed: {e}"))?;
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}
