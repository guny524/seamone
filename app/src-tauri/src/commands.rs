use crate::{
    audio::{AudioLibrary, StagedAudio},
    audio_ingest,
    store::Store,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use seamone_core::{
    AppInfo,
    capture::CaptureStatus,
    error_marks::{ErrorMark, ErrorMarkBatch, ErrorMarkPage},
    review::{
        LibraryPage, PlaybackReply, RecordingInfo, ReviewAction, ReviewError, ReviewRequest,
        ReviewSegment, SampleRange,
    },
};
use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, FileAccessMode, FilePath, PickerMode};
use uuid::Uuid;

struct Listening {
    segment_id: String,
    revision: u64,
    complete: bool,
}

pub struct AppState {
    pub store: std::sync::Arc<Mutex<Store>>,
    pub background: crate::training::BackgroundWork,
    #[cfg(native_capture)]
    pub recorder: crate::capture::Recorder,
    listening: Mutex<HashMap<String, Listening>>,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        let store = std::sync::Arc::new(Mutex::new(store));
        let background = crate::training::BackgroundWork::new(&store, None);
        Self {
            store,
            background,
            #[cfg(native_capture)]
            recorder: crate::capture::Recorder::default(),
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
pub async fn import_audio<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<Option<RecordingInfo>, String> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = pick_audio(&app, &window) else {
            return Ok(None);
        };
        trusted(&window)?;
        let state = app.state::<AppState>();
        let _foreground = state.background.foreground.enter();
        let library = state.store()?.audio.clone();
        let (name, mut audio) = decode_selected(&app, &library, path)?;
        state
            .store()?
            .save_audio(name, &mut audio, || trusted(&window).is_ok())
            .map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
pub struct TestAudioPicker(pub Mutex<std::collections::VecDeque<Option<FilePath>>>);

fn pick_audio<R: Runtime>(
    app: &AppHandle<R>,
    #[cfg_attr(
        not(desktop),
        expect(
            unused_variables,
            reason = "Only desktop dialogs take a parent window."
        )
    )]
    window: &WebviewWindow<R>,
) -> Option<FilePath> {
    #[cfg(test)]
    if let Some(picker) = app.try_state::<TestAudioPicker>() {
        return picker.0.lock().unwrap().pop_front().flatten();
    }
    let dialog = app
        .dialog()
        .file()
        .add_filter("WAV, MP3, AAC-LC M4A", &["wav", "mp3", "m4a"])
        .set_picker_mode(PickerMode::Document)
        .set_file_access_mode(FileAccessMode::Scoped);
    #[cfg(desktop)]
    let dialog = dialog.set_parent(window);
    dialog.blocking_pick_file()
}

fn decode_selected<R: Runtime>(
    #[cfg_attr(
        not(mobile),
        expect(
            unused_variables,
            reason = "Only mobile document URIs require the app filesystem plugin."
        )
    )]
    app: &AppHandle<R>,
    library: &AudioLibrary,
    selection: FilePath,
) -> Result<(String, StagedAudio), String> {
    #[cfg(mobile)]
    if let FilePath::Url(url) = &selection {
        use tauri_plugin_fs::{FsExt, OpenOptions};
        let fs = app.fs();
        let mut options = OpenOptions::new();
        options.read(true);
        let input = fs
            .open(selection.clone(), options)
            .map_err(|e| format!("선택한 음성 파일을 열지 못했습니다. {e}"));
        #[cfg(target_os = "ios")]
        {
            let source = url
                .to_file_path()
                .map_err(|()| "선택한 파일 경로를 읽지 못했습니다.");
            let result = input.and_then(|file| audio_ingest::decode(library, file));
            let _ = fs.stop_accessing_security_scoped_resource(selection);
            return Ok((audio_ingest::display_name(&source?), result?));
        }
        #[cfg(target_os = "android")]
        if url.scheme() == "content" {
            use std::io::Read;
            // A document provider can return a pipe. Spool a bounded local copy for seeking.
            let mut input = input?.take(audio_ingest::MAX_FILE_BYTES + 1);
            let mut scratch = library.scratch_file()?;
            let bytes = std::io::copy(&mut input, &mut scratch).map_err(|e| e.to_string())?;
            if bytes > audio_ingest::MAX_FILE_BYTES {
                return Err(audio_ingest::SIZE_ERROR.into());
            }
            return Ok((
                "가져온 음성".into(),
                audio_ingest::decode(library, scratch)?,
            ));
        }
    }
    let path = selection
        .into_path()
        .map_err(|e| format!("선택한 파일 경로를 읽지 못했습니다. {e}"))?;
    let audio = audio_ingest::decode(library, library.open_source(&path)?)?;
    Ok((audio_ingest::display_name(&path), audio))
}

#[tauri::command]
pub async fn get_capture_status<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<CaptureStatus, String> {
    trusted(&window)?;
    #[cfg(native_capture)]
    return Ok(window.state::<AppState>().recorder.status());
    #[cfg(not(native_capture))]
    Ok(CaptureStatus::Unavailable)
}

#[cfg(native_capture)]
#[tauri::command]
pub async fn start_recording<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<CaptureStatus, String> {
    trusted(&window)?;
    let state = window.state::<AppState>();
    let library = state.store()?.audio.clone();
    state
        .recorder
        .start(library, state.background.foreground.enter())
}

#[cfg(native_capture)]
#[tauri::command]
pub async fn finish_recording<R: Runtime>(
    window: WebviewWindow<R>,
    session_id: String,
    save: bool,
) -> Result<Option<RecordingInfo>, String> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = window.state::<AppState>();
        let _foreground = state.background.foreground.enter();
        state.recorder.finish(&session_id, save, |audio, active| {
            state
                .store()?
                .save_audio("마이크 녹음.wav".into(), audio, active)
        })
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
pub async fn get_recording_playback<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    recording_id: String,
    revision: u64,
    range: SampleRange,
) -> Result<String, String> {
    trusted(&window)?;
    let store = state.store()?;
    let recording = store.recording(&recording_id)?;
    if recording.revision != revision {
        return Err(ReviewError::StaleRevision.to_string());
    }
    let wav = store
        .audio
        .range_wav(&recording_id, range, recording.samples)?;
    Ok(STANDARD.encode(wav))
}

#[tauri::command]
pub async fn get_error_marks<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    recording_id: String,
    offset: u32,
) -> Result<ErrorMarkPage, String> {
    trusted(&window)?;
    state.store()?.error_mark_page(&recording_id, offset)
}

#[tauri::command]
pub async fn save_error_mark<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    mark: ErrorMark,
) -> Result<(), String> {
    trusted(&window)?;
    state.store()?.save_error_mark(&mark)
}

#[tauri::command]
pub async fn save_error_marks<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    batch: ErrorMarkBatch,
) -> Result<(), String> {
    trusted(&window)?;
    state.store()?.save_error_marks(&batch)
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
