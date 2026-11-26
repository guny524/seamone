use crate::{audio::canonical_spec, store::Store};
use seamone_asr::FakeTranscriber;
use seamone_core::error_marks::ErrorMark;
use seamone_core::review::{
    LabelSource, ReviewAction, ReviewRequest, ReviewSegment, SampleRange, Verdict,
};
use std::{fs, io::Cursor, path::PathBuf};
use uuid::Uuid;

pub(crate) struct Fixture {
    pub(crate) root: PathBuf,
    pub(crate) source: PathBuf,
}
impl Fixture {
    pub(crate) fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp/native-tests")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        let source = root.join("input.wav");
        let mut wav = hound::WavWriter::create(&source, canonical_spec()).unwrap();
        for n in 0_i16..16_000 {
            wav.write_sample(n).unwrap();
        }
        wav.finalize().unwrap();
        Self { root, source }
    }
    pub(crate) fn store(&self) -> Store {
        Store::open(&self.root.join("app")).unwrap()
    }
    pub(crate) fn transcribed(&self) -> (Store, ReviewSegment) {
        let mut store = self.store();
        let info = store.import(&self.source).unwrap();
        let mut engine = FakeTranscriber::new().with_texts(vec!["네 안녕하세요".into()]);
        let segments = store
            .transcribe(&info.id, &mut engine, "test", "fixture-v1")
            .unwrap();
        (store, segments[0].clone())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn request(segment: &ReviewSegment, action: ReviewAction) -> ReviewRequest {
    ReviewRequest {
        event_id: Uuid::new_v4().to_string(),
        segment_id: segment.id.clone(),
        expected_revision: segment.revision,
        action,
    }
}

fn authorize(request: &ReviewRequest) -> crate::playback_evidence::AuthorizedReview<'_> {
    let playbacks = crate::playback_evidence::Playbacks::default();
    let token = playbacks
        .begin(&request.segment_id, request.expected_revision)
        .unwrap();
    playbacks.ended(&token).unwrap();
    playbacks.authorize(request, Some(&token)).unwrap()
}

#[test]
fn saved_error_marks_reopen_and_flag_only_overlapping_rows_without_relabeling() {
    use seamone_core::TranscriptSegment;

    let f = Fixture::new();
    let mut store = f.store();
    let info = store.import(&f.source).unwrap();
    let lines = [(0, 300, "가"), (300, 600, "나"), (700, 1000, "다")];
    let original = store
        .commit_transcription(
            &info,
            lines
                .into_iter()
                .map(|(start_ms, end_ms, text)| TranscriptSegment {
                    start_ms,
                    end_ms,
                    text: text.into(),
                    speaker: None,
                    timing: None,
                })
                .collect(),
            "test",
            "fixture-v1",
        )
        .unwrap();
    let checked = store
        .mutate(authorize(&request(
            &original[0],
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    let draft = store
        .mutate(authorize(&request(
            &original[1],
            ReviewAction::SaveDraft {
                text: "수정 초안".into(),
            },
        )))
        .unwrap();

    let mark = ErrorMark {
        id: Uuid::new_v4().to_string(),
        recording_id: info.id.clone(),
        recording_revision: info.revision,
        range: SampleRange {
            start: 4500,
            end: 6500,
        },
    };
    let mut second_connection = f.store();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            barrier.wait();
            store.save_error_mark(&mark).unwrap();
        });
        barrier.wait();
        second_connection.save_error_mark(&mark).unwrap();
        first.join().unwrap();
    });

    let mut gap = mark.clone();
    gap.id = Uuid::new_v4().to_string();
    gap.range = SampleRange {
        start: 9600,
        end: 11_200,
    };
    assert!(
        second_connection
            .save_error_mark(&ErrorMark {
                id: mark.id.clone(),
                ..gap.clone()
            })
            .is_err()
    );
    second_connection.save_error_mark(&gap).unwrap();

    let store = f.store();
    assert_eq!(
        store.error_mark_page(&info.id, 0).unwrap().marks,
        vec![mark, gap]
    );
    let affected = store.segments(&info.id).unwrap();
    for (row, old) in affected.iter().take(2).zip([checked, draft]) {
        assert_eq!(row.range, old.range);
        assert_eq!(row.machine_text, old.machine_text);
        assert_eq!(row.text, old.text);
        assert_eq!(row.verdict, old.verdict);
        assert_eq!(row.revision, old.revision + 1);
        assert!(row.needs_review());
        assert_eq!(row.needs_correction, old.needs_correction);
        assert_eq!(row.error_marked_text.as_deref(), Some(old.text.as_str()));
        assert!(row.verified.is_none());
        assert!(store.eligible(&row.id, row.revision).is_err());
    }
    assert_eq!(affected[2], original[2]);
    assert_eq!(store.library(false).unwrap().pending_training, 0);
}

#[test]
fn batch_rolls_back_all_ranges_and_same_id_retries_once() {
    use seamone_core::error_marks::ErrorMarkBatch;

    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let checked = store
        .mutate(authorize(&request(
            &segment,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    let zero_and_one = SampleRange {
        start: 0,
        end: 4_000,
    };
    let four = SampleRange {
        start: 8_000,
        end: 10_000,
    };
    let batch = ErrorMarkBatch {
        id: "two branches".into(),
        recording_id: checked.recording_id.clone(),
        recording_revision: checked.recording_revision,
        ranges: vec![four, zero_and_one, four],
    };
    let db = rusqlite::Connection::open(f.root.join("app/seamone.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_second BEFORE INSERT ON error_marks WHEN NEW.start_sample=8000 BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
    assert!(store.save_error_marks(&batch).is_err());
    assert_eq!(store.segment(&checked.id).unwrap(), checked);
    assert_eq!(store.library(false).unwrap().pending_training, 1);
    assert_eq!(
        store.error_mark_page(&batch.recording_id, 0).unwrap().marks,
        []
    );
    db.execute_batch("DROP TRIGGER fail_second;").unwrap();

    let invalid = ErrorMarkBatch {
        ranges: vec![
            zero_and_one,
            SampleRange {
                start: 15_999,
                end: 16_001,
            },
        ],
        ..batch.clone()
    };
    assert!(store.save_error_marks(&invalid).is_err());
    assert!(
        store
            .save_error_marks(&ErrorMarkBatch {
                recording_revision: batch.recording_revision + 1,
                ..batch.clone()
            })
            .is_err()
    );
    assert_eq!(store.segment(&checked.id).unwrap(), checked);

    store.save_error_marks(&batch).unwrap();
    drop(store);
    let mut store = f.store();
    store
        .save_error_marks(&ErrorMarkBatch {
            ranges: vec![zero_and_one, four],
            ..batch.clone()
        })
        .unwrap();
    assert!(
        store
            .save_error_marks(&ErrorMarkBatch {
                ranges: vec![zero_and_one],
                ..batch.clone()
            })
            .is_err()
    );
    let saved = store.error_mark_page(&batch.recording_id, 0).unwrap().marks;
    assert_eq!(
        saved.iter().map(|mark| mark.range).collect::<Vec<_>>(),
        [zero_and_one, four]
    );
    let marked = store.segment(&checked.id).unwrap();
    assert_eq!(marked.revision, checked.revision + 1);
    assert_eq!(
        marked.error_marked_text.as_deref(),
        Some(checked.text.as_str())
    );
    assert_eq!(store.library(false).unwrap().pending_training, 0);
    assert!(store.eligible(&marked.id, marked.revision).is_err());
}

#[test]
fn remarking_dismissed_audio_refreshes_review_without_duplicate_sample_rows() {
    use seamone_core::error_marks::ErrorMarkBatch;

    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let zero_and_one = SampleRange {
        start: 0,
        end: 4_000,
    };
    let four = SampleRange {
        start: 8_000,
        end: 10_000,
    };
    let batch = ErrorMarkBatch {
        id: "first review".into(),
        recording_id: segment.recording_id.clone(),
        recording_revision: segment.recording_revision,
        ranges: vec![zero_and_one, four],
    };
    store.save_error_marks(&batch).unwrap();
    let marked = store.segment(&segment.id).unwrap();
    let dismissed = store
        .mutate(authorize(&request(&marked, ReviewAction::DismissErrorMark)))
        .unwrap();
    store.save_error_marks(&batch).unwrap();
    assert_eq!(store.segment(&segment.id).unwrap(), dismissed);

    store
        .save_error_marks(&ErrorMarkBatch {
            id: "later review".into(),
            ..batch.clone()
        })
        .unwrap();
    let marked_again = store.segment(&segment.id).unwrap();
    assert_eq!(marked_again.revision, dismissed.revision + 1);
    assert_eq!(
        marked_again.error_marked_text.as_deref(),
        Some(dismissed.text.as_str())
    );
    assert_eq!(
        store
            .error_mark_page(&batch.recording_id, 0)
            .unwrap()
            .marks
            .iter()
            .map(|mark| mark.range)
            .collect::<Vec<_>>(),
        [zero_and_one, four]
    );
    store
        .save_error_marks(&ErrorMarkBatch {
            id: "partly new audio".into(),
            ranges: vec![
                SampleRange {
                    start: 3_000,
                    end: 9_000,
                },
                SampleRange {
                    start: 3_500,
                    end: 5_000,
                },
            ],
            ..batch.clone()
        })
        .unwrap();
    let marks = store.error_mark_page(&batch.recording_id, 0).unwrap().marks;
    assert_eq!(
        marks.iter().map(|mark| mark.range).collect::<Vec<_>>(),
        [
            zero_and_one,
            four,
            SampleRange {
                start: 4_000,
                end: 8_000
            }
        ]
    );
}

#[test]
fn reopening_version_three_keeps_marks_and_transcript_rows_without_token_timing() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let legacy_mark = ErrorMark {
        id: "legacy mark".into(),
        recording_id: segment.recording_id.clone(),
        recording_revision: segment.recording_revision,
        range: SampleRange { start: 16, end: 32 },
    };
    store.save_error_mark(&legacy_mark).unwrap();
    let marked = store.segment(&segment.id).unwrap();
    drop(store);
    let old = rusqlite::Connection::open(f.root.join("app/seamone.sqlite3")).unwrap();
    old.execute_batch("DROP TABLE error_mark_batches; PRAGMA user_version = 3;")
        .unwrap();
    drop(old);

    let store = f.store();
    let reopened = store.segment(&marked.id).unwrap();
    assert_eq!(reopened, marked);
    assert_eq!(reopened.display_timing(), None);
    assert_eq!(
        store
            .error_mark_page(&segment.recording_id, 0)
            .unwrap()
            .marks,
        [legacy_mark]
    );
}

#[test]
fn mark_migration_preserves_verified_data_and_rejects_stale_or_invalid_inputs() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let checked = store
        .mutate(authorize(&request(
            &segment,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    let pending = store.import(&f.source).unwrap();
    drop(store);
    let old = rusqlite::Connection::open(f.root.join("app/seamone.sqlite3")).unwrap();
    old.execute_batch(
        "DROP TABLE error_mark_batches; DROP TABLE error_marks; PRAGMA user_version = 2;",
    )
    .unwrap();
    drop(old);
    let mut store = f.store();
    assert_eq!(store.segment(&checked.id).unwrap(), checked);
    assert!(store.eligible(&checked.id, checked.revision).is_ok());

    let mark = ErrorMark {
        id: "invalid".into(),
        recording_id: segment.recording_id.clone(),
        recording_revision: segment.recording_revision,
        range: segment.range,
    };
    for invalid in [
        ErrorMark {
            recording_id: pending.id,
            ..mark.clone()
        },
        ErrorMark {
            recording_revision: segment.recording_revision + 1,
            ..mark.clone()
        },
        ErrorMark {
            range: SampleRange {
                start: 0,
                end: pending.samples + 1,
            },
            ..mark
        },
    ] {
        assert!(store.save_error_mark(&invalid).is_err());
    }
    assert_eq!(
        store
            .error_mark_page(&segment.recording_id, 0)
            .unwrap()
            .marks,
        []
    );
    assert_eq!(store.segment(&checked.id).unwrap(), checked);
}

#[test]
fn error_mark_blocks_unchanged_latest_text_after_draft_save_and_reopen() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let edited = store
        .mutate(authorize(&request(
            &segment,
            ReviewAction::SaveDraft {
                text: "표시 당시수정한 글".into(),
            },
        )))
        .unwrap();
    let mark = ErrorMark {
        id: Uuid::new_v4().to_string(),
        recording_id: edited.recording_id.clone(),
        recording_revision: edited.recording_revision,
        range: SampleRange {
            start: edited.range.start,
            end: edited.range.start + 16,
        },
    };
    store.save_error_mark(&mark).unwrap();
    let marked = store.segment(&edited.id).unwrap();
    for action in [
        ReviewAction::ConfirmCorrection,
        ReviewAction::CorrectAndConfirm {
            text: "  표시 당시수정한 글  ".into(),
        },
    ] {
        assert_eq!(
            store
                .mutate(authorize(&request(&marked, action)))
                .unwrap_err(),
            seamone_core::review::ReviewError::UnchangedErrorMarked.to_string(),
        );
    }
    let draft = store
        .mutate(authorize(&request(
            &marked,
            ReviewAction::SaveDraft {
                text: "  표시 당시수정한 글  ".into(),
            },
        )))
        .unwrap();
    drop(store);
    let mut store = f.store();
    assert!(
        store
            .mutate(authorize(&request(&draft, ReviewAction::ConfirmCorrection)))
            .is_err()
    );
    let changed_draft = store
        .mutate(authorize(&request(
            &draft,
            ReviewAction::SaveDraft {
                text: "표시 당시 수정한 글".into(),
            },
        )))
        .unwrap();
    assert!(
        store
            .mutate(authorize(&request(
                &changed_draft,
                ReviewAction::CorrectAndConfirm { text: edited.text }
            )))
            .is_err()
    );
    assert_eq!(store.library(false).unwrap().pending_training, 0);
    let confirmed = store
        .mutate(authorize(&request(
            &changed_draft,
            ReviewAction::ConfirmCorrection,
        )))
        .unwrap();
    assert!(store.eligible(&confirmed.id, confirmed.revision).is_ok());
}

#[test]
fn dismissing_a_repeated_error_mark_preserves_draft_and_original_ranges() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let first = ErrorMark {
        id: "first".into(),
        recording_id: segment.recording_id.clone(),
        recording_revision: segment.recording_revision,
        range: SampleRange {
            start: segment.range.start,
            end: segment.range.start + 16,
        },
    };
    store.save_error_mark(&first).unwrap();
    let marked = store.segment(&segment.id).unwrap();
    assert!(marked.needs_review());
    let draft = store
        .mutate(authorize(&request(
            &marked,
            ReviewAction::SaveDraft {
                text: "사용자가 고친 글".into(),
            },
        )))
        .unwrap();
    let second = ErrorMark {
        id: "second".into(),
        range: SampleRange {
            start: first.range.end,
            end: first.range.end + 16,
        },
        ..first.clone()
    };
    store.save_error_mark(&second).unwrap();
    store.save_error_mark(&first).unwrap();
    let remarked = store.segment(&segment.id).unwrap();
    assert_eq!(remarked.revision, draft.revision + 1);
    assert_eq!(
        remarked.error_marked_text.as_deref(),
        Some(draft.text.as_str())
    );
    assert!(
        store
            .mutate(authorize(&request(
                &remarked,
                ReviewAction::ConfirmCorrection
            )))
            .is_err()
    );

    let dismissed = store
        .mutate(authorize(&request(
            &remarked,
            ReviewAction::DismissErrorMark,
        )))
        .unwrap();
    store.save_error_mark(&second).unwrap();
    drop(store);
    let store = f.store();
    assert_eq!(store.segment(&segment.id).unwrap(), dismissed);
    assert_eq!(dismissed.error_marked_text, None);
    assert_eq!(dismissed.text, draft.text);
    assert_eq!(dismissed.machine_text, segment.machine_text);
    assert_eq!(dismissed.range, segment.range);
    assert!(dismissed.needs_correction);
    assert!(store.eligible(&dismissed.id, dismissed.revision).is_err());
    assert_eq!(store.library(false).unwrap().pending_training, 0);
    assert_eq!(
        store
            .error_mark_page(&segment.recording_id, 0)
            .unwrap()
            .marks,
        [first, second]
    );
}

#[test]
fn a_failed_mark_write_rolls_back_correction_and_allows_retry() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let mark = ErrorMark {
        id: Uuid::new_v4().to_string(),
        recording_id: segment.recording_id.clone(),
        recording_revision: segment.recording_revision,
        range: SampleRange {
            start: segment.range.start,
            end: segment.range.start + 1,
        },
    };
    let db = rusqlite::Connection::open(f.root.join("app/seamone.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_mark BEFORE INSERT ON error_marks BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
    assert!(store.save_error_mark(&mark).is_err());
    assert_eq!(store.segment(&segment.id).unwrap(), segment);
    assert_eq!(
        store
            .error_mark_page(&segment.recording_id, 0)
            .unwrap()
            .marks,
        []
    );
    db.execute_batch("DROP TRIGGER fail_mark;").unwrap();
    store.save_error_mark(&mark).unwrap();
    assert!(store.segment(&segment.id).unwrap().needs_review());
}

#[test]
fn import_verify_reopen_exports_without_blocking_and_keeps_exact_snapshot() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    assert_eq!(
        store.export_text(&segment.recording_id).unwrap(),
        segment.machine_text
    );
    assert!(store.eligible(&segment.id, 1).is_err());
    assert_eq!(store.library(false).unwrap().pending_training, 0);
    let accepted = store
        .mutate(authorize(&request(
            &segment,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    assert_eq!(
        accepted.verified.as_ref().unwrap().source,
        LabelSource::Accepted
    );
    assert_eq!(store.library(false).unwrap().pending_training, 1);
    drop(store);
    let store = f.store();
    assert_eq!(store.segment(&segment.id).unwrap(), accepted);
    assert!(store.eligible(&segment.id, accepted.revision).is_ok());
}

#[test]
fn no_and_edits_are_not_labels_and_confirmation_is_invalidated_by_later_edits() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let rejected = store
        .mutate(authorize(&request(
            &segment,
            ReviewAction::Judge {
                verdict: Verdict::No,
            },
        )))
        .unwrap();
    assert!(store.next_job().unwrap().is_none());
    let draft = store
        .mutate(authorize(&request(
            &rejected,
            ReviewAction::SaveDraft {
                text: "네 안녕하십니까".into(),
            },
        )))
        .unwrap();
    assert!(store.next_job().unwrap().is_none());
    let confirmed = store
        .mutate(authorize(&request(&draft, ReviewAction::ConfirmCorrection)))
        .unwrap();
    let example = store.next_job().unwrap().unwrap();
    assert_eq!(example.segment(), &confirmed);
    assert_eq!(
        example.segment().verified.as_ref().unwrap().source,
        LabelSource::Corrected
    );
    let edited = store
        .mutate(authorize(&request(
            &confirmed,
            ReviewAction::SaveDraft {
                text: "회의 요약 초안".into(),
            },
        )))
        .unwrap();
    assert!(store.publish(&example, "candidate").is_err());
    assert!(store.eligible(&edited.id, edited.revision).is_err());
    assert_eq!(
        store.export_text(&segment.recording_id).unwrap(),
        "회의 요약 초안"
    );
}

#[test]
fn replay_is_idempotent_and_stale_or_colliding_events_fail() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let req = request(
        &segment,
        ReviewAction::Judge {
            verdict: Verdict::Yes,
        },
    );
    let accepted = store.mutate(authorize(&req)).unwrap();
    assert_eq!(store.mutate(authorize(&req)).unwrap(), accepted);
    assert_eq!(store.library(false).unwrap().pending_training, 1);
    let mut collision = req.clone();
    collision.action = ReviewAction::Judge {
        verdict: Verdict::No,
    };
    assert!(store.mutate(authorize(&collision)).is_err());
    let mut second = f.store();
    assert!(
        second
            .mutate(authorize(&request(
                &segment,
                ReviewAction::SaveDraft {
                    text: "오래된 수정".into()
                }
            )))
            .unwrap_err()
            .contains("다른 수정본")
    );
    assert_eq!(second.segment(&segment.id).unwrap(), accepted);
}

#[test]
fn queue_and_verified_snapshot_roll_back_together() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let other = rusqlite::Connection::open(f.root.join("app/seamone.sqlite3")).unwrap();
    other.execute_batch("CREATE TRIGGER fail_queue BEFORE INSERT ON training_queue BEGIN SELECT RAISE(ABORT, 'disk full simulation'); END;").unwrap();
    assert!(
        store
            .mutate(authorize(&request(
                &segment,
                ReviewAction::Judge {
                    verdict: Verdict::Yes
                }
            )))
            .is_err()
    );
    assert_eq!(store.segment(&segment.id).unwrap(), segment);
    assert_eq!(store.library(false).unwrap().pending_training, 0);
}

#[test]
fn pending_queue_recovers_and_publication_rechecks_source() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let accepted = store
        .mutate(authorize(&request(
            &segment,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    store.next_job().unwrap().unwrap();
    drop(store);
    let connection = rusqlite::Connection::open(f.root.join("app/seamone.sqlite3")).unwrap();
    connection
        .execute(
            "UPDATE training_queue SET lease_until=0 WHERE status='running'",
            [],
        )
        .unwrap();
    let mut store = f.store();
    let recovered = store.next_job().unwrap().unwrap();
    assert_eq!(recovered.segment(), &accepted);
    fs::remove_file(store.audio.existing_path(&segment.recording_id).unwrap()).unwrap();
    assert!(store.publish(&recovered, "adapter").is_err());
    store.defer_job(&recovered).unwrap();
    assert!(store.next_job().unwrap().is_none());
}

#[test]
fn playback_is_exact_truncated_range_and_rejects_zero_or_out_of_bounds() {
    let f = Fixture::new();
    let (store, segment) = f.transcribed();
    let bytes = store
        .audio
        .range_wav(
            &segment.recording_id,
            SampleRange { start: 31, end: 47 },
            16_000,
        )
        .unwrap();
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
    assert_eq!(reader.spec(), canonical_spec());
    assert_eq!(reader.duration(), 16);
    assert_eq!(
        reader
            .samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        (31..47).collect::<Vec<i16>>()
    );
    for range in [
        SampleRange { start: 0, end: 0 },
        SampleRange {
            start: 0,
            end: 16_001,
        },
    ] {
        assert!(
            store
                .audio
                .range_wav(&segment.recording_id, range, 16_000)
                .is_err()
        );
    }
    assert!(store.audio.path("../../etc/passwd").is_err());
}

#[test]
fn modified_audio_prevents_confirmation_and_training() {
    let f = Fixture::new();
    let (mut store, segment) = f.transcribed();
    let accepted = store
        .mutate(authorize(&request(
            &segment,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    fs::write(
        store.audio.existing_path(&segment.recording_id).unwrap(),
        b"changed",
    )
    .unwrap();
    assert!(store.eligible(&accepted.id, accepted.revision).is_err());
    assert!(store.next_job().unwrap().is_none());
}

#[test]
fn identical_save_preserves_verification_and_claimed_job() {
    let f = Fixture::new();
    let (mut store, s) = f.transcribed();
    let accepted = store
        .mutate(authorize(&request(
            &s,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    let example = store.next_job().unwrap().unwrap();
    let saved = store
        .mutate(authorize(&request(
            &accepted,
            ReviewAction::SaveDraft {
                text: accepted.text.clone(),
            },
        )))
        .unwrap();
    assert_eq!(saved, accepted);
    store.publish(&example, "candidate").unwrap();
    assert_eq!(store.library(false).unwrap().pending_training, 0);
}

#[test]
fn edited_and_invalidated_segments_remain_in_correction_workflow() {
    use seamone_core::review_flow::{ReviewFlow, ReviewMode};
    let f = Fixture::new();
    let (mut store, s) = f.transcribed();
    let draft = store
        .mutate(authorize(&request(
            &s,
            ReviewAction::SaveDraft {
                text: "수정한 글".into(),
            },
        )))
        .unwrap();
    assert_eq!(draft.verdict, None, "편집은 No 판정을 만들지 않는다");
    let invalidated = store
        .mutate(authorize(&request(&draft, ReviewAction::Invalidate)))
        .unwrap();
    assert!(invalidated.needs_correction);
    let mut flow = ReviewFlow::default();
    flow.start(ReviewMode::Correction, std::slice::from_ref(&invalidated));
    assert_eq!(flow.remaining(), 1);
    let confirmed = store
        .mutate(authorize(&request(
            &invalidated,
            ReviewAction::ConfirmCorrection,
        )))
        .unwrap();
    assert!(store.eligible(&confirmed.id, confirmed.revision).is_ok());
    let invalidated = store
        .mutate(authorize(&request(&confirmed, ReviewAction::Invalidate)))
        .unwrap();
    flow.start(ReviewMode::Correction, &[invalidated]);
    assert_eq!(flow.remaining(), 1);
}

#[test]
fn after_listening_a_changed_source_can_be_rejected_but_cannot_be_accepted() {
    let fixture = Fixture::new();
    let (store, segment) = fixture.transcribed();
    let path = store.audio.existing_path(&segment.recording_id).unwrap();
    let state = crate::LibraryService::new(store);
    let playback = state.playback(&segment.id, segment.revision).unwrap();
    let yes = request(
        &segment,
        ReviewAction::Judge {
            verdict: Verdict::Yes,
        },
    );
    let no = request(
        &segment,
        ReviewAction::Judge {
            verdict: Verdict::No,
        },
    );

    assert!(state.review(&no, Some(&playback.token)).is_err());
    state.playback_ended(&playback.token).unwrap();
    fs::write(path, b"changed recording").unwrap();

    assert_eq!(
        state.review(&yes, Some(&playback.token)).unwrap_err(),
        seamone_core::review::ReviewError::MissingSource.to_string()
    );
    let rejected = state.review(&no, Some(&playback.token)).unwrap();
    assert_eq!(rejected.verdict, Some(Verdict::No));
    assert!(rejected.verified.is_none());
    assert_eq!(state.review(&no, None).unwrap(), rejected);
    assert_eq!(
        state
            .store()
            .unwrap()
            .library(false)
            .unwrap()
            .pending_training,
        0
    );
}

#[test]
fn neither_correction_can_confirm_a_changed_recording_with_completed_playback() {
    let fixture = Fixture::new();
    let (store, segment) = fixture.transcribed();
    let path = store.audio.existing_path(&segment.recording_id).unwrap();
    let state = crate::LibraryService::new(store);
    let draft = state
        .review(
            &request(
                &segment,
                ReviewAction::SaveDraft {
                    text: "고쳐 쓴 말".into(),
                },
            ),
            None,
        )
        .unwrap();
    let playback = state.playback(&draft.id, draft.revision).unwrap();
    state.playback_ended(&playback.token).unwrap();
    fs::write(path, b"changed recording").unwrap();

    for action in [
        ReviewAction::ConfirmCorrection,
        ReviewAction::CorrectAndConfirm {
            text: "다시 고친 말".into(),
        },
    ] {
        assert_eq!(
            state
                .review(&request(&draft, action), Some(&playback.token))
                .unwrap_err(),
            seamone_core::review::ReviewError::MissingSource.to_string()
        );
    }
    assert_eq!(state.store().unwrap().segment(&draft.id).unwrap(), draft);
}

#[test]
fn unreadable_source_is_retryable_and_recovers_without_reconfirming() {
    let f = Fixture::new();
    let (mut store, s) = f.transcribed();
    let saved = store
        .mutate(authorize(&request(
            &s,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    let path = store.audio.existing_path(&s.recording_id).unwrap();
    let bytes = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(store.next_job().unwrap().is_none());
    assert!(
        store.next_job().unwrap().is_none(),
        "모든 원본이 없을 때도 반복문이 끝난다"
    );
    assert_eq!(store.library(false).unwrap().retry_training, 1);
    assert_eq!(store.segment(&s.id).unwrap(), saved);
    fs::write(path, bytes).unwrap();
    let example = store.next_job().unwrap().unwrap();
    assert_eq!(example.segment(), &saved);
    assert_eq!(store.library(false).unwrap().retry_training, 0);
}

#[test]
fn another_store_cannot_reset_or_publish_an_active_claim() {
    let f = Fixture::new();
    let (mut first, s) = f.transcribed();
    first
        .mutate(authorize(&request(
            &s,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    let example = first.next_job().unwrap().unwrap();
    let mut second = f.store();
    assert!(second.next_job().unwrap().is_none());
    assert!(second.publish(&example, "foreign-owner").is_err());
    first.publish(&example, "owner-output").unwrap();
}

#[test]
fn two_connections_return_only_one_successful_claim() {
    use std::sync::{Arc, Barrier};
    let f = Fixture::new();
    let (mut store, s) = f.transcribed();
    store
        .mutate(authorize(&request(
            &s,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let root = f.root.join("app");
            std::thread::spawn(move || {
                let store = Store::open(&root).unwrap();
                barrier.wait();
                store.next_job().unwrap()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        handles
            .into_iter()
            .filter_map(|h| h.join().unwrap())
            .count(),
        1,
        "store::Store::next_job에서 확인된 revision의 claim을 한 연결만 획득해야 한다"
    );
}

#[test]
fn concurrent_updates_and_replayed_events_use_durable_cas() {
    use std::sync::{Arc, Barrier};
    let f = Fixture::new();
    let (_store, s) = f.transcribed();
    let req = request(
        &s,
        ReviewAction::SaveDraft {
            text: "두 번 온 같은 초안".into(),
        },
    );
    let barrier = Arc::new(Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let root = f.root.join("app");
            let req = req.clone();
            std::thread::spawn(move || {
                let mut store = Store::open(&root).unwrap();
                barrier.wait();
                store.mutate(authorize(&req)).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let saved = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(saved[0], saved[1]);
    assert_eq!(saved[0].revision, 2);
    let mut first = f.store();
    let mut second = f.store();
    let next = request(
        &saved[0],
        ReviewAction::SaveDraft {
            text: "먼저 저장".into(),
        },
    );
    first.mutate(authorize(&next)).unwrap();
    let stale = request(&saved[0], ReviewAction::Invalidate);
    assert!(
        second
            .mutate(authorize(&stale))
            .unwrap_err()
            .contains("다른 수정본")
    );
}

#[test]
fn inference_commit_is_atomic_even_for_a_silent_zero_segment_result() {
    let f = Fixture::new();
    let mut first = f.store();
    let info = first.import(&f.source).unwrap();
    let mut second = f.store();
    let (started_first, _) = first.prepare_transcription(&info.id).unwrap();
    let (started_second, _) = second.prepare_transcription(&info.id).unwrap();
    first
        .commit_transcription(&started_first, Vec::new(), "test", "silent")
        .unwrap();
    assert!(
        second
            .commit_transcription(
                &started_second,
                vec![seamone_core::TranscriptSegment::new(0, 100, "중복")],
                "test",
                "duplicate"
            )
            .is_err()
    );
    assert_eq!(
        first.segments(&info.id).unwrap(),
        Vec::<ReviewSegment>::new()
    );
    assert!(second.prepare_transcription(&info.id).is_err());
    let info = first.import(&f.source).unwrap();
    let (one, _) = first.prepare_transcription(&info.id).unwrap();
    let (two, _) = second.prepare_transcription(&info.id).unwrap();
    first
        .commit_transcription(
            &one,
            vec![seamone_core::TranscriptSegment::new(0, 100, "한 번만")],
            "test",
            "fixture",
        )
        .unwrap();
    assert!(
        second
            .commit_transcription(&two, Vec::new(), "test", "duplicate")
            .is_err()
    );
    assert_eq!(first.segments(&info.id).unwrap().len(), 1);
}

#[test]
fn one_listen_correction_atomically_stores_edited_text_and_verified_snapshot() {
    use crate::LibraryService;
    let f = Fixture::new();
    let (mut store, s) = f.transcribed();
    let rejected = store
        .mutate(authorize(&request(
            &s,
            ReviewAction::Judge {
                verdict: Verdict::No,
            },
        )))
        .unwrap();
    let state = LibraryService::new(store);
    let playback = state.playback(&rejected.id, rejected.revision).unwrap();
    state.playback_ended(&playback.token).unwrap();
    let action = request(
        &rejected,
        ReviewAction::CorrectAndConfirm {
            text: "한 번 듣고 고친 말 그대로".into(),
        },
    );
    let saved = state.review(&action, Some(&playback.token)).unwrap();
    assert_eq!(saved.text, "한 번 듣고 고친 말 그대로");
    let label = saved.verified.as_ref().unwrap();
    assert_eq!(label.text, saved.text);
    assert_eq!(label.revision, saved.revision);
    assert_eq!(label.source, LabelSource::Corrected);
    assert!(
        state
            .store()
            .unwrap()
            .eligible(&saved.id, saved.revision)
            .is_ok()
    );
    let draft = state
        .review(
            &request(
                &saved,
                ReviewAction::SaveDraft {
                    text: "뒤에 바뀐 초안".into(),
                },
            ),
            None,
        )
        .unwrap();
    let confirm = request(
        &draft,
        ReviewAction::CorrectAndConfirm {
            text: "오래된 재생으로 확인".into(),
        },
    );
    assert!(state.review(&confirm, Some(&playback.token)).is_err());
    assert!(
        state
            .store()
            .unwrap()
            .segment(&draft.id)
            .unwrap()
            .verified
            .is_none()
    );
}

#[test]
fn unchanged_no_and_whitespace_edits_require_explicit_reset_then_binary_review() {
    let f = Fixture::new();
    let (mut store, s) = f.transcribed();
    let no = store
        .mutate(authorize(&request(
            &s,
            ReviewAction::Judge {
                verdict: Verdict::No,
            },
        )))
        .unwrap();
    assert!(
        store
            .mutate(authorize(&request(&no, ReviewAction::ConfirmCorrection)))
            .is_err()
    );
    assert!(
        store
            .mutate(authorize(&request(
                &no,
                ReviewAction::CorrectAndConfirm {
                    text: format!("  {} \n", no.machine_text)
                }
            )))
            .is_err()
    );
    assert_eq!(store.library(false).unwrap().pending_training, 0);
    let draft = store
        .mutate(authorize(&request(
            &no,
            ReviewAction::SaveDraft {
                text: format!(" {} ", no.machine_text),
            },
        )))
        .unwrap();
    assert!(
        store
            .mutate(authorize(&request(&draft, ReviewAction::ConfirmCorrection)))
            .is_err()
    );
    let reset = store
        .mutate(authorize(&request(&draft, ReviewAction::Invalidate)))
        .unwrap();
    assert_eq!(reset.verdict, None);
    assert!(!reset.needs_correction);
    assert_eq!(reset.text, reset.machine_text);
    assert!(reset.verified.is_none());
    assert_eq!(store.library(false).unwrap().pending_training, 0);
    let accepted = store
        .mutate(authorize(&request(
            &reset,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    assert!(store.eligible(&accepted.id, accepted.revision).is_ok());
    assert_eq!(accepted.verified.unwrap().source, LabelSource::Accepted);
}

struct TestTrainer<F>(F);
impl<F> crate::training::LocalTrainer for TestTrainer<F>
where
    F: FnMut(&seamone_core::review::EligibleExample, &[f32]) -> Result<String, String> + Send,
{
    fn train(
        &mut self,
        example: &seamone_core::review::EligibleExample,
        audio: &[f32],
    ) -> Result<String, String> {
        self.0(example, audio)
    }
}

#[test]
fn foreground_work_defers_a_candidate_and_source_failure_is_retryable() {
    use crate::training::{Foreground, run_next};
    use std::sync::{Arc, Mutex};
    let f = Fixture::new();
    let (mut store, s) = f.transcribed();
    store
        .mutate(authorize(&request(
            &s,
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
        )))
        .unwrap();
    let source = store.audio.existing_path(&s.recording_id).unwrap();
    let original = fs::read(&source).unwrap();
    let store = Mutex::new(store);
    let foreground = Arc::new(Foreground::default());
    let activity = Arc::clone(&foreground);
    let mut preempted = TestTrainer(
        move |_: &seamone_core::review::EligibleExample, audio: &[f32]| {
            assert_eq!(audio.len(), 16_000);
            let _work = activity.enter();
            Ok("test-only-candidate".into())
        },
    );
    assert!(!run_next(&store, &foreground, &mut preempted).unwrap());
    assert_eq!(
        store
            .lock()
            .unwrap()
            .library(false)
            .unwrap()
            .pending_training,
        1
    );
    let mut removed_source = TestTrainer(
        move |_: &seamone_core::review::EligibleExample, _: &[f32]| {
            fs::remove_file(&source).unwrap();
            Ok("test-only-candidate".into())
        },
    );
    assert!(run_next(&store, &foreground, &mut removed_source).is_err());
    assert_eq!(
        store.lock().unwrap().library(false).unwrap().retry_training,
        1
    );
    let path = store.lock().unwrap().audio.path(&s.recording_id).unwrap();
    fs::write(path, original).unwrap();
    let mut ready = TestTrainer(|_: &seamone_core::review::EligibleExample, _: &[f32]| {
        Ok("test-only-candidate".into())
    });
    assert!(run_next(&store, &foreground, &mut ready).unwrap());
    assert_eq!(
        store
            .lock()
            .unwrap()
            .library(false)
            .unwrap()
            .pending_training,
        0
    );
}

#[cfg(all(
    feature = "desktop-whisper",
    not(any(target_os = "android", target_os = "ios"))
))]
#[test]
#[ignore = "requires offline official Whisper small model and a public Korean WAV"]
fn real_desktop_path() {
    use crate::{LibraryService, inference};
    let f = Fixture::new();
    let model =
        PathBuf::from(std::env::var("SEAMONE_WHISPER_MODEL").expect("SEAMONE_WHISPER_MODEL"));
    let wav = PathBuf::from(std::env::var("SEAMONE_TEST_WAV").expect("SEAMONE_TEST_WAV"));
    let mut store = f.store();
    let info = store.import(&wav).unwrap();
    let state = LibraryService::new(store);
    inference::select(&state, &model).unwrap();
    let segments = inference::transcribe(&state, &info.id).unwrap();
    assert_ne!(segments, Vec::<ReviewSegment>::new());
    assert!(segments.iter().all(|s| !s.machine_text.trim().is_empty()));
    let s = segments
        .iter()
        .find(|line| line.display_timing().is_some_and(|spans| spans.len() > 1))
        .unwrap_or_else(|| panic!("Whisper small returned no aligned text groups: {segments:?}"));
    let old_playback = state.playback(&s.id, s.revision).unwrap();
    state.playback_ended(&old_playback.token).unwrap();
    let middle = s.range.start + (s.range.end - s.range.start) / 2;
    let mark = ErrorMark {
        id: Uuid::new_v4().to_string(),
        recording_id: info.id.clone(),
        recording_revision: info.revision,
        range: SampleRange {
            start: s.range.start,
            end: middle,
        },
    };
    let marked = {
        let mut store = state.store().unwrap();
        let preview = store
            .audio
            .range_wav(&info.id, mark.range, info.samples)
            .unwrap();
        assert_eq!(
            u64::from(
                hound::WavReader::new(Cursor::new(preview))
                    .unwrap()
                    .duration()
            ),
            middle - s.range.start
        );
        store.save_error_mark(&mark).unwrap();
        let marked = store.segment(&s.id).unwrap();
        assert!(marked.needs_review());
        assert!(store.eligible(&marked.id, marked.revision).is_err());
        marked
    };
    let draft = state
        .review(
            &request(
                &marked,
                ReviewAction::SaveDraft {
                    text: "통합시험용 수정본".into(),
                },
            ),
            None,
        )
        .unwrap();
    assert!(draft.display_timing().is_none());
    let confirm = request(&draft, ReviewAction::ConfirmCorrection);
    assert!(state.review(&confirm, None).is_err());
    assert!(state.review(&confirm, Some(&old_playback.token)).is_err());
    let playback = state.playback(&draft.id, draft.revision).unwrap();
    assert!(state.review(&confirm, Some(&playback.token)).is_err());
    // Exercise the native gate with a simulated UI event.
    state.playback_ended(&playback.token).unwrap();
    let confirmed = state.review(&confirm, Some(&playback.token)).unwrap();
    assert!(
        state
            .store()
            .unwrap()
            .eligible(&confirmed.id, confirmed.revision)
            .is_ok()
    );
    drop(state);
    let store = f.store();
    let reopened = store.segment(&confirmed.id).unwrap();
    assert_eq!(reopened, confirmed);
    assert_eq!(reopened.machine_timing, s.machine_timing);
    assert!(reopened.display_timing().is_none());
    assert_eq!(
        store.error_mark_page(&info.id, 0).unwrap().marks,
        vec![mark]
    );
    println!(
        "canonical import -> real Whisper small with multiple DTW text spans -> range preview -> error mark -> full revision playback -> explicit confirmation -> eligible -> reopen passed (simulated review, no training)"
    );

    #[cfg(unix)]
    {
        let selected = f.root.join("selected-model.bin");
        std::os::unix::fs::symlink(&model, &selected).unwrap();
        let state = LibraryService::new(f.store());
        inference::select(&state, &selected).unwrap();
        fs::remove_file(&selected).unwrap();
        fs::write(&selected, b"replaced model bytes").unwrap();
        let error = inference::transcribe(&state, &info.id).unwrap_err();
        assert!(error.contains("SHA-256"));
    }
}
