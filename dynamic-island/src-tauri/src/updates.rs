//! In-app update check.
//!
//! Reads the public release list of the Pill repository and reports a newer
//! version together with the action the user should take.
//!
//! One build serves both distribution paths. Where the copy came from is
//! resolved at runtime (`detect_source`) and everything downstream is driven by
//! the result, so no caller ever asks "is this the Store build?":
//!
//! | source      | how it was installed        | action                       |
//! |-------------|-----------------------------|------------------------------|
//! | `github`    | NSIS/MSI from the releases  | download the new installer   |
//! | `store`     | MSIX or the Store product   | open the Microsoft Store page |
//!
//! Store copies must never download or launch an installer themselves: the
//! install directory is owned by Windows and Store policy expects the Store to
//! deliver updates.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tauri::AppHandle;

const RELEASES_URL: &str = "https://api.github.com/repos/ASRumon/Pill/releases?per_page=10";

/// Used as the update destination for direct-download installs.
const RELEASES_PAGE: &str = "https://github.com/ASRumon/Pill/releases";

/// Partner Center product id (`apps.microsoft.com/detail/<id>`). Only needed for
/// unpackaged Store installs, where Windows knows no package family name.
const STORE_PRODUCT_ID: &str = "";

/// Store copies are updated by Windows. Keep `true` to show an "available" chip
/// that only opens the product page, `false` to leave Store installs silent.
const NOTIFY_STORE_INSTALLS: bool = true;

/// Where this copy of the app gets its updates from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateSource {
    /// Direct download: the app fetches and runs the installer itself.
    Github,
    /// Microsoft Store: Windows owns the update, the app only links to it.
    Store,
}

/// Resolved once per launch and shared by the chip and Settings.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateChannel {
    pub source: UpdateSource,
    /// Display name of the channel ("GitHub" / "Microsoft Store").
    pub label: String,
    pub current_version: String,
    /// Windows owns updates: never download or launch an installer.
    pub store_managed: bool,
    /// Windows sees this process as a packaged (MSIX) app.
    pub packaged: bool,
    /// Package family name of the MSIX package, empty when unpackaged.
    pub package_family_name: String,
    /// Where Settings sends the user for updates.
    pub channel_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub name: String,
    pub notes: String,
    pub release_url: String,
    pub download_name: String,
    pub published_at: String,
    /// Which channel produced this notice.
    pub source: UpdateSource,
    /// Button text, chosen by the backend for this install type.
    pub action_label: String,
    /// Installer download (direct) or the Store product page (Store).
    pub action_url: String,
}

#[derive(Clone)]
struct Install {
    packaged: bool,
    package_family_name: String,
}

/// The package family name of the MSIX package this process runs in.
///
/// `GetCurrentPackageFamilyName` fails with `APPMODEL_ERROR_NO_PACKAGE` for a
/// plain Win32 install, so a name is a reliable "this came from a package"
/// signal. Sideloaded MSIX builds report one too, which is correct: their
/// files are just as read-only and they cannot self-update either.
#[cfg(windows)]
fn package_family_name() -> Option<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
    use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFamilyName;

    let mut length = 0u32;
    let probe = unsafe { GetCurrentPackageFamilyName(&mut length, None) };
    if probe != ERROR_INSUFFICIENT_BUFFER || length == 0 {
        return None;
    }

    let mut buffer = vec![0u16; length as usize];
    let status = unsafe {
        GetCurrentPackageFamilyName(&mut length, Some(PWSTR(buffer.as_mut_ptr())))
    };
    if status != ERROR_SUCCESS {
        return None;
    }

    // The API writes a NUL-terminated string into the buffer it just sized.
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    let name = String::from_utf16_lossy(&buffer[..end]);
    (!name.trim().is_empty()).then_some(name)
}

#[cfg(not(windows))]
fn package_family_name() -> Option<String> {
    None
}

fn install() -> &'static Install {
    static INSTALL: OnceLock<Install> = OnceLock::new();
    INSTALL.get_or_init(|| {
        let package_family_name = package_family_name().unwrap_or_default();
        Install {
            packaged: !package_family_name.is_empty(),
            package_family_name,
        }
    })
}

/// `PILL_UPDATE_SOURCE=store|github` forces a channel without rebuilding, so the
/// Store UI can be checked on a direct-download install (and the other way).
fn forced_source() -> Option<UpdateSource> {
    let value = std::env::var("PILL_UPDATE_SOURCE").ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "store" | "msix" | "microsoft-store" => Some(UpdateSource::Store),
        "github" | "direct" | "web" => Some(UpdateSource::Github),
        _ => None,
    }
}

/// Runtime package identity first, then the build marker, then the env override.
fn detect_source(packaged: bool) -> UpdateSource {
    if let Some(forced) = forced_source() {
        return forced;
    }
    if packaged {
        return UpdateSource::Store;
    }
    if cfg!(feature = "store") {
        return UpdateSource::Store;
    }
    UpdateSource::Github
}

fn store_page_url(package_family_name: &str) -> String {
    if !package_family_name.is_empty() {
        format!("ms-windows-store://pdp/?PFN={package_family_name}")
    } else if !STORE_PRODUCT_ID.trim().is_empty() {
        format!("ms-windows-store://pdp/?productid={}", STORE_PRODUCT_ID.trim())
    } else {
        // No identity and no product id yet: a Store search beats a dead link.
        "https://apps.microsoft.com/search?query=Pill".to_string()
    }
}

pub fn channel(app: &AppHandle) -> UpdateChannel {
    let install = install().clone();
    let source = detect_source(install.packaged);

    UpdateChannel {
        source,
        label: match source {
            UpdateSource::Github => "GitHub".to_string(),
            UpdateSource::Store => "Microsoft Store".to_string(),
        },
        current_version: app.package_info().version.to_string(),
        store_managed: source == UpdateSource::Store,
        packaged: install.packaged,
        channel_url: match source {
            UpdateSource::Github => RELEASES_PAGE.to_string(),
            UpdateSource::Store => store_page_url(&install.package_family_name),
        },
        package_family_name: install.package_family_name,
    }
}

/// Lets the UI label itself without guessing where the app came from.
#[tauri::command]
pub fn update_channel(app: AppHandle) -> UpdateChannel {
    channel(&app)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// `1.2.3`, `v1.2.3` and `1.2.3-beta.1` all become comparable numbers.
fn version_parts(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .map(|part| {
            part.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .collect()
}

fn is_newer(candidate: &str, current: &str) -> bool {
    let candidate_parts = version_parts(candidate);
    let current_parts = version_parts(current);
    let length = candidate_parts.len().max(current_parts.len());
    for index in 0..length {
        let left = *candidate_parts.get(index).unwrap_or(&0);
        let right = *current_parts.get(index).unwrap_or(&0);
        if left != right {
            return left > right;
        }
    }
    false
}

/// Prefer the NSIS installer so one click downloads a runnable file, then the
/// MSI, and fall back to the release page when a release carries neither.
fn installer_of(release: &Release) -> (String, String) {
    let mut exe = None;
    let mut msi = None;
    for asset in &release.assets {
        let name = asset.name.to_lowercase();
        if name.ends_with(".exe") && exe.is_none() {
            exe = Some(asset);
        } else if name.ends_with(".msi") && msi.is_none() {
            msi = Some(asset);
        }
    }
    match exe.or(msi) {
        Some(asset) => (asset.browser_download_url.clone(), asset.name.clone()),
        None => (release.html_url.clone(), String::new()),
    }
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let channel = channel(&app);
    if channel.store_managed && !NOTIFY_STORE_INSTALLS {
        return Ok(None);
    }
    let current = channel.current_version.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<Option<UpdateInfo>, String> {
        let (status, _, body) = crate::http::get(RELEASES_URL, 1_500_000)?;
        if !(200..300).contains(&status) {
            return Err(format!("GitHub returned HTTP {status}"));
        }

        let releases: Vec<Release> = serde_json::from_slice(&body)
            .map_err(|error| format!("cannot read the release list: {error}"))?;

        // Newest published release wins, not merely the first entry returned.
        let mut latest: Option<Release> = None;
        for release in releases {
            if release.draft || release.prerelease {
                continue;
            }
            let is_highest = latest
                .as_ref()
                .map(|best| is_newer(&release.tag_name, &best.tag_name))
                .unwrap_or(true);
            if is_highest {
                latest = Some(release);
            }
        }

        let Some(release) = latest else {
            return Ok(None);
        };
        if !is_newer(&release.tag_name, &current) {
            return Ok(None);
        }

        // Same notice, different action: a Store copy points at the product
        // page because Windows owns both the install and the update.
        let (action_url, download_name) = match channel.source {
            UpdateSource::Store => (store_page_url(&channel.package_family_name), String::new()),
            UpdateSource::Github => installer_of(&release),
        };
        let action_label = match channel.source {
            UpdateSource::Store => "Open Microsoft Store".to_string(),
            UpdateSource::Github => "Download".to_string(),
        };
        Ok(Some(UpdateInfo {
            current_version: current,
            latest_version: release
                .tag_name
                .trim()
                .trim_start_matches(['v', 'V'])
                .to_string(),
            name: release
                .name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| release.tag_name.clone()),
            notes: release.body.unwrap_or_default().chars().take(600).collect(),
            release_url: release.html_url,
            download_name,
            published_at: release.published_at.unwrap_or_default(),
            source: channel.source,
            action_label,
            action_url,
        }))
    })
    .await
    .map_err(|error| format!("update check failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, assets: Vec<Asset>) -> Release {
        Release {
            tag_name: tag.to_string(),
            name: None,
            html_url: format!("https://github.com/ASRumon/Pill/releases/tag/{tag}"),
            body: None,
            published_at: None,
            draft: false,
            prerelease: false,
            assets,
        }
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("1.10.0", "1.9.0"));
        assert!(is_newer("v2.0.1", "2.0.0"));
        assert!(!is_newer("2.0.0", "2.0.0"));
        assert!(!is_newer("1.0.9", "1.0.10"));
        assert!(!is_newer("0.1.0", "2.0.0"));
    }

    #[test]
    fn prefers_the_exe_installer() {
        let release = release(
            "2.0.0",
            vec![
                Asset {
                    name: "Pill_2.0.0_x64_en-US.msi".into(),
                    browser_download_url: "https://example.com/a.msi".into(),
                },
                Asset {
                    name: "Pill_2.0.0_x64-setup.exe".into(),
                    browser_download_url: "https://example.com/a.exe".into(),
                },
            ],
        );
        assert_eq!(installer_of(&release).0, "https://example.com/a.exe");
    }

    #[test]
    fn falls_back_to_the_release_page_without_assets() {
        let release = release("2.0.0", Vec::new());
        let (url, name) = installer_of(&release);
        assert_eq!(url, "https://github.com/ASRumon/Pill/releases/tag/2.0.0");
        assert!(name.is_empty());
    }

    #[test]
    fn store_links_prefer_the_package_family_name() {
        assert_eq!(
            store_page_url("ASRumon.Pill_8wekyb3d8bbwe"),
            "ms-windows-store://pdp/?PFN=ASRumon.Pill_8wekyb3d8bbwe"
        );
    }

    #[test]
    fn packaged_builds_use_the_store_channel() {
        if forced_source().is_some() {
            // PILL_UPDATE_SOURCE intentionally overrides the detection.
            return;
        }
        assert_eq!(detect_source(true), UpdateSource::Store);
        let unpackaged = if cfg!(feature = "store") {
            UpdateSource::Store
        } else {
            UpdateSource::Github
        };
        assert_eq!(detect_source(false), unpackaged);
    }

    #[test]
    fn parses_the_github_release_list() {
        let json = r#"[
            {"tag_name":"2.1.0","name":"Pill 2.1.0","html_url":"https://github.com/ASRumon/Pill/releases/tag/2.1.0","body":"notes","published_at":"2026-11-01T00:00:00Z","draft":false,"prerelease":false,"assets":[{"name":"Pill_2.1.0_x64-setup.exe","browser_download_url":"https://github.com/ASRumon/Pill/releases/download/2.1.0/Pill_2.1.0_x64-setup.exe"}]},
            {"tag_name":"2.0.1","html_url":"https://github.com/ASRumon/Pill/releases/tag/2.0.1","draft":true,"prerelease":false,"assets":[]}
        ]"#;
        let releases: Vec<Release> = serde_json::from_str(json).expect("release list parses");
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].assets[0].name, "Pill_2.1.0_x64-setup.exe");
        assert!(releases[1].draft);
    }
}
