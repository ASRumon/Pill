use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaTimeline {
    pub position_seconds: u32,
    pub duration_seconds: u32,
    pub playing: bool,
}

impl Default for MediaTimeline {
    fn default() -> Self {
        Self {
            position_seconds: 0,
            duration_seconds: 0,
            playing: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaState {
    pub title: String,
    pub artist: String,
    pub source: String,
    pub playing: bool,
    pub progress_seconds: u32,
    pub duration_seconds: u32,
    pub artwork_url: Option<String>,
}

impl Default for MediaState {
    fn default() -> Self {
        Self {
            title: "Nothing Playing".into(),
            artist: "Play media in a browser or app".into(),
            source: String::new(),
            playing: false,
            progress_seconds: 0,
            duration_seconds: 0,
            artwork_url: None,
        }
    }
}

#[cfg(windows)]
mod windows_media {
    use super::MediaState;
    use base64::Engine;
    use windows::{
        Media::Control::{
            GlobalSystemMediaTransportControlsSession,
            GlobalSystemMediaTransportControlsSessionManager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus,
        },
        Storage::Streams::DataReader,
        Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
    };

    struct Apartment;

    impl Apartment {
        fn initialize() -> windows::core::Result<Self> {
            unsafe { RoInitialize(RO_INIT_MULTITHREADED)?; }
            Ok(Self)
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) { unsafe { RoUninitialize(); } }
    }

    fn current_session() -> windows::core::Result<GlobalSystemMediaTransportControlsSession> {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.join()?;
        let sessions = manager.GetSessions()?;
        let mut fallback = None;
        for index in 0..sessions.Size()? {
            let session = sessions.GetAt(index)?;
            if session.GetPlaybackInfo()?.PlaybackStatus()? == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
                return Ok(session);
            }
            if fallback.is_none() { fallback = Some(session); }
        }
        match fallback {
            Some(session) => Ok(session),
            None => manager.GetCurrentSession(),
        }
    }

    fn artwork(session: &GlobalSystemMediaTransportControlsSession) -> Option<String> {
        let properties = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
        let reference = properties.Thumbnail().ok()?;
        let stream = reference.OpenReadAsync().ok()?.join().ok()?;
        let size = stream.Size().ok()?;
        if size == 0 || size > 4 * 1024 * 1024 { return None; }
        let input = stream.GetInputStreamAt(0).ok()?;
        let reader = DataReader::CreateDataReader(&input).ok()?;
        reader.LoadAsync(size as u32).ok()?.join().ok()?;
        let mut bytes = vec![0; size as usize];
        reader.ReadBytes(&mut bytes).ok()?;
        let mime = stream.ContentType().ok()?.to_string();
        Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    }

    /// Position only: skips thumbnail decoding and media properties, which are
    /// the expensive WinRT calls. Used by the seek bar twice a second.
    pub fn timeline() -> Result<super::MediaTimeline, String> {
        let _apartment = Apartment::initialize().map_err(|error| format!("WinRT initialization failed: {error}"))?;
        let session = current_session().map_err(|error| format!("No Windows media session is available: {error}"))?;
        let timeline = session.GetTimelineProperties().map_err(|error| error.to_string())?;
        let playback = session.GetPlaybackInfo().map_err(|error| error.to_string())?;
        let ticks_to_seconds = |ticks: i64| (ticks.max(0) / 10_000_000) as u32;
        Ok(super::MediaTimeline {
            position_seconds: ticks_to_seconds(timeline.Position().map_err(|error| error.to_string())?.Duration),
            duration_seconds: ticks_to_seconds(timeline.EndTime().map_err(|error| error.to_string())?.Duration),
            playing: playback.PlaybackStatus().map_err(|error| error.to_string())?
                == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing,
        })
    }

    /// Decoded artwork is a large base64 string. Re-reading the thumbnail on
    /// every poll (every few seconds) is the single most expensive WinRT call,
    /// so keep the last result keyed by track.
    static ARTWORK_CACHE: std::sync::Mutex<Option<(String, Option<String>)>> =
        std::sync::Mutex::new(None);

    fn artwork_cached(key: &str, session: &GlobalSystemMediaTransportControlsSession) -> Option<String> {
        let cached = ARTWORK_CACHE.lock().ok().and_then(|guard| guard.clone());
        if let Some((cached_key, value)) = cached {
            if cached_key == key {
                return value;
            }
        }
        let value = artwork(session);
        if let Ok(mut guard) = ARTWORK_CACHE.lock() {
            *guard = Some((key.to_string(), value.clone()));
        }
        value
    }

    pub fn read() -> Result<MediaState, String> {
        let _apartment = Apartment::initialize().map_err(|error| format!("WinRT initialization failed: {error}"))?;
        let session = current_session().map_err(|error| format!("No Windows media session is available: {error}"))?;
        let properties = session.TryGetMediaPropertiesAsync().map_err(|error| error.to_string())?
            .join().map_err(|error| error.to_string())?;
        let timeline = session.GetTimelineProperties().map_err(|error| error.to_string())?;
        let playback = session.GetPlaybackInfo().map_err(|error| error.to_string())?;
        let ticks_to_seconds = |ticks: i64| (ticks.max(0) / 10_000_000) as u32;
        let position = ticks_to_seconds(timeline.Position().map_err(|error| error.to_string())?.Duration);
        let duration = ticks_to_seconds(timeline.EndTime().map_err(|error| error.to_string())?.Duration);
        let app = session.SourceAppUserModelId().map(|id| id.to_string()).unwrap_or_default();
        let title = properties.Title().map(|value| value.to_string()).unwrap_or_else(|_| "Unknown track".into());
        let artist = {
            let value = properties.Artist().map(|value| value.to_string()).unwrap_or_default();
            if value.is_empty() {
                properties.AlbumArtist().map(|value| value.to_string()).unwrap_or_default()
            } else {
                value
            }
        };
        let artwork_url = artwork_cached(&format!("{title}|{artist}|{duration}"), &session);
        Ok(MediaState {
            title,
            artist,
            source: if app.is_empty() { "Media app".into() } else { app.rsplit(['!', '.']).next().unwrap_or(&app).to_string() },
            playing: playback.PlaybackStatus().map_err(|error| error.to_string())? == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing,
            progress_seconds: position,
            duration_seconds: duration,
            artwork_url,
        })
    }

    pub fn action(action: &str) -> bool {
        let Ok(_apartment) = Apartment::initialize() else { return false; };
        let Ok(session) = current_session() else { return false; };
        let operation = match action {
            "previous" => session.TrySkipPreviousAsync(),
            "play_pause" => session.TryTogglePlayPauseAsync(),
            "next" => session.TrySkipNextAsync(),
            _ => return false,
        };
        operation.and_then(|operation| operation.join()).unwrap_or(false)
    }
}

#[tauri::command]
pub async fn get_media_state() -> MediaState {
    #[cfg(windows)]
    {
        return windows_media::read().unwrap_or_else(|error| {
            eprintln!("Media session detection: {error}");
            MediaState { artist: error, ..MediaState::default() }
        });
    }
    #[cfg(not(windows))]
    { MediaState::default() }
}

#[tauri::command]
pub async fn get_media_timeline() -> MediaTimeline {
    #[cfg(windows)]
    {
        return windows_media::timeline().unwrap_or_default();
    }
    #[cfg(not(windows))]
    { MediaTimeline::default() }
}

#[tauri::command]
pub async fn media_action(action: String) -> bool {
    #[cfg(windows)]
    { return windows_media::action(&action); }
    #[cfg(not(windows))]
    { let _ = action; false }
}
