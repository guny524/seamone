use crate::{LibraryService, store_tests::Fixture};
use seamone_core::{
    error_marks::{ErrorMark, ErrorMarkBatch},
    review::{ReviewAction, ReviewRequest, SampleRange, Verdict},
};

fn request(segment: &seamone_core::review::ReviewSegment, action: ReviewAction) -> ReviewRequest {
    ReviewRequest {
        event_id: uuid::Uuid::new_v4().to_string(),
        segment_id: segment.id.clone(),
        expected_revision: segment.revision,
        action,
    }
}

#[test]
fn review_requires_completed_current_playback_and_replays_once() {
    let fixture = Fixture::new();
    let (store, segment) = fixture.transcribed();
    let service = LibraryService::new(store);
    let request = request(
        &segment,
        ReviewAction::Judge {
            verdict: Verdict::No,
        },
    );
    assert!(service.review(&request, None).is_err());
    let playback = service.playback(&segment.id, segment.revision).unwrap();
    assert!(service.review(&request, Some(&playback.token)).is_err());
    service.playback_ended(&playback.token).unwrap();
    let rejected = service.review(&request, Some(&playback.token)).unwrap();
    assert!(rejected.verified.is_none());
    assert_eq!(service.review(&request, None).unwrap(), rejected);
}

#[test]
fn preview_and_error_mark_require_new_full_playback_for_correction() {
    let fixture = Fixture::new();
    let (store, segment) = fixture.transcribed();
    let service = LibraryService::new(store);
    let mark = ErrorMark {
        id: "mark".into(),
        recording_id: segment.recording_id.clone(),
        recording_revision: segment.recording_revision,
        range: SampleRange {
            start: segment.range.start,
            end: segment.range.start + 16,
        },
    };
    let preview = service
        .recording_playback(&mark.recording_id, mark.recording_revision, mark.range)
        .unwrap();
    assert_ne!(preview, "");
    assert!(service.playback_ended(&preview).is_err());
    let old = service.playback(&segment.id, segment.revision).unwrap();
    service.playback_ended(&old.token).unwrap();
    service.save_error_mark(&mark).unwrap();
    service.save_error_mark(&mark).unwrap();
    assert_eq!(
        service.error_marks(&segment.recording_id, 0).unwrap().marks,
        vec![mark]
    );
    let mut request = request(
        &segment,
        ReviewAction::CorrectAndConfirm {
            text: "고친 말".into(),
        },
    );
    request.expected_revision += 1;
    assert!(service.review(&request, Some(&old.token)).is_err());
    let full = service
        .playback(&segment.id, request.expected_revision)
        .unwrap();
    assert!(service.review(&request, Some(&full.token)).is_err());
    service.playback_ended(&full.token).unwrap();
    let unchanged = ReviewRequest {
        action: ReviewAction::ConfirmCorrection,
        event_id: "unchanged".into(),
        ..request.clone()
    };
    assert!(service.review(&unchanged, Some(&full.token)).is_err());
    let corrected = service.review(&request, Some(&full.token)).unwrap();
    assert_eq!(corrected.range, segment.range);
    assert_eq!(corrected.verified.unwrap().text, "고친 말");
}

#[test]
fn correction_rejects_stale_revisions_and_preserves_verified_export() {
    let fixture = Fixture::new();
    let (store, segment) = fixture.transcribed();
    let service = LibraryService::new(store);
    let playback = service.playback(&segment.id, segment.revision).unwrap();
    service.playback_ended(&playback.token).unwrap();
    let draft = request(
        &segment,
        ReviewAction::SaveDraft {
            text: "수정 초안".into(),
        },
    );
    let saved = service.review(&draft, None).unwrap();
    let stale = ReviewRequest {
        event_id: "stale".into(),
        ..draft
    };
    assert!(
        service
            .review(&stale, None)
            .unwrap_err()
            .contains("다른 수정본")
    );
    let confirm = request(&saved, ReviewAction::ConfirmCorrection);
    assert!(service.review(&confirm, Some(&playback.token)).is_err());
    let playback = service.playback(&saved.id, saved.revision).unwrap();
    service.playback_ended(&playback.token).unwrap();
    let confirmed = service.review(&confirm, Some(&playback.token)).unwrap();
    assert!(
        service
            .store()
            .unwrap()
            .eligible(&confirmed.id, confirmed.revision)
            .is_ok()
    );
    assert_eq!(
        service.export_text(&segment.recording_id).unwrap(),
        "수정 초안"
    );
    drop(service);
    let reopened = LibraryService::open(&fixture.root.join("app")).unwrap();
    assert_eq!(
        reopened
            .segments(&segment.recording_id, 0)
            .unwrap()
            .segments[0],
        confirmed
    );
}

#[test]
fn batch_saves_disjoint_ranges_idempotently() {
    let fixture = Fixture::new();
    let (store, segment) = fixture.transcribed();
    let service = LibraryService::new(store);
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
    service.save_error_marks(&batch).unwrap();
    service.save_error_marks(&batch).unwrap();
    assert_eq!(
        service
            .error_marks(&segment.recording_id, 0)
            .unwrap()
            .marks
            .iter()
            .map(|mark| mark.range)
            .collect::<Vec<_>>(),
        batch.ranges
    );
}

#[test]
fn document_stream_import_reopens_and_rechecks_permission_before_commit() {
    // A document-provider pipe permits Read without Seek.
    struct Stream<'a>(&'a [u8]);
    impl std::io::Read for Stream<'_> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            self.0.read(output)
        }
    }
    let fixture = Fixture::new();
    let service = LibraryService::open(&fixture.root.join("app")).unwrap();
    let bytes = std::fs::read(&fixture.source).unwrap();
    assert!(
        service
            .import_stream("denied.wav".into(), Stream(&bytes), || false)
            .is_err()
    );
    assert_eq!(service.library().unwrap().recordings, []);
    let recording = service
        .import_stream("stream.wav".into(), Stream(&bytes), || true)
        .unwrap();
    assert_eq!(recording.samples, 16_000);
    drop(service);
    let reopened = LibraryService::open(&fixture.root.join("app")).unwrap();
    assert_eq!(
        reopened.library().unwrap().recordings,
        vec![recording.clone()]
    );
    assert_ne!(
        reopened
            .recording_playback(
                &recording.id,
                recording.revision,
                SampleRange { start: 0, end: 16 }
            )
            .unwrap(),
        ""
    );
}

#[test]
fn opened_file_import_uses_the_runtime_display_name_and_permission_check() {
    let fixture = Fixture::new();
    let service = LibraryService::open(&fixture.root.join("app")).unwrap();
    let open = || std::fs::File::open(&fixture.source).unwrap();
    assert!(
        service
            .import_file("/private/denied.wav", open(), || false)
            .is_err()
    );
    assert_eq!(service.library().unwrap().recordings, []);
    let long_name = format!("{}.wav", "가".repeat(201));
    for (path, expected) in [
        (std::path::PathBuf::from("/"), "가져온 음성".to_string()),
        (
            std::path::PathBuf::from("/private").join(&long_name),
            long_name.chars().take(200).collect(),
        ),
    ] {
        let saved = service.import_file(path, open(), || true).unwrap();
        assert_eq!(saved.name, expected);
        assert_eq!(saved.samples, 16_000);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn capture_saves_cancels_and_defers_close_until_unsaved_audio_is_discarded() {
    use seamone_core::capture::CaptureStatus;
    use std::time::{Duration, Instant};
    let fixture = Fixture::new();
    let mut service = LibraryService::open(&fixture.root.join("app")).unwrap();
    service.recorder = crate::capture::Recorder::fake();
    for save in [false, true] {
        let started = service.start_recording().unwrap();
        let (CaptureStatus::Starting { session_id } | CaptureStatus::Recording { session_id, .. }) =
            started
        else {
            panic!("{started:?}")
        };
        assert!(service.start_recording().is_err());
        let deadline = Instant::now() + Duration::from_secs(2);
        while !matches!(service.capture_status(), CaptureStatus::Recording { .. }) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let saved = service.finish_recording(&session_id, save).unwrap();
        assert!(service.finish_recording(&session_id, save).is_err());
        if save {
            let info = saved.unwrap();
            assert_eq!(info.samples, 16_000);
            let (_, audio) = service
                .store()
                .unwrap()
                .prepare_transcription(&info.id)
                .unwrap();
            assert!((audio[1000] - 0.25).abs() < f32::EPSILON);
        } else {
            assert!(saved.is_none());
        }
    }
    service.start_recording().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !matches!(service.capture_status(), CaptureStatus::Recording { .. }) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!service.request_close());
    let deadline = Instant::now() + Duration::from_secs(2);
    let session_id = loop {
        if let CaptureStatus::Interrupted {
            session_id, reason, ..
        } = service.capture_status()
        {
            assert!(reason.contains("창을 닫기 전에"));
            break session_id;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    service.finish_recording(&session_id, false).unwrap();
    assert!(service.request_close());
    service.close();
    assert_eq!(service.library().unwrap().recordings.len(), 1);
}
