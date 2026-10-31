//! Minimal HTTP client used for link previews and update checks.
//!
//! On Windows the request goes through WinHTTP, which ships with the OS: no TLS
//! crate, proxy and redirect handling included. The URL parser is plain Rust so
//! the rest of the app can resolve relative links on every platform.

pub struct Endpoint {
    pub secure: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub fn parse_url(url: &str) -> Option<Endpoint> {
    let (secure, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        (false, url.strip_prefix("//")?)
    };
    let (authority, path) = match rest.find(['/', '?', '#']) {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
            (host, port.parse::<u16>().ok()?)
        }
        _ => (authority, if secure { 443 } else { 80 }),
    };
    if host.is_empty() {
        return None;
    }
    Some(Endpoint {
        secure,
        host: host.to_string(),
        port,
        path: path.to_string(),
    })
}

/// `(status, content_type, body)` with the body capped at `max_bytes`.
pub fn get(url: &str, max_bytes: usize) -> Result<(u16, String, Vec<u8>), String> {
    #[cfg(windows)]
    {
        win::get(url, max_bytes)
    }
    #[cfg(not(windows))]
    {
        let _ = (url, max_bytes);
        Err("HTTP requests are only available on Windows".to_string())
    }
}

#[cfg(windows)]
mod win {
    use super::parse_url;
    use windows_sys::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
        WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
        INTERNET_DEFAULT_HTTP_PORT, INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
        WINHTTP_FLAG_SECURE, WINHTTP_QUERY_CUSTOM, WINHTTP_QUERY_FLAG_NUMBER,
        WINHTTP_QUERY_STATUS_CODE,
    };

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn get(url: &str, max_bytes: usize) -> Result<(u16, String, Vec<u8>), String> {
        let endpoint = parse_url(url).ok_or_else(|| "unsupported url".to_string())?;
        let agent = wide("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Pill/2.0");
        let host = wide(&endpoint.host);
        let verb = wide("GET");
        let object = wide(&endpoint.path);
        let headers = wide(
            "Accept: application/json, text/html, application/xhtml+xml, application/xml;q=0.9, image/avif, image/webp, */*;q=0.8\r\nAccept-Language: en-US,en;q=0.9\r\n",
        );
        let port = if endpoint.secure {
            INTERNET_DEFAULT_HTTPS_PORT
        } else {
            INTERNET_DEFAULT_HTTP_PORT
        };
        let flags = if endpoint.secure {
            WINHTTP_FLAG_SECURE as u32
        } else {
            0
        };

        unsafe {
            let session = WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                std::ptr::null(),
                std::ptr::null(),
                0,
            );
            if session.is_null() {
                return Err("WinHttpOpen failed".to_string());
            }

            let outcome = (|| -> Result<(u16, String, Vec<u8>), String> {
                WinHttpSetTimeouts(session, 4_000, 4_000, 6_000, 10_000);
                let connection = WinHttpConnect(session, host.as_ptr(), port, 0);
                if connection.is_null() {
                    return Err("WinHttpConnect failed".to_string());
                }

                let outcome = (|| -> Result<(u16, String, Vec<u8>), String> {
                    let request = WinHttpOpenRequest(
                        connection,
                        verb.as_ptr(),
                        object.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        std::ptr::null(),
                        flags,
                    );
                    if request.is_null() {
                        return Err("WinHttpOpenRequest failed".to_string());
                    }

                    let outcome = (|| -> Result<(u16, String, Vec<u8>), String> {
                        if WinHttpSendRequest(
                            request,
                            headers.as_ptr(),
                            u32::MAX,
                            std::ptr::null(),
                            0,
                            0,
                            0,
                        ) == 0
                        {
                            return Err("request could not be sent".to_string());
                        }
                        if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
                            return Err("no response from server".to_string());
                        }

                        let mut status: u32 = 0;
                        let mut status_length = std::mem::size_of::<u32>() as u32;
                        WinHttpQueryHeaders(
                            request,
                            (WINHTTP_QUERY_STATUS_CODE as u32) | (WINHTTP_QUERY_FLAG_NUMBER as u32),
                            std::ptr::null(),
                            &mut status as *mut u32 as *mut std::ffi::c_void,
                            &mut status_length,
                            std::ptr::null_mut(),
                        );

                        let content_type_name = wide("Content-Type");
                        let mut buffer = [0u16; 256];
                        let mut length = (buffer.len() * 2) as u32;
                        let mut content_type = String::new();
                        if WinHttpQueryHeaders(
                            request,
                            WINHTTP_QUERY_CUSTOM as u32,
                            content_type_name.as_ptr(),
                            buffer.as_mut_ptr() as *mut std::ffi::c_void,
                            &mut length,
                            std::ptr::null_mut(),
                        ) != 0
                        {
                            let units = ((length / 2) as usize).min(buffer.len());
                            content_type = String::from_utf16_lossy(&buffer[..units])
                                .trim_matches(char::from(0))
                                .trim()
                                .to_lowercase();
                        }

                        let mut body = Vec::new();
                        let mut chunk = vec![0u8; 16 * 1024];
                        loop {
                            let mut read: u32 = 0;
                            if WinHttpReadData(
                                request,
                                chunk.as_mut_ptr() as *mut std::ffi::c_void,
                                chunk.len() as u32,
                                &mut read,
                            ) == 0
                                || read == 0
                            {
                                break;
                            }
                            body.extend_from_slice(&chunk[..read as usize]);
                            if body.len() >= max_bytes {
                                body.truncate(max_bytes);
                                break;
                            }
                        }

                        Ok((status as u16, content_type, body))
                    })();

                    WinHttpCloseHandle(request);
                    outcome
                })();

                WinHttpCloseHandle(connection);
                outcome
            })();

            WinHttpCloseHandle(session);
            outcome
        }
    }
}
