use super::*;
use crate::store_tests::Fixture;
use seamone_core::review::{Action as ReviewAction, Verdict};

#[test]
fn platform_factory_starts_without_opening_a_device() {
    let playbacks = Playbacks::new();
    assert_eq!(playbacks.status().unwrap(), Status::default());
}

#[test]
fn review_needs_device_output_and_exact_scope_and_preview_never_authorizes() {
    let fixture = Fixture::new();
    let (store, segment) = fixture.transcribed();
    let playbacks = Playbacks::fake();
    for action in [
        ReviewAction::Judge {
            verdict: Verdict::Yes,
        },
        ReviewAction::Judge {
            verdict: Verdict::No,
        },
        ReviewAction::ConfirmCorrection,
        ReviewAction::CorrectAndConfirm {
            text: "교정".into(),
        },
    ] {
        let request = Request {
            event_id: "event".into(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision,
            action,
        };
        assert!(playbacks.authorize_review(&request, None).is_err());
        let reply = playbacks
            .prepare_review(&store, &segment.id, segment.revision)
            .unwrap();
        assert!(
            playbacks
                .authorize_review(&request, Some(&reply.session))
                .is_err()
        );
        playbacks.control(&reply.session, Action::Play).unwrap();
        assert!(
            playbacks
                .authorize_review(&request, Some(&reply.session))
                .is_err()
        );
        playbacks.advance(60_000).unwrap();
        assert_eq!(
            playbacks
                .authorize_review(&request, Some(&reply.session))
                .unwrap()
                .request(),
            &request
        );
        let stale = Request {
            expected_revision: request.expected_revision + 1,
            ..request.clone()
        };
        assert!(
            playbacks
                .authorize_review(&stale, Some(&reply.session))
                .is_err()
        );
        let other = Request {
            segment_id: "other".into(),
            ..request.clone()
        };
        assert!(
            playbacks
                .authorize_review(&other, Some(&reply.session))
                .is_err()
        );
        let preview = playbacks
            .play_recording(
                &store,
                &segment.recording_id,
                segment.recording_revision,
                segment.range,
            )
            .unwrap();
        playbacks.advance(60_000).unwrap();
        assert!(playbacks.status().unwrap().heard);
        assert!(
            playbacks
                .authorize_review(&request, preview.session.as_deref())
                .is_err()
        );
        assert!(
            playbacks
                .authorize_review(&request, Some(&reply.session))
                .is_err()
        );
    }
}

#[test]
fn preparation_requires_current_readable_audio_and_close_discards_authority() {
    let fixture = Fixture::new();
    let (store, segment) = fixture.transcribed();
    let playbacks = Playbacks::fake();
    assert!(
        playbacks
            .prepare_review(&store, "missing", segment.revision)
            .is_err()
    );
    assert_eq!(
        playbacks
            .prepare_review(&store, &segment.id, segment.revision + 1)
            .unwrap_err(),
        Error::StaleRevision.to_string()
    );
    let reply = playbacks
        .prepare_review(&store, &segment.id, segment.revision)
        .unwrap();
    assert_ne!(reply.wav_base64, "");
    playbacks.finish(&reply.session).unwrap();
    assert!(playbacks.status().unwrap().heard);
    playbacks.close();
    assert_eq!(playbacks.status().unwrap(), Status::default());
    assert!(playbacks.control(&reply.session, Action::Play).is_err());
    std::fs::write(
        store.audio.existing_path(&segment.recording_id).unwrap(),
        b"unreadable WAV",
    )
    .unwrap();
    assert!(
        playbacks
            .prepare_review(&store, &segment.id, segment.revision)
            .is_err()
    );
}

#[test]
fn proof_free_edits_cannot_gain_verified_write_authority() {
    let fixture = Fixture::new();
    let (_store, segment) = fixture.transcribed();
    let playbacks = Playbacks::fake();
    for action in [
        ReviewAction::SaveDraft {
            text: "교정".into(),
        },
        ReviewAction::Invalidate,
        ReviewAction::DismissErrorMark,
    ] {
        let request = Request {
            event_id: "event".into(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision,
            action,
        };
        assert_eq!(
            playbacks
                .authorize_review(&request, None)
                .unwrap()
                .request(),
            &request
        );
    }
    let draft = Request {
        event_id: "draft".into(),
        segment_id: segment.id.clone(),
        expected_revision: segment.revision,
        action: ReviewAction::SaveDraft {
            text: "교정".into(),
        },
    };
    let mut next = segment.apply(&draft, 16_000).unwrap();
    playbacks
        .authorize_review(&draft, None)
        .unwrap()
        .admit(&segment, &next)
        .unwrap();
    next.verified = Some(
        serde_json::from_value(
            serde_json::json!({"revision":next.revision,"text":next.text,"source":"corrected"}),
        )
        .unwrap(),
    );
    assert!(
        playbacks
            .authorize_review(&draft, None)
            .unwrap()
            .admit(&segment, &next)
            .is_err()
    );
}
