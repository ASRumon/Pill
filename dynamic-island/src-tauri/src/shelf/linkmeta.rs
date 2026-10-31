//! Link previews for URL shelf items.
//!
//! Fetches the page, extracts title / description / site name / Open Graph
//! image / favicon with a small purpose-built HTML scanner, caches the images
//! next to the other thumbnails and stores the result on the shelf row.
//!
//! Requests go through `crate::http` (WinHTTP), which follows redirects and
//! reports the content type so non-HTML responses are skipped.

use crate::shelf::{now_millis, KIND_URL};
use crate::state::{AppState, ShelfEntry};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

// ------------------------------------------------------------- HTML scan ---

fn decode_entities(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(position) = rest.find('&') {
        output.push_str(&rest[..position]);
        let tail = &rest[position..];
        let Some(end) = tail.find(';').filter(|end| *end <= 12) else {
            output.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "hellip" => Some('…'),
            _ => {
                if let Some(hex) = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                } else if let Some(decimal) = entity.strip_prefix('#') {
                    decimal.parse::<u32>().ok().and_then(char::from_u32)
                } else {
                    None
                }
            }
        };
        match decoded {
            Some(value) => output.push(value),
            None => {
                output.push('&');
                output.push_str(&tail[1..=end]);
            }
        }
        rest = &tail[end + 1..];
    }
    output.push_str(rest);
    output
}

fn attributes(tag: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = tag.chars().collect();
    let mut result = Vec::new();
    let mut index = 0;
    let is_name = |c: char| c.is_alphanumeric() || matches!(c, '-' | '_' | ':');
    while index < chars.len() {
        while index < chars.len() && !is_name(chars[index]) {
            index += 1;
        }
        let start = index;
        while index < chars.len() && is_name(chars[index]) {
            index += 1;
        }
        if start == index {
            break;
        }
        let name: String = chars[start..index].iter().collect::<String>().to_lowercase();
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        let mut value = String::new();
        if index < chars.len() && chars[index] == '=' {
            index += 1;
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            if index < chars.len() && (chars[index] == '"' || chars[index] == '\'') {
                let quote = chars[index];
                index += 1;
                let start = index;
                while index < chars.len() && chars[index] != quote {
                    index += 1;
                }
                value = chars[start..index.min(chars.len())].iter().collect();
                if index < chars.len() {
                    index += 1;
                }
            } else {
                let start = index;
                while index < chars.len() && !chars[index].is_whitespace() && chars[index] != '>' {
                    index += 1;
                }
                value = chars[start..index].iter().collect();
            }
        }
        result.push((name, value));
    }
    result
}

fn find_meta(html: &str, keys: &[&str]) -> Option<String> {
    let bytes = html.as_bytes();
    let mut index = 0;
    while let Some(offset) = html[index..].find('<') {
        let start = index + offset;
        if start + 5 <= html.len() && bytes[start..start + 5].eq_ignore_ascii_case(b"<meta") {
            let Some(tag_end) = html[start..].find('>') else {
                break;
            };
            let end = start + tag_end;
            let parsed = attributes(&html[start..end]);
            let key = parsed
                .iter()
                .find(|(name, _)| name == "property" || name == "name")
                .map(|(_, value)| value.trim().to_lowercase());
            if let Some(key) = key {
                if keys.iter().any(|wanted| key == *wanted) {
                    if let Some((_, content)) =
                        parsed.iter().find(|(name, _)| name == "content")
                    {
                        let value = decode_entities(content).trim().to_string();
                        if !value.is_empty() {
                            return Some(value);
                        }
                    }
                }
            }
            index = end + 1;
        } else {
            index = start + 1;
        }
    }
    None
}

fn find_title(html: &str) -> Option<String> {
    // ASCII lowercasing preserves byte offsets, unlike `to_lowercase`.
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open_end = lower[start..].find('>')? + start + 1;
    let close = lower[open_end..].find("</title>")? + open_end;
    let value = decode_entities(&html[open_end..close]).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn find_icon(html: &str) -> Option<String> {
    let bytes = html.as_bytes();
    let mut index = 0;
    let mut fallback = None;
    while let Some(offset) = html[index..].find('<') {
        let start = index + offset;
        if start + 5 <= html.len() && bytes[start..start + 5].eq_ignore_ascii_case(b"<link") {
            let Some(tag_end) = html[start..].find('>') else {
                break;
            };
            let end = start + tag_end;
            let parsed = attributes(&html[start..end]);
            let rel = parsed
                .iter()
                .find(|(name, _)| name == "rel")
                .map(|(_, value)| value.to_lowercase())
                .unwrap_or_default();
            let href = parsed
                .iter()
                .find(|(name, _)| name == "href")
                .map(|(_, value)| value.clone());
            if let Some(href) = href {
                if !href.trim().is_empty() {
                    if rel.split_whitespace().any(|part| part == "icon") {
                        // Prefer a real icon over apple-touch/fluid icons.
                        if rel.contains("shortcut") || rel == "icon" {
                            return Some(href);
                        }
                        fallback.get_or_insert(href);
                    }
                }
            }
            index = end + 1;
        } else {
            index = start + 1;
        }
    }
    fallback
}

fn origin_of(url: &str) -> String {
    if let Some(endpoint) = crate::http::parse_url(url) {
        let scheme = if endpoint.secure { "https" } else { "http" };
        let default_port = if endpoint.secure { 443 } else { 80 };
        return if endpoint.port == default_port {
            format!("{scheme}://{}", endpoint.host)
        } else {
            format!("{scheme}://{}:{}", endpoint.host, endpoint.port)
        };
    }
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(without_scheme)
        .to_string()
}

fn resolve_url(base: &str, target: &str) -> String {
    let target = target.trim();
    if target.starts_with("http://") || target.starts_with("https://") {
        return target.to_string();
    }
    let origin = origin_of(base);
    if let Some(rest) = target.strip_prefix("//") {
        return format!("https:{rest}");
    }
    if target.starts_with('/') {
        return format!("{origin}{target}");
    }
    let without_query = base.split(['?', '#']).next().unwrap_or(base);
    let directory = without_query
        .rfind('/')
        .map(|index| &without_query[..index + 1])
        .unwrap_or(base);
    format!("{directory}{target}")
}

fn thumbnail_dir() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push("dynamic-island-thumbs");
    dir
}

fn download_image(id: &str, prefix: &str, url: &str) -> Option<String> {
    let (status, content_type, body) = crate::http::get(url, 3 * 1024 * 1024).ok()?;
    if !(200..300).contains(&status) || body.len() < 64 {
        return None;
    }
    let declared_image = content_type.starts_with("image/") || content_type.is_empty();
    if !declared_image {
        return None;
    }
    let extension: String = if content_type.contains("png") {
        "png".to_string()
    } else if content_type.contains("jpeg") || content_type.contains("jpg") {
        "jpg".to_string()
    } else if content_type.contains("webp") {
        "webp".to_string()
    } else if content_type.contains("gif") {
        "gif".to_string()
    } else if content_type.contains("svg") {
        "svg".to_string()
    } else if let Some(extension) = url
        .split(['?', '#'])
        .next()
        .and_then(|path| path.rsplit_once('.'))
        .map(|(_, extension)| extension.to_lowercase())
        .filter(|extension| {
            matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg")
        })
    {
        if extension == "jpeg" { "jpg".to_string() } else { extension }
    } else {
        "png".to_string()
    };

    let dir = thumbnail_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let target = dir.join(format!("link-{prefix}-{id}.{extension}"));
    std::fs::write(&target, body).ok()?;
    Some(target.to_string_lossy().to_string())
}

// --------------------------------------------------------------- refresh ---

fn host_label(url: &str) -> String {
    origin_of(url)
        .split_once("://")
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| origin_of(url))
        .trim_start_matches("www.")
        .to_string()
}

fn sanitize_title(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}

/// Fetches metadata for one link. Runs on a worker thread; never blocks the UI.
///
/// A custom title always wins, and failures leave the URL (and any previous
/// preview) untouched.
pub fn refresh(app: &AppHandle, state: &AppState, id: &str, force: bool) -> Result<ShelfEntry, String> {
    let entry = {
        let shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        shelf
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
            .ok_or_else(|| "shelf item not found".to_string())?
    };

    if entry.kind != KIND_URL {
        return Ok(entry);
    }

    const REFRESH_AFTER_MS: i64 = 7 * 24 * 3_600_000;
    if !force && entry.meta_fetched_at > 0 && now_millis() - entry.meta_fetched_at < REFRESH_AFTER_MS {
        return Ok(entry);
    }

    let url = entry.content.trim().to_string();
    let mut updated = entry.clone();
    updated.meta_fetched_at = now_millis();

    match crate::http::get(&url, 1_500_000) {
        Ok((status, content_type, body)) if (200..400).contains(&status) => {
            let html = String::from_utf8_lossy(&body).to_string();
            let is_html = content_type.is_empty() || content_type.contains("html") || content_type.contains("xml");
            if is_html {
                if let Some(title) = find_meta(&html, &["og:title", "twitter:title"])
                    .or_else(|| find_title(&html))
                {
                    updated.meta_title = sanitize_title(&title);
                }
                if let Some(description) =
                    find_meta(&html, &["og:description", "twitter:description", "description"])
                {
                    updated.meta_description = description.chars().take(280).collect();
                }
                if let Some(site) = find_meta(&html, &["og:site_name", "application-name"]) {
                    updated.meta_site = site.chars().take(80).collect();
                }

                let image = find_meta(
                    &html,
                    &["og:image", "og:image:url", "og:image:secure_url", "twitter:image"],
                )
                .map(|value| resolve_url(&url, &value));
                if let Some(image_url) = image {
                    if let Some(path) = download_image(&entry.id, "thumb", &image_url) {
                        updated.meta_image = path;
                    }
                }

                let icon = find_icon(&html)
                    .map(|value| resolve_url(&url, &value))
                    .unwrap_or_else(|| format!("{}/favicon.ico", origin_of(&url)));
                if let Some(path) = download_image(&entry.id, "icon", &icon) {
                    updated.meta_favicon = path;
                }
            }
        }
        Ok((status, _, _)) => {
            eprintln!("Link metadata: {url} returned HTTP {status}");
        }
        Err(error) => {
            eprintln!("Link metadata: {url} failed: {error}");
        }
    }

    if updated.meta_site.is_empty() {
        updated.meta_site = host_label(&url);
    }
    // Never overwrite a title the user typed in.
    if updated.custom_title.is_empty() {
        updated.name = if updated.meta_title.is_empty() {
            host_label(&url)
        } else {
            sanitize_title(&updated.meta_title)
        };
    }

    {
        let mut shelf = state
            .shelf
            .lock()
            .map_err(|_| "inbox lock failed".to_string())?;
        match shelf.iter_mut().find(|item| item.id == updated.id) {
            Some(slot) => {
                *slot = updated.clone();
                crate::shelf::sort_shelf(&mut shelf);
            }
            // The item was removed while the request was in flight; do not
            // write it back into the shelf.
            None => return Ok(updated),
        }
    }
    crate::shelf::store_insert(state, &updated)?;
    let _ = app.emit("shelf-changed", ());
    Ok(updated)
}
