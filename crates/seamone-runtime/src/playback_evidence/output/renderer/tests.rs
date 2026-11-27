use super::super::heard::Heard;
use super::*;

struct Device {
    renderer: Renderer,
    reader: Reader,
    controls: Producer<Action>,
    heard: Heard,
    frames: u64,
    rate: u32,
    channels: u16,
    fault: Arc<AtomicBool>,
}

impl Device {
    fn new(source: Vec<f32>, rate: u32, channels: u16) -> Self {
        let length = u64::try_from(source.len()).unwrap();
        let fault = Arc::new(AtomicBool::new(false));
        let (renderer, reader, controls) = Renderer::new(
            source.into(),
            Format { rate, channels },
            0,
            Arc::clone(&fault),
        )
        .unwrap();
        Self {
            renderer,
            reader,
            controls,
            heard: Heard::new(length),
            frames: 0,
            rate,
            channels,
            fault,
        }
    }

    fn action(&mut self, action: Action) {
        self.controls.push(action).unwrap();
    }

    fn sync(&mut self) -> Snapshot {
        while let Some(run) = self.reader.closed() {
            self.heard.add(run);
        }
        let snapshot = self.reader.snapshot().unwrap();
        self.heard.add(PlayedRun {
            start: snapshot.run.start,
            end: snapshot.run.end,
        });
        if self.reader.broken() || snapshot.state == State::Failed {
            self.heard.invalidate();
        }
        snapshot
    }

    fn render(&mut self, frames: usize, timing: Timing) -> Vec<f32> {
        let mut output = vec![0.0; frames * usize::from(self.channels)];
        self.renderer.render(&mut output, timing, |sample| sample);
        self.sync();
        output
    }

    fn tick(&mut self, frames: usize) -> Vec<f32> {
        let time = self.frames * 1_000_000_000 / u64::from(self.rate);
        let output = self.render(
            frames,
            Timing {
                callback: time,
                playback: time,
            },
        );
        self.frames += u64::try_from(frames).unwrap();
        output
    }
}

#[test]
fn submitted_buffers_are_not_heard_until_the_device_clock_reaches_their_end() {
    let mut device = Device::new(vec![0.5; 16_000], 48_000, 1);
    device.action(Action::Play);
    device.render(
        48_000,
        Timing {
            callback: 0,
            playback: 20_000_000,
        },
    );
    assert!(!device.heard.complete());
    assert_eq!(device.sync().position, 0);
    device.render(
        512,
        Timing {
            callback: 1_019_999_999,
            playback: 1_039_999_999,
        },
    );
    assert!(!device.heard.complete());
    device.render(
        512,
        Timing {
            callback: 1_020_000_000,
            playback: 1_040_000_000,
        },
    );
    assert!(device.heard.complete());
    assert_eq!(device.sync().state, State::Finished);
    assert_eq!(device.sync().position, 16_000);
}

#[test]
fn device_latency_delays_coverage_and_cursor_until_the_buffer_has_left_the_device() {
    use std::time::Duration;
    let mut device = Device::new(vec![0.5; 16_000], 48_000, 1);
    device.action(Action::Play);
    let stamp = |callback| {
        super::super::clock::timing(
            Some(Duration::from_nanos(callback)),
            Some(Duration::from_nanos(callback + 20_000_000)),
            Some(Duration::from_millis(250)),
        )
        .unwrap()
    };
    device.render(48_000, stamp(0));
    device.render(512, stamp(1_020_000_000));
    assert!(!device.heard.complete());
    assert_eq!(device.sync().position, 0);
    device.action(Action::Pause);
    device.render(512, stamp(1_269_999_999));
    assert!(!device.heard.complete());
    assert_eq!(device.sync().position, 0);
    device.render(512, stamp(1_270_000_000));
    assert!(device.heard.complete());
    assert_eq!(device.sync().position, 16_000);
}

#[test]
fn pausing_retains_only_played_audio_and_resuming_can_complete_it() {
    let mut device = Device::new(vec![0.5; 16_000], 48_000, 1);
    device.action(Action::Play);
    device.tick(24_000);
    device.action(Action::Pause);
    let paused = device.tick(512);
    assert!(paused.iter().all(|sample| *sample == 0.0));
    assert_eq!(device.sync().state, State::Paused);
    assert_eq!(device.sync().position, 8_000);
    assert!(!device.heard.complete());
    device.tick(24_000);
    assert_eq!(device.sync().position, 8_000);
    device.action(Action::Play);
    device.tick(24_000);
    device.tick(512);
    assert!(device.heard.complete());
}

#[test]
fn rewinding_does_not_count_the_same_half_twice() {
    let mut device = Device::new(vec![0.5; 16_000], 48_000, 1);
    device.action(Action::Play);
    device.tick(24_000);
    device.action(Action::Pause);
    device.tick(512);
    device.action(Action::Seek { at: 0 });
    device.action(Action::Play);
    device.tick(24_000);
    device.action(Action::Pause);
    device.tick(512);
    assert!(!device.heard.complete());
    assert_eq!(device.sync().position, 8_000);
    device.action(Action::Play);
    device.tick(24_000);
    device.tick(512);
    assert!(device.heard.complete());
}

#[test]
fn seeking_forward_leaves_a_gap_that_a_later_run_can_fill() {
    let mut device = Device::new(vec![0.5; 16_000], 48_000, 1);
    device.action(Action::Play);
    device.tick(24_000);
    device.action(Action::Pause);
    device.tick(512);
    device.action(Action::Seek { at: 12_000 });
    device.action(Action::Play);
    device.tick(12_000);
    device.tick(512);
    assert_eq!(device.sync().state, State::Finished);
    assert!(!device.heard.complete());
    device.action(Action::Seek { at: 8_000 });
    device.action(Action::Play);
    device.tick(12_000);
    device.tick(512);
    assert!(device.heard.complete());
}

#[test]
fn missing_device_latency_invalidates_the_output_session() {
    use std::time::Duration;
    let mut device = Device::new(vec![0.5; 16_000], 48_000, 1);
    device.action(Action::Play);
    let timing = super::super::clock::timing(Some(Duration::ZERO), Some(Duration::ZERO), None)
        .unwrap_or(Timing {
            callback: 1,
            playback: 0,
        });
    device.render(48_000, timing);
    assert_eq!(device.sync().state, State::Failed);
    assert!(!device.heard.complete());
    assert_eq!(device.sync().position, 0);
}

#[test]
fn invalid_device_timestamps_never_authorize_submitted_audio() {
    for timing in [
        Timing {
            callback: 1,
            playback: 0,
        },
        Timing {
            callback: 0,
            playback: 0,
        },
    ] {
        let mut device = Device::new(vec![0.5; 16_000], 48_000, 1);
        device.action(Action::Play);
        device.render(
            48_000,
            Timing {
                callback: 0,
                playback: 0,
            },
        );
        device.render(512, timing);
        assert_eq!(device.sync().state, State::Failed);
        assert!(!device.heard.complete());
    }
    let mut device = Device::new(vec![0.5; 16_000], 48_000, 1);
    device.action(Action::Play);
    device.render(
        48_000,
        Timing {
            callback: 100,
            playback: 20_000_000,
        },
    );
    device.render(
        512,
        Timing {
            callback: 200,
            playback: 19_999_999,
        },
    );
    assert_eq!(device.sync().state, State::Failed);
    assert!(!device.heard.complete());
}

#[test]
fn output_errors_invalidate_already_heard_audio() {
    let mut device = Device::new(vec![0.5; 16], 48_000, 1);
    device.action(Action::Play);
    device.tick(48);
    device.tick(48);
    assert!(device.heard.complete());
    device.fault.store(true, Ordering::Release);
    device.tick(48);
    assert!(!device.heard.complete());
    assert_eq!(device.sync().state, State::Failed);
    device.action(Action::Play);
    device.tick(48);
    assert!(!device.heard.complete());
}

#[test]
fn resampling_finishes_at_the_exact_source_end_at_common_device_rates() {
    for rate in [44_100, 48_000] {
        let mut device = Device::new(vec![0.5; 161], rate, 2);
        device.action(Action::Play);
        let mut audible = 0;
        for _ in 0..8 {
            let output = device.tick(257);
            for frame in output.as_chunks::<2>().0 {
                assert_eq!(frame[0], frame[1]);
                audible += usize::from(frame[0] != 0.0);
            }
        }
        let expected = (161_u64 * u64::from(rate)).div_ceil(16_000);
        assert_eq!(u64::try_from(audible).unwrap(), expected);
        assert_eq!(device.sync().position, 161);
        assert_eq!(device.sync().state, State::Finished);
        assert!(device.heard.complete());
    }
}

#[test]
fn mono_tone_keeps_its_frequency_when_rendered_as_stereo_at_48_khz() {
    let source = (0..16_000_u16)
        .map(|index| (std::f32::consts::TAU * f32::from(index % 16) / 16.0).sin())
        .collect();
    let mut device = Device::new(source, 48_000, 2);
    device.action(Action::Play);
    let output = device.tick(48_000);
    let mut previous = 0.0;
    let mut rising = 0;
    for frame in output.as_chunks::<2>().0 {
        assert_eq!(frame[0], frame[1]);
        assert!(frame[0].abs() <= 1.0);
        rising += usize::from(previous <= 0.0 && frame[0] > 0.0);
        previous = frame[0];
    }
    assert_eq!(rising, 1_000);
    assert!(!device.heard.complete());
    device.tick(512);
    assert!(device.heard.complete());
}
