//! Callback-owned progress uses device time and never allocates while rendering.

use num_traits::ToPrimitive;
use rtrb::{Consumer, Producer, RingBuffer};
use seamone_core::playback::{Action, State};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
};

const SOURCE_RATE: u64 = 16_000;
const COMMANDS: usize = 128;
const PENDING_BUFFERS: usize = 2_048;

pub(super) struct PlayedRun {
    start: u64,
    end: u64,
}

impl PlayedRun {
    pub(super) fn range(self) -> std::ops::Range<u64> {
        self.start..self.end
    }
}

#[derive(Clone, Copy)]
pub(super) struct Timing {
    pub(super) callback: u64,
    pub(super) playback: u64,
}

pub(super) struct Format {
    pub(super) rate: u32,
    pub(super) channels: u16,
}

impl Format {
    pub(super) fn validate(&self) -> Result<(), String> {
        if !(16_000..=192_000).contains(&self.rate) || !(1..=32).contains(&self.channels) {
            return Err("지원하지 않는 출력 장치 설정입니다.".into());
        }
        Ok(())
    }
}

struct Progress {
    sequence: AtomicU64,
    start: AtomicU64,
    end: AtomicU64,
    state: AtomicU8,
}

pub(super) struct Snapshot {
    pub(super) state: State,
    pub(super) position: u64,
    pub(super) run: PlayedRun,
}

pub(super) struct Reader {
    progress: Arc<Progress>,
    closed: Consumer<PlayedRun>,
    fault: Arc<AtomicBool>,
}

impl Reader {
    #[cfg(test)]
    pub(super) fn inject_error(&self) {
        self.fault.store(true, Ordering::SeqCst);
    }

    pub(super) fn broken(&self) -> bool {
        self.fault.load(Ordering::SeqCst)
    }

    pub(super) fn closed(&mut self) -> Option<PlayedRun> {
        self.closed.pop().ok()
    }

    pub(super) fn snapshot(&self) -> Option<Snapshot> {
        for _ in 0..8 {
            let before = self.progress.sequence.load(Ordering::SeqCst);
            if !before.is_multiple_of(2) {
                continue;
            }
            let start = self.progress.start.load(Ordering::SeqCst);
            let end = self.progress.end.load(Ordering::SeqCst);
            let state = match self.progress.state.load(Ordering::SeqCst) {
                0 => State::Ready,
                1 => State::Playing,
                2 => State::Paused,
                3 => State::Finished,
                _ => State::Failed,
            };
            let after = self.progress.sequence.load(Ordering::SeqCst);
            if before == after {
                return Some(Snapshot {
                    state,
                    position: end,
                    run: PlayedRun { start, end },
                });
            }
        }
        None
    }
}

struct Pending {
    deadline: u64,
    end: u64,
}

pub(super) struct Renderer {
    source: Arc<[f32]>,
    format: Format,
    commands: Consumer<Action>,
    closed: Producer<PlayedRun>,
    progress: Arc<Progress>,
    fault: Arc<AtomicBool>,
    pending: VecDeque<Pending>,
    cursor: u64,
    start: u64,
    confirmed: u64,
    state: State,
    previous: Option<(u64, u64)>,
}

impl Renderer {
    pub(super) fn new(
        source: Arc<[f32]>,
        format: Format,
        at: u64,
        fault: Arc<AtomicBool>,
    ) -> Result<(Self, Reader, Producer<Action>), String> {
        format.validate()?;
        if source.is_empty()
            || source.len()
                > usize::try_from(seamone_core::review::MAX_PLAYBACK_SAMPLES).unwrap_or(0)
            || source.iter().any(|sample| !sample.is_finite())
            || at > u64::try_from(source.len()).map_err(|e| e.to_string())?
        {
            return Err("재생할 음성 범위가 잘못됐습니다.".into());
        }
        let (commands, control) = RingBuffer::new(COMMANDS);
        let (closed, completed) = RingBuffer::new(COMMANDS);
        let progress = Arc::new(Progress {
            sequence: AtomicU64::new(0),
            start: AtomicU64::new(at),
            end: AtomicU64::new(at),
            state: AtomicU8::new(0),
        });
        let reader = Reader {
            progress: Arc::clone(&progress),
            closed: completed,
            fault: Arc::clone(&fault),
        };
        let renderer = Self {
            source,
            cursor: at * u64::from(format.rate),
            format,
            commands: control,
            closed,
            progress,
            fault,
            pending: VecDeque::with_capacity(PENDING_BUFFERS),
            start: at,
            confirmed: at,
            state: State::Ready,
            previous: None,
        };
        Ok((renderer, reader, commands))
    }

    fn fail(&mut self) {
        self.state = State::Failed;
        self.pending.clear();
    }

    fn publish(&self) {
        self.progress.sequence.fetch_add(1, Ordering::SeqCst);
        self.progress.start.store(self.start, Ordering::SeqCst);
        self.progress.end.store(self.confirmed, Ordering::SeqCst);
        self.progress.state.store(
            match self.state {
                State::Ready => 0,
                State::Playing => 1,
                State::Paused => 2,
                State::Finished => 3,
                _ => 4,
            },
            Ordering::SeqCst,
        );
        self.progress.sequence.fetch_add(1, Ordering::SeqCst);
    }

    fn seek(&mut self, at: u64) {
        if self
            .closed
            .push(PlayedRun {
                start: self.start,
                end: self.confirmed,
            })
            .is_err()
        {
            self.fail();
            return;
        }
        self.pending.clear();
        self.cursor = at * u64::from(self.format.rate);
        self.start = at;
        self.confirmed = at;
        if self.state == State::Finished {
            self.state = State::Paused;
        }
    }

    fn controls(&mut self) {
        while let Ok(action) = self.commands.pop() {
            if self.state == State::Failed {
                break;
            }
            match action {
                Action::Play => {
                    if self.state == State::Finished {
                        self.seek(0);
                    }
                    if self.state != State::Failed {
                        self.state = State::Playing;
                    }
                }
                Action::Pause | Action::Stop => {
                    if self.state == State::Playing {
                        self.state = State::Paused;
                    }
                }
                Action::Seek { at } => self.seek(at),
            }
        }
    }

    pub(super) fn render<T: Copy>(
        &mut self,
        output: &mut [T],
        timing: Timing,
        convert: impl Fn(f32) -> T,
    ) {
        output.fill(convert(0.0));
        if self.state == State::Failed {
            return;
        }
        if self.fault.load(Ordering::SeqCst)
            || timing.playback < timing.callback
            || self.previous.is_some_and(|(callback, playback)| {
                timing.callback <= callback || timing.playback < playback
            })
            || !output
                .len()
                .is_multiple_of(usize::from(self.format.channels))
        {
            self.fail();
            self.publish();
            return;
        }
        self.previous = Some((timing.callback, timing.playback));
        while self
            .pending
            .front()
            .is_some_and(|buffer| buffer.deadline <= timing.callback)
        {
            if let Some(buffer) = self.pending.pop_front() {
                self.confirmed = buffer.end;
            }
        }
        let length = u64::try_from(self.source.len()).unwrap_or(u64::MAX);
        if self.cursor / u64::from(self.format.rate) >= length && self.confirmed == length {
            self.state = State::Finished;
        }
        self.controls();
        if self.state == State::Playing {
            let before = self.cursor;
            for frame in output.chunks_exact_mut(usize::from(self.format.channels)) {
                let index = self.cursor / u64::from(self.format.rate);
                if index >= length {
                    break;
                }
                let index = usize::try_from(index).unwrap_or(self.source.len());
                let first = self.source[index];
                let second = self.source.get(index + 1).copied().unwrap_or(first);
                let fraction =
                    u32::try_from(self.cursor % u64::from(self.format.rate)).unwrap_or(0);
                let fraction = (f64::from(fraction) / f64::from(self.format.rate))
                    .to_f32()
                    .unwrap_or(0.0);
                frame.fill(convert(first + (second - first) * fraction));
                self.cursor = (self.cursor + SOURCE_RATE).min(length * u64::from(self.format.rate));
            }
            if self.cursor != before {
                let frames = u64::try_from(output.len() / usize::from(self.format.channels))
                    .unwrap_or(u64::MAX);
                let duration = frames
                    .saturating_mul(1_000_000_000)
                    .div_ceil(u64::from(self.format.rate));
                if let Some(deadline) = timing.playback.checked_add(duration) {
                    if self.pending.len() < PENDING_BUFFERS {
                        self.pending.push_back(Pending {
                            deadline,
                            end: self.cursor / u64::from(self.format.rate),
                        });
                    } else {
                        self.fail();
                    }
                } else {
                    self.fail();
                }
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests;
