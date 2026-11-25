mod audio;
mod audio_ingest;
#[cfg(test)]
mod audio_input_tests;
#[cfg(native_capture)]
mod capture;
mod commands;
mod inference;
mod normalize;
pub mod store;
#[cfg(test)]
mod store_tests;
pub mod training;

use commands::AppState;
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
            let store = store::Store::open(&root).map_err(std::io::Error::other)?;
            app.manage(AppState::new(store));
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

    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let recorder = &state.recorder;
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

        fn check(document: &str, commands: &[&str], platforms: Option<&[&str]>) {
            let cap: Value = serde_json::from_str(document).unwrap();
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
                check(include_str!("../capabilities/default.json"), &[$(stringify!($shared)),*], None);
                check(
                    include_str!("../capabilities/capture.json"),
                    &[$(stringify!($capture)),*],
                    Some(&["linux", "macOS", "windows"]),
                );
            }};
        }
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

    fn review_app_fixture() -> (
        super::store_tests::Fixture,
        tauri::App<tauri::test::MockRuntime>,
        seamone_core::review::ReviewSegment,
    ) {
        let fixture = super::store_tests::Fixture::new();
        let (store, segment) = fixture.transcribed();
        let app = register_commands(mock_builder())
            .manage(super::commands::AppState::new(store))
            .build(mock_context(noop_assets()))
            .unwrap();
        (fixture, app, segment)
    }

    #[test]
    fn registered_import_uses_the_picker_and_opens_the_saved_audio_in_sqlite() {
        use seamone_core::review::RecordingInfo;
        use serde_json::json;
        use std::path::Path;

        let fixture = super::store_tests::Fixture::new();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/stereo-48000.m4a");
        let choices = std::collections::VecDeque::from([None, Some(source.into())]);
        let picker = super::commands::TestAudioPicker(choices.into());
        let app = register_commands(mock_builder())
            .manage(super::commands::AppState::new(fixture.store()))
            .manage(picker)
            .build(mock_context(noop_assets()))
            .unwrap();
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
        let (_, audio) = fixture.store().prepare_transcription(&saved.id).unwrap();
        assert_eq!(audio.len() as u64, saved.samples);
    }

    #[cfg(native_capture)]
    #[test]
    fn registered_capture_ipc_starts_cancels_and_saves_for_the_main_window() {
        use seamone_core::{capture::CaptureStatus, review::RecordingInfo};
        use serde_json::json;
        use std::time::{Duration, Instant};

        let fixture = super::store_tests::Fixture::new();
        let mut state = super::commands::AppState::new(fixture.store());
        state.recorder = super::capture::Recorder::fake();
        let app = register_commands(mock_builder())
            .manage(state)
            .build(mock_context(noop_assets()))
            .unwrap();
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
            let started: CaptureStatus =
                serde_json::from_value(invoke(&main, "start_recording", json!({})).unwrap())
                    .unwrap();
            let (CaptureStatus::Starting { session_id }
            | CaptureStatus::Recording { session_id, .. }) = started
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
                let (_, audio) = fixture.store().prepare_transcription(&info.id).unwrap();
                assert_eq!(audio.len(), 16_000);
                assert!((audio[1000] - 0.25).abs() < f32::EPSILON);
            } else {
                assert_eq!(result, serde_json::Value::Null);
                assert_eq!(library["recordings"], json!([]));
            }
        }
    }

    #[cfg(native_capture)]
    #[test]
    fn main_window_close_waits_for_unsaved_audio_to_be_discarded() {
        use seamone_core::capture::CaptureStatus;
        use std::{
            cell::Cell,
            rc::Rc,
            time::{Duration, Instant},
        };
        use tauri::{RunEvent, WindowEvent};

        let fixture = super::store_tests::Fixture::new();
        let mut state = super::commands::AppState::new(fixture.store());
        state.recorder = super::capture::Recorder::fake();
        state
            .recorder
            .start(
                state.store().unwrap().audio.clone(),
                state.background.foreground.enter(),
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !matches!(state.recorder.status(), CaptureStatus::Recording { .. }) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let app = register_commands(mock_builder())
            .manage(state)
            .build(mock_context(noop_assets()))
            .unwrap();
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
                    let state = app.state::<super::commands::AppState>();
                    let deadline = Instant::now() + Duration::from_secs(2);
                    loop {
                        if let CaptureStatus::Interrupted { session_id, reason, .. } = state.recorder.status() {
                            assert!(reason.contains("창을 닫기 전에"));
                            state.recorder.finish(&session_id, false, |_, _| unreachable!()).unwrap();
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
        assert_eq!(fixture.store().library(false).unwrap().recordings, []);
    }

    #[test]
    fn registered_error_mark_and_preview_require_separate_full_playback_for_correction() {
        use seamone_core::{
            error_marks::{ErrorMark, ErrorMarkPage},
            review::{PlaybackReply, ReviewAction, ReviewRequest, ReviewSegment, SampleRange},
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
        let preview = invoke(&window, "get_recording_playback", preview_args).unwrap();
        assert!(preview.as_str().is_some_and(|wav| !wav.is_empty()));
        assert!(invoke(&window, "playback_ended", json!({"token":preview})).is_err());
        let old: PlaybackReply = serde_json::from_value(
            invoke(
                &window,
                "get_playback",
                json!({"segmentId":segment.id,"revision":segment.revision}),
            )
            .unwrap(),
        )
        .unwrap();
        invoke(&window, "playback_ended", json!({"token":old.token})).unwrap();
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
        let req = ReviewRequest {
            event_id: "correct".into(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision + 1,
            action: ReviewAction::CorrectAndConfirm {
                text: "고친 말".into(),
            },
        };
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":req,"proof":old.token})
            )
            .is_err()
        );
        let full: PlaybackReply = serde_json::from_value(
            invoke(
                &window,
                "get_playback",
                json!({"segmentId":segment.id,"revision":req.expected_revision}),
            )
            .unwrap(),
        )
        .unwrap();
        let correction = json!({"request":req,"proof":full.token});
        assert!(invoke(&window, "save_review", correction.clone()).is_err());
        invoke(&window, "playback_ended", json!({"token":full.token})).unwrap();
        let unchanged = ReviewRequest {
            event_id: "unchanged".into(),
            action: ReviewAction::ConfirmCorrection,
            ..req
        };
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":unchanged,"proof":full.token})
            )
            .is_err()
        );
        let corrected: ReviewSegment =
            serde_json::from_value(invoke(&window, "save_review", correction).unwrap()).unwrap();
        assert_eq!(corrected.range, segment.range);
        assert_eq!(corrected.verified.unwrap().text, "고친 말");
    }

    #[test]
    fn registered_review_ipc_checks_playback_revision_replay_and_window_trust() {
        use seamone_core::review::{
            PlaybackReply, ReviewAction, ReviewRequest, ReviewSegment, Verdict,
        };
        use serde_json::json;
        use uuid::Uuid;
        let (_fixture, app, segment) = review_app_fixture();
        let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        let untrusted = WebviewWindowBuilder::new(&app, "other", WebviewUrl::default())
            .build()
            .unwrap();
        assert!(invoke(&untrusted, "get_library", json!({})).is_err());
        let req = ReviewRequest {
            event_id: Uuid::new_v4().to_string(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision,
            action: ReviewAction::Judge {
                verdict: Verdict::No,
            },
        };
        assert!(invoke(&window, "save_review", json!({"request":req,"proof":null})).is_err());
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
                json!({"request":req,"proof":playback.token})
            )
            .is_err()
        );
        invoke(&window, "playback_ended", json!({"token":playback.token})).unwrap();
        let rejected: ReviewSegment = serde_json::from_value(
            invoke(
                &window,
                "save_review",
                json!({"request":req,"proof":playback.token}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(rejected.verified.is_none());
        let duplicate =
            invoke(&window, "save_review", json!({"request":req,"proof":null})).unwrap();
        assert_eq!(
            serde_json::from_value::<ReviewSegment>(duplicate).unwrap(),
            rejected
        );
    }

    #[test]
    fn registered_correction_ipc_rejects_stale_revision_and_old_playback() {
        use super::commands::AppState;
        use seamone_core::review::{PlaybackReply, ReviewAction, ReviewRequest, ReviewSegment};
        use serde_json::json;
        use uuid::Uuid;
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
        invoke(&window, "playback_ended", json!({"token":playback.token})).unwrap();
        let rejected = segment.clone();
        let draft = ReviewRequest {
            event_id: Uuid::new_v4().to_string(),
            segment_id: segment.id.clone(),
            expected_revision: rejected.revision,
            action: ReviewAction::SaveDraft {
                text: "수정 초안".into(),
            },
        };
        let saved: ReviewSegment = serde_json::from_value(
            invoke(
                &window,
                "save_review",
                json!({"request":draft,"proof":null}),
            )
            .unwrap(),
        )
        .unwrap();
        let stale = ReviewRequest {
            event_id: Uuid::new_v4().to_string(),
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
        let confirm = ReviewRequest {
            event_id: Uuid::new_v4().to_string(),
            segment_id: segment.id.clone(),
            expected_revision: saved.revision,
            action: ReviewAction::ConfirmCorrection,
        };
        assert!(
            invoke(
                &window,
                "save_review",
                json!({"request":confirm,"proof":playback.token})
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
        invoke(&window, "playback_ended", json!({"token":playback.token})).unwrap();
        let confirmed: ReviewSegment = serde_json::from_value(
            invoke(
                &window,
                "save_review",
                json!({"request":confirm,"proof":playback.token}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            app.state::<AppState>()
                .store()
                .unwrap()
                .eligible(&confirmed.id, confirmed.revision)
                .is_ok()
        );
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
}
