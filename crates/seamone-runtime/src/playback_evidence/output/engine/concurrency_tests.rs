use super::*;
use crate::{LibraryService, playback_evidence::player::Player, store_tests::Fixture};
use seamone_core::review::{Action as ReviewAction, Request, Verdict};
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

struct DeviceControl {
    fail: AtomicBool,
    block: AtomicBool,
    started: mpsc::Sender<()>,
    resume: Mutex<mpsc::Receiver<()>>,
}

struct ControlledDevice(Arc<DeviceControl>);

impl Device for ControlledDevice {
    fn start(&self, source: Arc<[f32]>, at: u64) -> Result<Running, String> {
        if self.0.fail.load(Ordering::SeqCst) {
            return Err("device latency unavailable".into());
        }
        if self.0.block.load(Ordering::SeqCst) {
            self.0.started.send(()).map_err(|error| error.to_string())?;
            self.0
                .resume
                .lock()
                .map_err(|error| error.to_string())?
                .recv()
                .map_err(|error| error.to_string())?;
        }
        super::super::fake::Device.start(source, at)
    }
}

struct DeviceHarness {
    player: Player,
    control: Arc<DeviceControl>,
    started: mpsc::Receiver<()>,
    resume: mpsc::Sender<()>,
}

impl DeviceHarness {
    fn new() -> Self {
        let (entered, started) = mpsc::channel();
        let (resume, released) = mpsc::channel();
        let control = Arc::new(DeviceControl {
            fail: AtomicBool::new(false),
            block: AtomicBool::new(false),
            started: entered,
            resume: Mutex::new(released),
        });
        let device = Arc::clone(&control);
        Self {
            player: Player::with_output(Box::new(move |source, origin| {
                create(
                    source,
                    origin,
                    Box::new(ControlledDevice(Arc::clone(&device))),
                )
            })),
            control,
            started,
            resume,
        }
    }
}

#[test]
fn pending_review_during_device_open_leaves_library_reads_and_draft_saves_available() {
    for reopening in [false, true] {
        let fixture = Fixture::new();
        let (store, segment) = fixture.transcribed();
        let harness = DeviceHarness::new();
        let mut service = LibraryService::new(store);
        service.listening.player = Mutex::new(harness.player);
        let (authorizing, authorization_started) = mpsc::channel();
        service.listening.authorization_started = Some(authorizing);
        let service = Arc::new(service);
        let playback = service.playback(&segment.id, segment.revision).unwrap();
        service
            .control_playback(&playback.session, Action::Play)
            .unwrap();
        service.listening.finish(&playback.session).unwrap();
        assert!(service.playback_status().unwrap().heard);
        service
            .control_playback(&playback.session, Action::Stop)
            .unwrap();
        harness.control.block.store(true, Ordering::SeqCst);
        let opening_service = Arc::clone(&service);
        let opening_segment = segment.clone();
        let session = playback.session.clone();
        let opening = thread::spawn(move || {
            if reopening {
                opening_service
                    .control_playback(&session, Action::Play)
                    .map(|_| ())
            } else {
                opening_service
                    .playback(&opening_segment.id, opening_segment.revision)
                    .map(|_| ())
            }
        });
        harness
            .started
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        let reviewing_service = Arc::clone(&service);
        let request = Request {
            event_id: "pending-review".into(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision,
            action: ReviewAction::Judge {
                verdict: Verdict::No,
            },
        };
        let reviewing =
            thread::spawn(move || reviewing_service.review(&request, Some(&playback.session)));
        authorization_started
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        let (available, observed) = mpsc::channel();
        let reading_service = Arc::clone(&service);
        let expected_recording = segment.recording_id.clone();
        let reading = thread::spawn(move || {
            let library = reading_service.library()?;
            let saved = reading_service.review(
                &Request {
                    event_id: "independent-draft".into(),
                    segment_id: segment.id,
                    expected_revision: segment.revision,
                    action: ReviewAction::SaveDraft {
                        text: "후속 초안".into(),
                    },
                },
                None,
            )?;
            available.send(()).map_err(|error| error.to_string())?;
            Ok::<_, String>((library, saved))
        });
        let unblocked = observed.recv_timeout(Duration::from_secs(2));
        harness.resume.send(()).unwrap();
        opening.join().unwrap().unwrap();
        let review = reviewing.join().unwrap();
        let read = reading.join().unwrap();
        assert!(
            unblocked.is_ok(),
            "pending review blocked library reads and draft saves"
        );
        let (library, saved) = read.unwrap();
        assert!(
            library
                .recordings
                .iter()
                .any(|recording| recording.id == expected_recording)
        );
        assert_eq!(saved.text, "후속 초안");
        assert!(review.is_err());
    }
}

#[test]
fn failed_device_start_on_load_discards_any_previously_heard_session() {
    for previously_heard in [false, true] {
        let mut harness = DeviceHarness::new();
        if previously_heard {
            let session = harness.player.load(vec![0.25; 16_000], 0, None).unwrap();
            harness.player.control(&session, Action::Play).unwrap();
            harness.player.finish().unwrap();
            assert!(harness.player.status().heard);
        }
        harness.control.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            harness
                .player
                .load(vec![0.25; 16_000], 0, None)
                .unwrap_err(),
            "device latency unavailable"
        );
        let failed = harness.player.status();
        assert_eq!(failed.state, State::Failed);
        assert!(!failed.heard);
        assert!(failed.error.is_some());
        assert!(failed.session.is_none());
    }
}

#[test]
fn failed_device_start_after_stop_revokes_heard_review_authority() {
    let mut harness = DeviceHarness::new();
    let session = harness
        .player
        .load(vec![0.25; 16_000], 0, Some(("segment".into(), 7)))
        .unwrap();
    harness.player.control(&session, Action::Play).unwrap();
    harness.player.finish().unwrap();
    let request = Request {
        event_id: "review".into(),
        segment_id: "segment".into(),
        expected_revision: 7,
        action: ReviewAction::Judge {
            verdict: Verdict::Yes,
        },
    };
    harness.player.control(&session, Action::Stop).unwrap();
    assert!(harness.player.authorizes(&request, Some(&session)));
    harness.control.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        harness.player.control(&session, Action::Play).unwrap_err(),
        "device latency unavailable"
    );
    let failed = harness.player.status();
    assert_eq!(failed.state, State::Failed);
    assert!(!failed.heard);
    assert!(failed.error.is_some());
    assert!(!harness.player.authorizes(&request, Some(&session)));
}
