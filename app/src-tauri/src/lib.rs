mod audio;
mod commands;
mod inference;
pub mod store;
#[cfg(test)]
mod store_tests;
pub mod training;

use commands::AppState;
use tauri::Manager;

/// 앱 껍데기가 화면에 내주는 command 를 등록한다, 실제 앱과 시험이 같은 등록을 쓴다.
fn register_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        commands::get_app_info,
        commands::get_library,
        commands::get_segments,
        commands::import_wav,
        commands::transcribe_recording,
        commands::select_model,
        commands::get_playback,
        commands::playback_ended,
        commands::save_review,
        commands::dismiss_info,
        commands::export_text,
        commands::copy_saved_text,
        commands::save_text_file
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    register_commands(
        tauri::Builder::default()
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_clipboard_manager::init())
            .setup(|app| {
                let root = app.path().app_data_dir()?;
                let store = store::Store::open(&root).map_err(std::io::Error::other)?;
                app.manage(AppState::new(store));
                Ok(())
            }),
    )
    .run(tauri::generate_context!())
    .expect("seamone 을 실행하지 못했다");
}

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
}
