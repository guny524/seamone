use super::*;
use crate::{audio_input_tests::fixture, store::Store, training::Foreground};

struct SyntheticStream(Arc<AtomicBool>);

impl Drop for SyntheticStream {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn synthetic_input(active: Arc<AtomicBool>) -> Input<SyntheticStream> {
    let (mut producer, consumer) = RingBuffer::new(800);
    for _ in 0..800 {
        producer.push(-0.375).unwrap();
    }
    active.store(true, Ordering::Release);
    Input {
        stream: SyntheticStream(active),
        consumer,
        rate: 16_000,
        channels: 1,
    }
}

#[test]
fn duration_limit_releases_input_and_retains_only_samples_up_to_the_limit() {
    let root = fixture();
    let store = Store::open(root.path()).unwrap();
    let control = Arc::new(Control::new());
    let active = Arc::new(AtomicBool::new(true));
    let (mut producer, consumer) = RingBuffer::new(16_100);
    for _ in 0..16_000 {
        producer.push(-0.375).unwrap();
    }
    for _ in 0..100 {
        producer.push(0.75).unwrap();
    }
    let mut staged = record(&store.audio, &control, 1, |_| {
        Ok(Some(Input {
            stream: SyntheticStream(Arc::clone(&active)),
            rate: 16_000,
            channels: 1,
            consumer,
        }))
    })
    .unwrap()
    .unwrap();
    assert!(!active.load(Ordering::Acquire));
    assert!(matches!(control.status(), Status::LimitReached { .. }));
    let saved = store
        .save_audio("합성 음성".into(), &mut staged, || true)
        .unwrap();
    assert_eq!(saved.samples, 16_000);
    let (_, pcm) = store.prepare_transcription(&saved.id).unwrap();
    assert!(pcm.iter().all(|s| (*s + 0.375).abs() < f32::EPSILON));
}

struct ReleaseTailStream(Producer<f32>);

impl Drop for ReleaseTailStream {
    fn drop(&mut self) {
        self.0.push(-0.5).unwrap();
    }
}

#[test]
fn saving_drains_the_samples_accepted_while_releasing_the_input_stream() {
    let root = fixture();
    let store = Store::open(root.path()).unwrap();
    let control = Arc::new(Control::new());
    let mut audio = record(&store.audio, &control, 1, |control| {
        let (mut producer, consumer) = RingBuffer::new(8);
        producer.push(0.5).unwrap();
        control.request.store(SAVE, Ordering::Release);
        Ok(Some(Input {
            stream: ReleaseTailStream(producer),
            consumer,
            rate: 16_000,
            channels: 1,
        }))
    })
    .unwrap()
    .unwrap();
    let saved = store
        .save_audio("합성 음성".into(), &mut audio, || true)
        .unwrap();
    let (_, pcm) = store.prepare_transcription(&saved.id).unwrap();
    assert_eq!(pcm, [0.5, -0.5]);
}

fn wait_for(recorder: &Recorder, accepts: impl Fn(&Status) -> bool) -> Status {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let status = recorder.status();
        if accepts(&status) {
            return status;
        }
        assert!(Instant::now() < deadline, "timed out in {status:?}");
        thread::sleep(INPUT_POLL);
    }
}

#[test]
fn capture_recovers_from_open_failure_and_keeps_received_audio_on_disconnect_or_stall() {
    let root = fixture();
    let store = Store::open(root.path()).unwrap();
    let foreground = Arc::new(Foreground::default());
    let recorder = Recorder::default();
    recorder
        .start_using::<()>(store.audio.clone(), foreground.enter(), |_| {
            Err("입력 장치 없음".into())
        })
        .unwrap();
    assert_eq!(
        wait_for(&recorder, |s| matches!(s, Status::Failed { .. })),
        Status::Failed {
            message: "입력 장치 없음".into()
        }
    );

    for (fault, text) in [
        (DISCONNECTED, "연결이 끊겼습니다"),
        (0, "샘플을 받지 못했습니다"),
    ] {
        let stream_active = Arc::new(AtomicBool::new(false));
        let active = Arc::clone(&stream_active);
        recorder
            .start_using(store.audio.clone(), foreground.enter(), move |control| {
                let input = synthetic_input(active);
                control.fault.store(fault, Ordering::Release);
                Ok(Some(input))
            })
            .unwrap();
        let Status::Interrupted {
            session_id, reason, ..
        } = wait_for(&recorder, |s| matches!(s, Status::Interrupted { .. }))
        else {
            unreachable!()
        };
        assert!(reason.contains(text), "{reason}");
        assert!(!stream_active.load(Ordering::Acquire));
        assert!(
            !recorder.request_close(),
            "received audio is awaiting a choice"
        );
        assert!(
            recorder
                .start_using::<()>(store.audio.clone(), foreground.enter(), |_| {
                    panic!("pending capture opened an input")
                })
                .is_err(),
            "cannot replace the pending recording"
        );
        let saved = recorder
            .finish(&session_id, true, |audio, active| {
                store.save_audio("합성 음성".into(), audio, active)
            })
            .unwrap()
            .unwrap();
        assert_eq!(saved.samples, 800);
        let (_, pcm) = store.prepare_transcription(&saved.id).unwrap();
        assert!(pcm.iter().all(|s| (*s + 0.375).abs() < f32::EPSILON));
    }
    assert_eq!(store.library(false).unwrap().recordings.len(), 2);
}

#[test]
fn saving_again_after_a_sqlite_write_error_keeps_the_same_audio_once() {
    let root = fixture();
    let store = Store::open(root.path()).unwrap();
    let foreground = Arc::new(Foreground::default());
    let recorder = Recorder::default();
    let stream_active = Arc::new(AtomicBool::new(false));
    let active = Arc::clone(&stream_active);
    recorder
        .start_using(store.audio.clone(), foreground.enter(), move |_| {
            Ok(Some(synthetic_input(active)))
        })
        .unwrap();
    let Status::Recording { session_id, .. } =
        wait_for(&recorder, |s| matches!(s, Status::Recording { .. }))
    else {
        unreachable!()
    };
    let db = rusqlite::Connection::open(root.path().join("seamone.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_save BEFORE INSERT ON recordings BEGIN SELECT RAISE(FAIL, 'test error'); END;").unwrap();
    let commit = |audio: &mut StagedAudio, active: &dyn Fn() -> bool| {
        store.save_audio("합성 음성".into(), audio, active)
    };
    assert!(
        recorder
            .finish(&session_id, true, commit)
            .unwrap_err()
            .contains("test error")
    );
    assert!(!stream_active.load(Ordering::Acquire));
    assert!(
        matches!(recorder.status(), Status::Interrupted { session_id: id, .. } if id == session_id)
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("audio"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(store.library(false).unwrap().recordings, []);
    assert!(!recorder.request_close());

    db.execute_batch("DROP TRIGGER reject_save;").unwrap();
    let saved = recorder.finish(&session_id, true, commit).unwrap().unwrap();
    assert!(recorder.finish(&session_id, true, commit).is_err());
    let reopened = Store::open(root.path()).unwrap();
    assert_eq!(
        reopened.library(false).unwrap().recordings,
        vec![saved.clone()]
    );
    let (_, pcm) = reopened.prepare_transcription(&saved.id).unwrap();
    assert_eq!(pcm.len(), 800);
    assert!(pcm.iter().all(|s| (*s + 0.375).abs() < f32::EPSILON));
    assert!(recorder.request_close());
}

#[test]
fn forced_close_during_save_discards_and_rejects_overlapping_finish() {
    let root = fixture();
    let recorder = Arc::new(Recorder::default());
    let store = Store::open(root.path()).unwrap();
    let foreground = Arc::new(Foreground::default());
    let stream_active = Arc::new(AtomicBool::new(false));
    let active = Arc::clone(&stream_active);
    recorder
        .start_using(store.audio.clone(), foreground.enter(), move |_| {
            Ok(Some(synthetic_input(active)))
        })
        .unwrap();
    let Status::Recording { session_id, .. } =
        wait_for(&recorder, |s| matches!(s, Status::Recording { .. }))
    else {
        unreachable!()
    };
    let (committing, entered) = std::sync::mpsc::channel();
    let (resume, resume_commit) = std::sync::mpsc::channel();
    let worker = Arc::clone(&recorder);
    let id = session_id.clone();
    let saving = thread::spawn(move || {
        worker.finish(&id, true, |audio, active| {
            committing.send(()).unwrap();
            resume_commit.recv_timeout(Duration::from_secs(2)).unwrap();
            store.save_audio("합성 음성".into(), audio, active)
        })
    });
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(!stream_active.load(Ordering::Acquire));
    assert!(
        recorder
            .finish(&session_id, false, |_, _| unreachable!())
            .is_err()
    );
    assert!(!recorder.request_close());
    recorder.close();
    resume.send(()).unwrap();
    assert!(saving.join().unwrap().unwrap_err().contains("창이 닫혀"));
    let store = Store::open(root.path()).unwrap();
    assert_eq!(store.library(false).unwrap().recordings, []);
    assert_eq!(
        std::fs::read_dir(root.path().join("audio"))
            .unwrap()
            .count(),
        0
    );
}
