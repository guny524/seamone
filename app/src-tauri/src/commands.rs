use seamone_core::{
    ipc::{TypedCommand, command},
    review::RecordingInfo,
};
use seamone_runtime::LibraryService;
use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow, ipc};
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
pub async fn get_app_info<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<<command::GetAppInfo as TypedCommand>::Reply, String> {
    trusted(&window)?;
    Ok(seamone_core::app_info())
}

#[tauri::command]
pub async fn get_library<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
) -> Result<<command::GetLibrary as TypedCommand>::Reply, String> {
    trusted(&window)?;
    state.library()
}

#[tauri::command]
pub async fn get_segments<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    message: ipc::Request<'_>,
) -> Result<<command::GetSegments as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::GetSegments>(message.body())?;
    trusted(&window)?;
    state.segments(&args.recording_id, args.offset)
}

#[tauri::command]
pub async fn import_audio<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<<command::ImportAudio as TypedCommand>::Reply, String> {
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
pub async fn get_capture_status<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<<command::GetCaptureStatus as TypedCommand>::Reply, String> {
    trusted(&window)?;
    Ok(window.state::<LibraryService>().capture_status())
}

#[cfg(native_capture)]
#[tauri::command]
pub async fn start_recording<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<<command::StartRecording as TypedCommand>::Reply, String> {
    trusted(&window)?;
    window.state::<LibraryService>().start_recording()
}

#[cfg(native_capture)]
#[tauri::command]
pub async fn finish_recording<R: Runtime>(
    window: WebviewWindow<R>,
    message: ipc::Request<'_>,
) -> Result<<command::FinishRecording as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::FinishRecording>(message.body())?;
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        window
            .state::<LibraryService>()
            .finish_recording(&args.session_id, args.save)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn transcribe_recording<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    message: ipc::Request<'_>,
) -> Result<<command::TranscribeRecording as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::TranscribeRecording>(message.body())?;
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LibraryService>().transcribe(&args.recording_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn select_model<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
) -> Result<<command::SelectModel as TypedCommand>::Reply, String> {
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
    message: ipc::Request<'_>,
) -> Result<<command::GetPlayback as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::GetPlayback>(message.body())?;
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        window
            .state::<LibraryService>()
            .playback(&args.segment_id, args.revision)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_recording_playback<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    message: ipc::Request<'_>,
) -> Result<<command::GetRecordingPlayback as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::GetRecordingPlayback>(message.body())?;
    trusted(&window)?;
    state.recording_playback(&args.recording_id, args.revision, args.range)
}

#[tauri::command]
pub async fn get_error_marks<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    message: ipc::Request<'_>,
) -> Result<<command::GetErrorMarks as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::GetErrorMarks>(message.body())?;
    trusted(&window)?;
    state.error_marks(&args.recording_id, args.offset)
}

#[tauri::command]
pub async fn save_error_mark<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    message: ipc::Request<'_>,
) -> Result<<command::SaveErrorMark as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::SaveErrorMark>(message.body())?;
    trusted(&window)?;
    state.save_error_mark(&args.mark)
}

#[tauri::command]
pub async fn save_error_marks<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    message: ipc::Request<'_>,
) -> Result<<command::SaveErrorMarks as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::SaveErrorMarks>(message.body())?;
    trusted(&window)?;
    state.save_error_marks(&args.batch)
}

#[tauri::command]
pub async fn control_playback<R: Runtime>(
    window: WebviewWindow<R>,
    message: ipc::Request<'_>,
) -> Result<<command::ControlPlayback as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::ControlPlayback>(message.body())?;
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        window
            .state::<LibraryService>()
            .control_playback(&args.session, args.action)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_playback_status<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<<command::GetPlaybackStatus as TypedCommand>::Reply, String> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || window.state::<LibraryService>().playback_status())
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn play_recording<R: Runtime>(
    window: WebviewWindow<R>,
    message: ipc::Request<'_>,
) -> Result<<command::PlayRecording as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::PlayRecording>(message.body())?;
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        window.state::<LibraryService>().play_recording(
            &args.recording_id,
            args.revision,
            args.range,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn save_review<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    message: ipc::Request<'_>,
) -> Result<<command::SaveReview as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::SaveReview>(message.body())?;
    trusted(&window)?;
    state.review(&args.request, args.proof.as_deref())
}

fn ipc_args<C: TypedCommand>(body: &ipc::InvokeBody) -> Result<C::Args, String> {
    let ipc::InvokeBody::Json(body @ serde_json::Value::Object(_)) = body else {
        return Err(format!(
            "invalid args for command `{}`: expected a JSON object",
            C::NAME
        ));
    };
    C::Args::deserialize(body)
        .map_err(|error| format!("invalid args for command `{}`: {error}", C::NAME))
}

#[tauri::command]
pub async fn dismiss_info<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
) -> Result<<command::DismissInfo as TypedCommand>::Reply, String> {
    trusted(&window)?;
    state.dismiss_info()
}

#[tauri::command]
pub async fn export_text<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, LibraryService>,
    message: ipc::Request<'_>,
) -> Result<<command::ExportText as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::ExportText>(message.body())?;
    trusted(&window)?;
    state.export_text(&args.recording_id)
}

#[tauri::command]
pub async fn save_text_file<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    message: ipc::Request<'_>,
) -> Result<<command::SaveTextFile as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::SaveTextFile>(message.body())?;
    trusted(&window)?;
    let text = app
        .state::<LibraryService>()
        .export_text(&args.recording_id)?;
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
    message: ipc::Request<'_>,
) -> Result<<command::CopySavedText as TypedCommand>::Reply, String> {
    let args = ipc_args::<command::CopySavedText>(message.body())?;
    trusted(&window)?;
    let text = app
        .state::<LibraryService>()
        .export_text(&args.recording_id)?;
    app.clipboard()
        .write_text(text)
        .map_err(|e| format!("복사하지 못했습니다: {e}. 텍스트 파일 저장을 사용할 수 있습니다."))
}
