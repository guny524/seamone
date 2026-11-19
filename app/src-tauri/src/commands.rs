use crate::store::Store;
use base64::{Engine, engine::general_purpose::STANDARD};
use seamone_core::{
    AppInfo,
    review::{
        LibraryPage, PlaybackReply, RecordingInfo, ReviewAction, ReviewRequest, ReviewSegment,
    },
};
use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

struct Listening {
    segment_id: String,
    revision: u64,
    complete: bool,
}

pub struct AppState {
    pub store: std::sync::Arc<Mutex<Store>>,
    pub background: crate::training::BackgroundWork,
    listening: Mutex<HashMap<String, Listening>>,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        let store = std::sync::Arc::new(Mutex::new(store));
        let background = crate::training::BackgroundWork::new(&store, None);
        Self {
            store,
            background,
            listening: Mutex::default(),
        }
    }
    pub fn store(&self) -> Result<MutexGuard<'_, Store>, String> {
        self.store
            .lock()
            .map_err(|_| "보관함을 다시 여세요.".into())
    }

    pub fn playback(&self, id: &str, revision: u64) -> Result<PlaybackReply, String> {
        let store = self.store()?;
        let segment = store.segment(id)?;
        if segment.revision != revision {
            return Err(seamone_core::review::ReviewError::StaleRevision.to_string());
        }
        let recording = store.recording(&segment.recording_id)?;
        let wav = store
            .audio
            .range_wav(&segment.recording_id, segment.range, recording.samples)?;
        let token = Uuid::new_v4().to_string();
        let mut listening = self.listening.lock().map_err(|e| e.to_string())?;
        if listening.len() >= 128 {
            listening.clear();
        }
        listening.insert(
            token.clone(),
            Listening {
                segment_id: id.into(),
                revision,
                complete: false,
            },
        );
        Ok(PlaybackReply {
            segment_id: id.into(),
            revision,
            wav_base64: STANDARD.encode(wav),
            token,
        })
    }

    pub fn playback_ended(&self, token: &str) -> Result<(), String> {
        let mut listening = self.listening.lock().map_err(|e| e.to_string())?;
        let item = listening
            .get_mut(token)
            .ok_or("재생이 바뀌었습니다. 이 조각을 다시 들으세요.")?;
        item.complete = true;
        Ok(())
    }

    pub fn review(
        &self,
        request: &ReviewRequest,
        proof: Option<&str>,
    ) -> Result<ReviewSegment, String> {
        let mut store = self.store()?;
        if let Some(replayed) = store.replay(request)? {
            return Ok(replayed);
        }
        if matches!(
            request.action,
            ReviewAction::Judge { .. }
                | ReviewAction::ConfirmCorrection
                | ReviewAction::CorrectAndConfirm { .. }
        ) {
            let listening = self.listening.lock().map_err(|e| e.to_string())?;
            let valid = proof.and_then(|p| listening.get(p)).is_some_and(|l| {
                l.complete
                    && l.segment_id == request.segment_id
                    && l.revision == request.expected_revision
            });
            if !valid {
                return Err("현재 수정본의 음성을 끝까지 들은 뒤 확인하세요.".into());
            }
        }
        let saved = store.mutate(request)?;
        drop(store);
        if saved.verified.is_some() {
            self.background.notify();
        }
        Ok(saved)
    }
}

/// Capabilities restrict these commands to bundled content; this also rejects other windows and navigation.
fn trusted<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    let url = window.url().map_err(|e| e.to_string())?;
    let local = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https") && url.host_str() == Some("tauri.localhost"));
    let development = cfg!(debug_assertions)
        && url.scheme() == "http"
        && url.host_str() == Some("localhost")
        && url.port() == Some(1420);
    if window.label() != "main" || !(local || development) {
        return Err("이 창에서는 보관함에 접근할 수 없습니다.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_app_info<R: Runtime>(window: WebviewWindow<R>) -> Result<AppInfo, String> {
    trusted(&window)?;
    Ok(seamone_core::app_info())
}

#[tauri::command]
pub async fn get_library<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<LibraryPage, String> {
    trusted(&window)?;
    state.store()?.library(crate::inference::available())
}

#[tauri::command]
pub async fn get_segments<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    recording_id: String,
    offset: u32,
) -> Result<seamone_core::review::SegmentPage, String> {
    trusted(&window)?;
    state.store()?.segment_page(&recording_id, offset)
}

#[tauri::command]
pub async fn import_wav<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<Option<RecordingInfo>, String> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = app
            .dialog()
            .file()
            .add_filter("16kHz mono PCM16 WAV", &["wav"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        app.state::<AppState>().store()?.import(&path).map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn transcribe_recording<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    recording_id: String,
) -> Result<Vec<ReviewSegment>, String> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::inference::transcribe(&app.state::<AppState>(), &recording_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn select_model<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<bool, String> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        if !crate::inference::available() { return Err("이 설치본에는 Whisper 엔진이 없습니다. desktop-whisper feature로 만든 데스크톱 설치본을 사용하세요.".into()); }
        let Some(path) = app.dialog().file().add_filter("Whisper small GGML", &["bin"]).blocking_pick_file() else { return Ok(false); };
        crate::inference::select(&app.state::<AppState>(), &path.into_path().map_err(|e| e.to_string())?)?;
        Ok(true)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_playback<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    segment_id: String,
    revision: u64,
) -> Result<PlaybackReply, String> {
    trusted(&window)?;
    state.playback(&segment_id, revision)
}

#[tauri::command]
pub async fn playback_ended<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    token: String,
) -> Result<(), String> {
    trusted(&window)?;
    state.playback_ended(&token)
}

#[tauri::command]
pub async fn save_review<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    request: ReviewRequest,
    proof: Option<String>,
) -> Result<ReviewSegment, String> {
    trusted(&window)?;
    state.review(&request, proof.as_deref())
}

#[tauri::command]
pub async fn dismiss_info<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    trusted(&window)?;
    state.store()?.set_setting("info_seen", "1")
}

#[tauri::command]
pub async fn export_text<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    recording_id: String,
) -> Result<String, String> {
    trusted(&window)?;
    state.store()?.export_text(&recording_id)
}

#[tauri::command]
pub async fn save_text_file<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    recording_id: String,
) -> Result<bool, String> {
    trusted(&window)?;
    let text = app
        .state::<AppState>()
        .store()?
        .export_text(&recording_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = app
            .dialog()
            .file()
            .set_file_name("seamone.txt")
            .add_filter("Text", &["txt"])
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let parent = path
            .parent()
            .ok_or("저장 폴더가 없습니다.")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("txt"))
            || parent.starts_with(data_dir)
            || path.is_symlink()
        {
            return Err("앱 보관함 밖에 새 텍스트 파일로 저장하세요.".into());
        }
        std::fs::write(path, text).map_err(|e| e.to_string())?;
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn copy_saved_text<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    recording_id: String,
) -> Result<(), String> {
    trusted(&window)?;
    let text = app
        .state::<AppState>()
        .store()?
        .export_text(&recording_id)?;
    app.clipboard()
        .write_text(text)
        .map_err(|e| format!("복사하지 못했습니다: {e}. 텍스트 파일 저장을 사용할 수 있습니다."))
}
