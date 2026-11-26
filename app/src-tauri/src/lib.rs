mod commands;

use seamone_runtime::LibraryService;
use tauri::Manager;

/// 앱 껍데기가 화면에 내주는 command 를 등록한다, 실제 앱과 시험이 같은 등록을 쓴다.
fn register_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    macro_rules! handlers {
        (shared { $($_variant:ident => $handler:ident,)* }
         desktop_capture { $($_capture_variant:ident => $capture_handler:ident,)* }) => {
            tauri::generate_handler![
                $(commands::$handler,)*
                $(#[cfg(native_capture)] commands::$capture_handler,)*
            ]
        };
    }
    builder.invoke_handler(seamone_core::with_ipc_commands!(handlers))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            app.manage(LibraryService::open(&root).map_err(std::io::Error::other)?);
            Ok(())
        });
    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_fs::init());
    register_commands(builder)
        .build(tauri::generate_context!())
        .expect("seamone 을 실행하지 못했다")
        .run(app_event);
}

#[cfg(native_capture)]
fn app_event<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: tauri::RunEvent) {
    use tauri::{RunEvent, WindowEvent};

    let Some(state) = app.try_state::<LibraryService>() else {
        return;
    };
    let recorder = &state;
    match event {
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } if label == "main" => {
            if !recorder.request_close() {
                api.prevent_close();
            }
        }
        RunEvent::ExitRequested {
            code: None, api, ..
        } => {
            if !recorder.request_close() {
                api.prevent_exit();
            }
        }
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::Destroyed,
            ..
        } if label == "main" => recorder.close(),
        RunEvent::ExitRequested { .. } | RunEvent::Exit => recorder.close(),
        _ => {}
    }
}

#[cfg(not(native_capture))]
fn app_event(_: &tauri::AppHandle, _: tauri::RunEvent) {}

#[cfg(test)]
mod tests {
    use super::register_commands;
    use seamone_core::{AppInfo, COMMAND_GET_APP_INFO};
    use tauri::http::HeaderMap;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
    use tauri::webview::InvokeRequest;
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    #[test]
    fn main_window_capabilities_grant_the_catalog_commands_on_their_platforms() {
        use serde_json::Value;
        use std::collections::BTreeSet;

        fn check(document: &str, id: &str, commands: &[&str], platforms: Option<&[&str]>) {
            let cap: Value = serde_json::from_str(document).unwrap();
            assert_eq!(cap["identifier"], id);
            assert_eq!(cap["windows"], serde_json::json!(["main"]));
            assert!(cap["webviews"].is_null() && cap["remote"].is_null());
            assert!(cap["local"].as_bool().unwrap_or(true));

            let permissions: BTreeSet<String> =
                serde_json::from_value(cap["permissions"].clone()).unwrap();
            let expected = commands
                .iter()
                .map(|name| format!("allow-{}", name.replace('_', "-")))
                .collect::<BTreeSet<_>>();
            assert_eq!(permissions, expected);

            let actual: Option<BTreeSet<String>> =
                serde_json::from_value(cap["platforms"].clone()).unwrap();
            let expected = platforms.map(|list| list.iter().map(ToString::to_string).collect());
            assert_eq!(actual, expected);
        }

        macro_rules! check_grants {
            (shared { $($_variant:ident => $shared:ident,)* }
             desktop_capture { $($_capture_variant:ident => $capture:ident,)* }) => {{
                check(include_str!("../capabilities/default.json"), "default", &[$(stringify!($shared)),*], None);
                check(
                    include_str!("../capabilities/capture.json"),
                    "capture",
                    &[$(stringify!($capture)),*],
                    Some(&["linux", "macOS", "windows"]),
                );
            }};
        }
        let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let selected: BTreeSet<String> =
            serde_json::from_value(config["app"]["security"]["capabilities"].clone()).unwrap();
        assert_eq!(
            selected,
            BTreeSet::from(["default".into(), "capture".into()])
        );
        seamone_core::with_ipc_commands!(check_grants);
    }

    #[test]
    fn registered_command_answers_over_ipc_with_the_core_app_info() {
        let app = register_commands(mock_builder())
            .build(mock_context(noop_assets()))
            .expect("앱을 만든다");
        let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .expect("창을 만든다");

        let response = get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: COMMAND_GET_APP_INFO.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                // 창이 열린 주소를 그대로 쓴다, 플랫폼마다 앱의 로컬 주소가 달라 고정한 주소는 원격으로 취급돼 거절된다
                url: webview.url().expect("창 주소를 읽는다"),
                body: InvokeBody::default(),
                headers: HeaderMap::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        );

        let info: AppInfo = response
            .expect("command 가 응답한다")
            .deserialize()
            .expect("응답이 AppInfo 로 읽힌다");
        assert_eq!(info, seamone_core::app_info());
    }

    fn invoke(
        window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        command: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        get_ipc_response(
            window,
            InvokeRequest {
                cmd: command.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: window.url().unwrap(),
                body: InvokeBody::Json(args),
                headers: HeaderMap::default(),
                invoke_key: INVOKE_KEY.into(),
            },
        )
        .map(|r| r.deserialize().unwrap())
    }

    #[test]
    fn capture_support_matches_host_registration() {
        assert_eq!(seamone_runtime::CAPTURE_SUPPORTED, cfg!(native_capture));
    }

    #[cfg(all(
        feature = "desktop-whisper",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    #[test]
    fn desktop_feature_reaches_the_runtime_engine() {
        assert!(seamone_runtime::LibraryService::inference_available());
    }

    fn app_fixture() -> (tempfile::TempDir, tauri::App<tauri::test::MockRuntime>) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/native-tests");
        std::fs::create_dir_all(&root).unwrap();
        let folder = tempfile::tempdir_in(root).unwrap();
        let app = register_commands(mock_builder())
            .manage(seamone_runtime::LibraryService::open(folder.path()).unwrap())
            .build(mock_context(noop_assets()))
            .unwrap();
        (folder, app)
    }

    #[test]
    fn registered_import_reopens_through_the_service_and_serves_bounded_playback() {
        use seamone_core::review::RecordingInfo;
        use serde_json::json;
        let (folder, app) = app_fixture();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/seamone-runtime/fixtures/stereo-48000.m4a");
        let picker = super::commands::TestAudioPicker(
            std::collections::VecDeque::from([None, Some(source.into())]).into(),
        );
        app.manage(picker);
        let main = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        assert_eq!(
            invoke(&main, "import_audio", json!({})).unwrap(),
            serde_json::Value::Null
        );
        let result = invoke(&main, "import_audio", json!({"path":"/unused.wav"})).unwrap();
        let saved: RecordingInfo = serde_json::from_value(result.clone()).unwrap();
        assert_eq!(saved.name, "stereo-48000.m4a");
        assert_eq!(
            invoke(&main, "get_library", json!({})).unwrap()["recordings"],
            json!([result])
        );
        let playback = invoke(
            &main,
            "get_recording_playback",
            json!({"recordingId":saved.id,"revision":saved.revision,"range":{"start":0,"end":16}}),
        )
        .unwrap();
        assert!(playback.as_str().is_some_and(|wav| !wav.is_empty()));
        assert!(invoke(&main, "get_recording_playback",
            json!({"recordingId":saved.id,"revision":saved.revision+1,"range":{"start":0,"end":16}})).is_err());
        assert!(
            invoke(
                &main,
                "get_segments",
                json!({"recordingId":saved.id,"offset":0})
            )
            .is_ok()
        );
        drop(main);
        drop(app);
        let service = seamone_runtime::LibraryService::open(folder.path()).unwrap();
        assert_eq!(service.library().unwrap().recordings, vec![saved]);
    }

    #[test]
    fn registered_commands_reject_other_windows_before_accessing_library_or_devices() {
        use serde_json::json;
        let (_folder, app) = app_fixture();
        let other = WebviewWindowBuilder::new(&app, "other", WebviewUrl::default())
            .build()
            .unwrap();
        let cases = [
            ("get_app_info", json!({})),
            ("get_library", json!({})),
            ("get_segments", json!({"recordingId":"missing","offset":0})),
            ("import_audio", json!({})),
            ("get_capture_status", json!({})),
            ("transcribe_recording", json!({"recordingId":"missing"})),
            ("select_model", json!({})),
            ("get_playback", json!({"segmentId":"missing","revision":1})),
            (
                "get_recording_playback",
                json!({"recordingId":"missing","revision":1,"range":{"start":0,"end":16}}),
            ),
            (
                "get_error_marks",
                json!({"recordingId":"missing","offset":0}),
            ),
            (
                "save_error_mark",
                json!({"mark":{"id":"mark","recording_id":"missing","recording_revision":1,"range":{"start":0,"end":16}}}),
            ),
            (
                "save_error_marks",
                json!({"batch":{"id":"batch","recording_id":"missing","recording_revision":1,"ranges":[{"start":0,"end":16}]}}),
            ),
            ("playback_ended", json!({"token":"missing"})),
            (
                "save_review",
                json!({"request":{"event_id":"event","segment_id":"missing","expected_revision":1,"action":{"kind":"judge","verdict":"no"}},"proof":null}),
            ),
            ("dismiss_info", json!({})),
            ("export_text", json!({"recordingId":"missing"})),
            ("copy_saved_text", json!({"recordingId":"missing"})),
            ("save_text_file", json!({"recordingId":"missing"})),
        ];
        for (command, args) in cases {
            let error = invoke(&other, command, args).unwrap_err();
            assert!(
                error
                    .as_str()
                    .is_some_and(|message| message.contains("이 창")),
                "{command}: {error}"
            );
        }
        #[cfg(native_capture)]
        for (command, args) in [
            ("start_recording", json!({})),
            (
                "finish_recording",
                json!({"sessionId":"missing","save":true}),
            ),
        ] {
            let error = invoke(&other, command, args).unwrap_err();
            assert!(
                error
                    .as_str()
                    .is_some_and(|message| message.contains("이 창")),
                "{command}: {error}"
            );
        }
    }

    #[cfg(native_capture)]
    #[test]
    fn idle_main_window_closes_through_the_service_event_adapter() {
        let (_folder, app) = app_fixture();
        let main = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        app.run(move |app, event| {
            let ready = matches!(event, tauri::RunEvent::Ready);
            super::app_event(app, event);
            if ready {
                main.close().unwrap();
            }
        });
    }
}
