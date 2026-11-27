use seamone_core::{
    AppInfo,
    capture::Status,
    error_marks::{ErrorMark, ErrorMarkBatch, ErrorMarkPage},
    review::{LibraryPage, PlaybackReply, RecordingInfo, Request, SampleRange, Segment},
};
use seamone_runtime::LibraryService;
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, FileAccessMode, FilePath, PickerMode};

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
    state: State<'_, LibraryService>,
) -> Result<LibraryPage, String> {
    trusted(&window)?;
    state.library()
}

#[tauri::command]
pub async fn get_segments<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    recording_id: String,
    offset: u32,
) -> Result<seamone_core::review::SegmentPage, String> {
    trusted(&window)?;
    state.segments(&recording_id, offset)
}

#[tauri::command]
pub async fn import_audio<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<Option<RecordingInfo>, String> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = pick_audio(&window) else {
            return Ok(None);
        };
        trusted(&window)?;
        import_selected(&app, &window, path).map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
pub struct TestAudioPicker(pub std::sync::Mutex<std::collections::VecDeque<Option<FilePath>>>);

fn pick_audio<R: Runtime>(window: &WebviewWindow<R>) -> Option<FilePath> {
    #[cfg(test)]
    if let Some(picker) = window.app_handle().try_state::<TestAudioPicker>() {
        return picker.0.lock().unwrap().pop_front().flatten();
    }
    pick_audio_dialog(window)
}

fn pick_audio_dialog<R: Runtime>(window: &WebviewWindow<R>) -> Option<FilePath> {
    let dialog = window
        .app_handle()
        .dialog()
        .file()
        .add_filter("WAV, MP3, AAC-LC M4A", &["wav", "mp3", "m4a"])
        .set_picker_mode(PickerMode::Document)
        .set_file_access_mode(FileAccessMode::Scoped);
    #[cfg(desktop)]
    let dialog = dialog.set_parent(window);
    dialog.blocking_pick_file()
}

fn import_selected<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    selection: FilePath,
) -> Result<RecordingInfo, String> {
    let service = app.state::<LibraryService>();
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
                .map_err(|()| "선택한 파일 경로를 읽지 못했습니다.".to_string());
            let result = source.and_then(|source| {
                input
                    .and_then(|file| service.import_file(&source, file, || trusted(window).is_ok()))
            });
            let _ = fs.stop_accessing_security_scoped_resource(selection);
            return result;
        }
        #[cfg(target_os = "android")]
        if url.scheme() == "content" {
            return service.import_stream("가져온 음성".into(), input?, || {
                trusted(window).is_ok()
            });
        }
    }
    let path = selection
        .into_path()
        .map_err(|e| format!("선택한 파일 경로를 읽지 못했습니다. {e}"))?;
    service.import_path(&path, || trusted(window).is_ok())
}

#[tauri::command]
pub async fn get_capture_status<R: Runtime>(window: WebviewWindow<R>) -> Result<Status, String> {
    trusted(&window)?;
    Ok(window.state::<LibraryService>().capture_status())
}

#[cfg(native_capture)]
#[tauri::command]
pub async fn start_recording<R: Runtime>(window: WebviewWindow<R>) -> Result<Status, String> {
    trusted(&window)?;
    window.state::<LibraryService>().start_recording()
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
        window
            .state::<LibraryService>()
            .finish_recording(&session_id, save)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn transcribe_recording<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    recording_id: String,
) -> Result<Vec<Segment>, String> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryService>().transcribe(&recording_id)
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
        if !LibraryService::inference_available() { return Err("이 설치본에는 Whisper 엔진이 없습니다. desktop-whisper feature로 만든 데스크톱 설치본을 사용하세요.".into()); }
        let Some(path) = app.dialog().file().add_filter("Whisper small GGML", &["bin"]).blocking_pick_file() else { return Ok(false); };
        app.state::<LibraryService>().select_model(&path.into_path().map_err(|e| e.to_string())?)?;
        Ok(true)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_playback<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    segment_id: String,
    revision: u64,
) -> Result<PlaybackReply, String> {
    trusted(&window)?;
    state.playback(&segment_id, revision)
}

#[tauri::command]
pub async fn get_recording_playback<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    recording_id: String,
    revision: u64,
    range: SampleRange,
) -> Result<String, String> {
    trusted(&window)?;
    state.recording_playback(&recording_id, revision, range)
}

#[tauri::command]
pub async fn get_error_marks<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    recording_id: String,
    offset: u32,
) -> Result<ErrorMarkPage, String> {
    trusted(&window)?;
    state.error_marks(&recording_id, offset)
}

#[tauri::command]
pub async fn save_error_mark<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    mark: ErrorMark,
) -> Result<(), String> {
    trusted(&window)?;
    state.save_error_mark(&mark)
}

#[tauri::command]
pub async fn save_error_marks<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    batch: ErrorMarkBatch,
) -> Result<(), String> {
    trusted(&window)?;
    state.save_error_marks(&batch)
}

#[tauri::command]
pub async fn playback_ended<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    token: String,
) -> Result<(), String> {
    trusted(&window)?;
    state.playback_ended(&token)
}

#[tauri::command]
pub async fn save_review<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    request: Request,
    proof: Option<String>,
) -> Result<Segment, String> {
    trusted(&window)?;
    state.review(&request, proof.as_deref())
}

#[tauri::command]
pub async fn dismiss_info<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
) -> Result<(), String> {
    trusted(&window)?;
    state.dismiss_info()
}

#[tauri::command]
pub async fn export_text<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    recording_id: String,
) -> Result<String, String> {
    trusted(&window)?;
    state.export_text(&recording_id)
}

#[tauri::command]
pub async fn save_text_file<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    recording_id: String,
) -> Result<bool, String> {
    trusted(&window)?;
    let text = app.state::<LibraryService>().export_text(&recording_id)?;
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
    let text = app.state::<LibraryService>().export_text(&recording_id)?;
    app.clipboard()
        .write_text(text)
        .map_err(|e| format!("복사하지 못했습니다: {e}. 텍스트 파일 저장을 사용할 수 있습니다."))
}
