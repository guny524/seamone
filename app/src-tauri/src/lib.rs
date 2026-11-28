mod commands;

use seamone_runtime::LibraryService;
use tauri::Manager;

const _: () = assert!(seamone_runtime::CAPTURE_SUPPORTED == cfg!(native_capture));

/// 앱 껍데기가 화면에 내주는 command 를 등록한다, 실제 앱과 시험이 같은 등록을 쓴다.
fn register_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    macro_rules! handlers {
        (shared { $($_variant:ident => $handler:ident: $args:ty, $reply:ty;)* }
         desktop_capture { $($_capture_variant:ident => $capture_handler:ident: $capture_args:ty, $capture_reply:ty;)* }) => {
            tauri::generate_handler![
                $(commands::$handler,)*
                $(#[cfg(native_capture)] commands::$capture_handler,)*
            ]
        };
    }
    builder.invoke_handler(seamone_core::with_ipc_commands!(handlers))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// # Panics
/// Tauri가 앱을 만들지 못하면 초기화를 중단한다
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
            (shared { $($_variant:ident => $shared:ident: $args:ty, $reply:ty;)* }
             desktop_capture { $($_capture_variant:ident => $capture:ident: $capture_args:ty, $capture_reply:ty;)* }) => {{
                check(include_str!("../capabilities/default.json"), "default", &[$(stringify!($shared),)*], None);
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

    #[cfg(all(
        feature = "desktop-whisper",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    #[test]
    fn desktop_feature_reaches_the_runtime_engine() {
        assert!(seamone_runtime::LibraryService::inference_available());
    }

    fn typed_reply<C: seamone_core::ipc::TypedCommand>(
        window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        args: serde_json::Value,
    ) -> C::Reply {
        serde_json::from_value(invoke(window, C::NAME, args).unwrap()).unwrap()
    }

    fn complete_playback(window: &tauri::WebviewWindow<tauri::test::MockRuntime>, session: &str) {
        use seamone_core::ipc::command;
        let accepted = typed_reply::<command::ControlPlayback>(
            window,
            serde_json::json!({"session":session,"action":{"kind":"play"}}),
        );
        assert_eq!(accepted.session.as_deref(), Some(session));
        let state = window.state::<seamone_runtime::LibraryService>();
        seamone_runtime::test_support::advance_playback(&state, 60_000).unwrap();
        let status = typed_reply::<command::GetPlaybackStatus>(window, serde_json::json!({}));
        assert!(status.heard);
    }

    fn app_fixture() -> (tempfile::TempDir, tauri::App<tauri::test::MockRuntime>) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/native-tests");
        std::fs::create_dir_all(&root).unwrap();
        let folder = tempfile::tempdir_in(root).unwrap();
        let app = register_commands(mock_builder())
            .manage(seamone_runtime::test_support::open(folder.path()).unwrap())
            .build(mock_context(noop_assets()))
            .unwrap();
        (folder, app)
    }

    fn review_app_fixture() -> (
        tempfile::TempDir,
        tauri::App<tauri::test::MockRuntime>,
        seamone_core::review::Segment,
    ) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/native-tests");
        std::fs::create_dir_all(&root).unwrap();
        let folder = tempfile::tempdir_in(root).unwrap();
        let (service, segment) = seamone_runtime::test_support::transcribed(folder.path()).unwrap();
        let app = register_commands(mock_builder())
            .manage(service)
            .build(mock_context(noop_assets()))
            .unwrap();
        (folder, app, segment)
    }

    #[test]
    fn registered_commands_reject_malformed_arguments() {
        use seamone_core::{
            ipc::{TypedCommand, command},
            playback::State,
        };
        use serde_json::json;

        let (_fixture, app) = app_fixture();
        let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        for (name, malformed) in [
            (
                command::GetPlayback::NAME,
                json!({"segment_id":"segment","revision":1}),
            ),
            (
                command::GetRecordingPlayback::NAME,
                json!({"recordingId":"recording","revision":1}),
            ),
            (
                command::PlayRecording::NAME,
                json!(["recording",1,{"start":0,"end":16}]),
            ),
            (
                command::ControlPlayback::NAME,
                json!({"session":"session","action":"play"}),
            ),
            (
                command::GetSegments::NAME,
                json!({"recording_id":"recording","offset":0}),
            ),
            (command::TranscribeRecording::NAME, json!({"recordingId":1})),
            (
                command::GetErrorMarks::NAME,
                json!({"recordingId":"recording","offset":"0"}),
            ),
            (
                command::SaveErrorMark::NAME,
                json!({"mark":{"id":"mark","recording_id":"recording","recording_revision":1}}),
            ),
            (
                command::SaveErrorMarks::NAME,
                json!({"batch":{"id":"batch","recording_id":"recording","recording_revision":1,"ranges":{}}}),
            ),
            (
                command::ExportText::NAME,
                json!({"recording_id":"recording"}),
            ),
            (command::CopySavedText::NAME, json!(["recording"])),
            (command::SaveTextFile::NAME, json!({"recordingId":1})),
            #[cfg(native_capture)]
            (
                command::FinishRecording::NAME,
                json!({"sessionId":"session","save":"true"}),
            ),
        ] {
            let error = invoke(&window, name, malformed).unwrap_err();
            let prefix = format!("invalid args for command `{name}`:");
            assert!(
                error
                    .as_str()
                    .is_some_and(|message| message.starts_with(&prefix)),
                "{name}: {error}"
            );
        }
        for ignored in [json!({}), json!({"extra":1}), json!([])] {
            let status = typed_reply::<command::GetPlaybackStatus>(&window, ignored);
            assert_eq!(status.state, State::Idle);
        }
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
            (
                "control_playback",
                json!({"session":"missing","action":{"kind":"play"}}),
            ),
            ("get_playback_status", json!({})),
            (
                "play_recording",
                json!({"recordingId":"missing","revision":1,"range":{"start":0,"end":16}}),
            ),
            (
                "save_review",
                json!({"request":{"event_id":"event","segment_id":"missing","expected_revision":1,"action":{"kind":"judge","verdict":"no"}},"proof":null}),
            ),
            ("dismiss_info", json!({})),
            ("export_text", json!({"recordingId":"missing"})),
            ("copy_saved_text", json!({"recordingId":"missing"})),
            ("save_text_file", json!({"recordingId":"missing"})),
            #[cfg(native_capture)]
            ("start_recording", json!({})),
            #[cfg(native_capture)]
            (
                "finish_recording",
                json!({"sessionId":"missing","save":true}),
            ),
        ];
        #[cfg(native_capture)]
        {
            let mut tested = cases.each_ref().map(|(name, _)| *name);
            tested.sort_unstable();
            let mut registered = seamone_core::ipc::COMMAND_NAMES.to_vec();
            registered.sort_unstable();
            assert_eq!(tested.as_slice(), registered.as_slice());
        }
        for (command, args) in cases {
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
    fn main_window_close_waits_for_unsaved_audio_to_be_discarded() {
        use seamone_core::capture::Status;
        use std::{
            cell::Cell,
            rc::Rc,
            time::{Duration, Instant},
        };
        use tauri::{RunEvent, WindowEvent};

        let (folder, app) = app_fixture();
        let state = app.state::<seamone_runtime::LibraryService>();
        state.start_recording().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !matches!(state.capture_status(), Status::Recording { .. }) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let main = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        let closed = Rc::new(Cell::new(0));
        let seen = Rc::clone(&closed);
        app.run(move |app, event| {
            let ready = matches!(event, RunEvent::Ready);
            let close_request = matches!(&event, RunEvent::WindowEvent { label, event: WindowEvent::CloseRequested { .. }, .. } if label == "main");
            super::app_event(app, event);
            if ready {
                main.close().unwrap();
            }
            if close_request {
                seen.set(seen.get() + 1);
                if seen.get() == 1 {
                    let state = app.state::<seamone_runtime::LibraryService>();
                    let deadline = Instant::now() + Duration::from_secs(2);
                    loop {
                        if let Status::Interrupted { session_id, reason, .. } = state.capture_status() {
                            assert!(reason.contains("창을 닫기 전에"));
                            state.finish_recording(&session_id, false).unwrap();
                            main.close().unwrap();
                            break;
                        }
                        assert!(Instant::now() < deadline);
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            }
        });
        assert_eq!(
            closed.get(),
            2,
            "the first native close request must be prevented"
        );
        assert_eq!(
            seamone_runtime::LibraryService::open(folder.path())
                .unwrap()
                .library()
                .unwrap()
                .recordings,
            []
        );
    }

    #[test]
    fn registered_batch_saves_disjoint_ranges_through_trusted_ipc() {
        use seamone_core::{
            error_marks::{ErrorMarkBatch, ErrorMarkPage},
            review::SampleRange,
        };
        use serde_json::json;
        let (_fixture, app, segment) = review_app_fixture();
        let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "other", WebviewUrl::default())
            .build()
            .unwrap();
        let batch = ErrorMarkBatch {
            id: "batch".into(),
            recording_id: segment.recording_id.clone(),
            recording_revision: segment.recording_revision,
            ranges: vec![
                SampleRange {
                    start: 0,
                    end: 4_000,
                },
                SampleRange {
                    start: 8_000,
                    end: 10_000,
                },
            ],
        };
        let args = json!({ "batch": batch });
        assert!(invoke(&other, "save_error_marks", args.clone()).is_err());
        invoke(&window, "save_error_marks", args.clone()).unwrap();
        invoke(&window, "save_error_marks", args).unwrap();
        let page: ErrorMarkPage = serde_json::from_value(
            invoke(
                &window,
                "get_error_marks",
                json!({"recordingId":segment.recording_id, "offset":0}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            page.marks.iter().map(|mark| mark.range).collect::<Vec<_>>(),
            batch.ranges
        );
    }

    #[cfg(native_capture)]
    #[test]
    fn registered_capture_ipc_starts_cancels_and_saves_for_the_main_window() {
        use seamone_core::{capture::Status, review::RecordingInfo};
        use serde_json::json;
        use std::time::{Duration, Instant};

        let (_folder, app) = app_fixture();
        let main = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "other", WebviewUrl::default())
            .build()
            .unwrap();
        assert_eq!(
            invoke(&main, "get_capture_status", json!({})).unwrap(),
            json!({"state":"idle"})
        );
        for (command, payload) in [
            ("get_capture_status", json!({})),
            ("start_recording", json!({})),
            ("finish_recording", json!({"sessionId":"other","save":true})),
            ("import_audio", json!({"path":"/untrusted.wav"})),
        ] {
            assert!(
                invoke(&other, command, payload)
                    .unwrap_err()
                    .as_str()
                    .unwrap()
                    .contains("이 창")
            );
        }
        for save in [false, true] {
            let started: Status =
                serde_json::from_value(invoke(&main, "start_recording", json!({})).unwrap())
                    .unwrap();
            let (Status::Starting { session_id } | Status::Recording { session_id, .. }) = started
            else {
                panic!("unexpected capture state: {started:?}");
            };
            assert!(invoke(&main, "start_recording", json!({})).is_err());
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let state = invoke(&main, "get_capture_status", json!({})).unwrap();
                if state["state"] == "recording" {
                    assert_eq!(state["session_id"], session_id);
                    break;
                }
                assert!(Instant::now() < deadline, "{state}");
                std::thread::sleep(Duration::from_millis(5));
            }
            let args = json!({"sessionId":session_id,"save":save});
            let result = invoke(&main, "finish_recording", args.clone()).unwrap();
            assert!(invoke(&main, "finish_recording", args).is_err());
            let library = invoke(&main, "get_library", json!({})).unwrap();
            if save {
                let info: RecordingInfo = serde_json::from_value(result.clone()).unwrap();
                assert_eq!(info.samples, 16_000);
                assert_eq!(library["recordings"][0], result);
            } else {
                assert_eq!(result, serde_json::Value::Null);
                assert_eq!(library["recordings"], json!([]));
            }
        }
    }

    #[test]
    fn registered_correction_ipc_rejects_stale_revision_and_old_playback() {
        use seamone_core::review::{Action, PlaybackReply, Request, Segment};
        use serde_json::json;
        let (_fixture, app, segment) = review_app_fixture();
        let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        let playback: PlaybackReply = serde_json::from_value(
            invoke(
                &window,
                "get_playback",
                json!({"segmentId":segment.id,"revision":segment.revision}),
            )
            .unwrap(),
        )
        .unwrap();
        complete_playback(&window, &playback.session);
        let draft = Request {
            event_id: "event-0".into(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision,
            action: Action::SaveDraft {
                text: "수정 초안".into(),
            },
        };
        let saved: Segment = serde_json::from_value(
            invoke(
                &window,
                "save_review",
                json!({"request":draft,"proof":null}),
            )
            .unwrap(),
        )
        .unwrap();
        let stale = Request {
            event_id: "event-1".into(),
            ..draft
        };
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":stale,"proof":null})
            )
            .unwrap_err()
            .as_str()
            .unwrap()
            .contains("다른 수정본")
        );
        let confirm = Request {
            event_id: "event-2".into(),
            segment_id: segment.id.clone(),
            expected_revision: saved.revision,
            action: Action::ConfirmCorrection,
        };
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":confirm,"proof":playback.session})
            )
            .is_err()
        );
        let playback: PlaybackReply = serde_json::from_value(
            invoke(
                &window,
                "get_playback",
                json!({"segmentId":segment.id,"revision":saved.revision}),
            )
            .unwrap(),
        )
        .unwrap();
        complete_playback(&window, &playback.session);
        let confirmed: Segment = serde_json::from_value(
            invoke(
                &window,
                "save_review",
                json!({"request":confirm,"proof":playback.session}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(confirmed.verified.unwrap().text(), "수정 초안");
        assert_eq!(
            invoke(
                &window,
                "export_text",
                json!({"recordingId":segment.recording_id})
            )
            .unwrap(),
            json!("수정 초안")
        );
    }

    #[test]
    fn registered_error_mark_and_preview_require_separate_full_playback_for_correction() {
        use seamone_core::{
            error_marks::{ErrorMark, ErrorMarkPage},
            ipc::command,
            review::{Action, Request, SampleRange, Segment},
        };
        use serde_json::json;

        let (_fixture, app, segment) = review_app_fixture();
        let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "other", WebviewUrl::default())
            .build()
            .unwrap();
        let mark = ErrorMark {
            id: "mark".into(),
            recording_id: segment.recording_id.clone(),
            recording_revision: segment.recording_revision,
            range: SampleRange {
                start: segment.range.start,
                end: segment.range.start + 16,
            },
        };
        let preview_args = json!({
            "recordingId":mark.recording_id,"revision":mark.recording_revision,"range":mark.range
        });
        assert!(invoke(&other, "get_recording_playback", preview_args.clone()).is_err());
        let preview = typed_reply::<command::GetRecordingPlayback>(&window, preview_args);
        assert!(preview.starts_with("UklGR"));
        let removed = invoke(&window, "playback_ended", json!({"token":preview})).unwrap_err();
        assert_eq!(removed.as_str(), Some("Command playback_ended not found"));
        let old = typed_reply::<command::GetPlayback>(
            &window,
            json!({"segmentId":segment.id,"revision":segment.revision}),
        );
        complete_playback(&window, &old.session);
        assert!(invoke(&other, "save_error_mark", json!({"mark":mark})).is_err());
        invoke(&window, "save_error_mark", json!({"mark":mark})).unwrap();
        invoke(&window, "save_error_mark", json!({"mark":mark})).unwrap();
        let marks: ErrorMarkPage = serde_json::from_value(
            invoke(
                &window,
                "get_error_marks",
                json!({"recordingId":segment.recording_id,"offset":0}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(marks.marks, vec![mark]);
        let req = Request {
            event_id: "correct".into(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision + 1,
            action: Action::CorrectAndConfirm {
                text: "고친 말".into(),
            },
        };
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":req,"proof":old.session})
            )
            .is_err()
        );
        let full = typed_reply::<command::GetPlayback>(
            &window,
            json!({"segmentId":segment.id,"revision":req.expected_revision}),
        );
        let correction = json!({"request":req,"proof":full.session});
        assert!(invoke(&window, "save_review", correction.clone()).is_err());
        complete_playback(&window, &full.session);
        let unchanged = Request {
            event_id: "unchanged".into(),
            action: Action::ConfirmCorrection,
            ..req
        };
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":unchanged,"proof":full.session})
            )
            .is_err()
        );
        let corrected: Segment =
            serde_json::from_value(invoke(&window, "save_review", correction).unwrap()).unwrap();
        assert_eq!(corrected.range, segment.range);
        assert_eq!(corrected.verified.unwrap().text(), "고친 말");
    }

    #[test]
    fn preview_output_cannot_authorize_a_review_and_stale_controls_fail() {
        use seamone_core::{
            ipc::command,
            playback::State,
            review::{Action, Request, Verdict},
        };
        use serde_json::json;
        let (_fixture, app, segment) = review_app_fixture();
        let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        let args = json!({"recordingId":segment.recording_id,"revision":segment.recording_revision,"range":segment.range});
        let playing = typed_reply::<command::PlayRecording>(&window, args.clone());
        let session = playing.session.as_deref().unwrap();
        complete_playback(&window, session);
        let request = Request {
            event_id: "preview-is-not-review".into(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision,
            action: Action::Judge {
                verdict: Verdict::Yes,
            },
        };
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":request,"proof":session})
            )
            .is_err()
        );
        let next = typed_reply::<command::PlayRecording>(&window, args);
        assert_ne!(next.session.as_deref(), Some(session));
        assert!(
            invoke(
                &window,
                "control_playback",
                json!({"session":session,"action":{"kind":"play"}})
            )
            .is_err()
        );
        window.state::<seamone_runtime::LibraryService>().close();
        assert_eq!(
            typed_reply::<command::GetPlaybackStatus>(&window, json!({})).state,
            State::Idle
        );
    }

    #[test]
    fn registered_review_ipc_checks_playback_revision_replay_and_window_trust() {
        use seamone_core::review::{Action, PlaybackReply, Request, Segment, Verdict};
        use serde_json::json;
        let (_fixture, app, segment) = review_app_fixture();
        let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        let untrusted = WebviewWindowBuilder::new(&app, "other", WebviewUrl::default())
            .build()
            .unwrap();
        assert!(invoke(&untrusted, "get_library", json!({})).is_err());
        let req = Request {
            event_id: "event-0".into(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision,
            action: Action::Judge {
                verdict: Verdict::No,
            },
        };
        let no_proof =
            invoke(&window, "save_review", json!({"request":req,"proof":null})).unwrap_err();
        assert_eq!(
            invoke(&window, "save_review", json!({"request":req})).unwrap_err(),
            no_proof
        );
        for malformed in [
            json!([req, null]),
            json!({"proof":null}),
            json!({"request":req,"proof":7}),
        ] {
            let error = invoke(&window, "save_review", malformed).unwrap_err();
            assert!(
                error
                    .as_str()
                    .is_some_and(|message| message.starts_with("invalid args for command")),
                "{error}"
            );
        }
        let playback: PlaybackReply = serde_json::from_value(
            invoke(
                &window,
                "get_playback",
                json!({"segmentId":segment.id,"revision":segment.revision}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":req,"proof":playback.session})
            )
            .is_err()
        );
        invoke(
            &window,
            "control_playback",
            json!({"session":playback.session,"action":{"kind":"play"}}),
        )
        .unwrap();
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":req,"proof":playback.session})
            )
            .is_err()
        );
        complete_playback(&window, &playback.session);
        let rejected: Segment = serde_json::from_value(
            invoke(
                &window,
                "save_review",
                json!({"request":req,"proof":playback.session}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(rejected.verified.is_none());
        let duplicate =
            invoke(&window, "save_review", json!({"request":req,"proof":null})).unwrap();
        assert_eq!(
            serde_json::from_value::<Segment>(duplicate).unwrap(),
            rejected
        );
    }
}
