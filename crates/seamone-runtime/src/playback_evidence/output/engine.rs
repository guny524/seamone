//! Owns an output stream and folds only callback-confirmed runs into listening coverage.

use super::{OUTPUT_ERROR, Observation, RenderedOutput, Stream, heard::Heard, renderer::Reader};
use rtrb::Producer;
use seamone_core::playback::{Action, State};
use std::sync::Arc;

pub(super) trait Driver: Send {
    fn stop(&mut self);
    #[cfg(any(test, feature = "test-support"))]
    fn advance(&mut self, _frames: usize) -> Result<(), String> {
        Err("실제 출력 장치는 시험 시계로 진행할 수 없습니다.".into())
    }
}

pub(super) trait Device: Send {
    fn start(&self, source: Arc<[f32]>, at: u64) -> Result<Running, String>;
}

pub(super) struct Running {
    pub(super) commands: Producer<Action>,
    pub(super) reader: Reader,
    pub(super) driver: Box<dyn Driver>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.driver.stop();
    }
}

struct OutputStream {
    source: Arc<[f32]>,
    origin: u64,
    heard: Heard,
    state: State,
    position: u64,
    running: Option<Running>,
    device: Box<dyn Device>,
    broken: bool,
}

pub(super) fn create(
    source: Arc<[f32]>,
    origin: u64,
    device: Box<dyn Device>,
) -> Result<Stream, String> {
    let len = u64::try_from(source.len()).map_err(|e| e.to_string())?;
    let running = device.start(Arc::clone(&source), 0)?;
    Ok(Stream {
        inner: Box::new(OutputStream {
            source,
            origin,
            heard: Heard::new(len),
            state: State::Ready,
            position: 0,
            running: Some(running),
            device,
            broken: false,
        }),
    })
}

impl OutputStream {
    fn collect(&mut self) {
        if let Some(running) = &mut self.running {
            if running.reader.broken() {
                self.broken = true;
            }
            while let Some(run) = running.reader.closed() {
                self.heard.add(run);
            }
            if let Some(snapshot) = running.reader.snapshot() {
                self.state = snapshot.state;
                self.position = snapshot.position;
                self.heard.add(snapshot.run);
                self.broken |= snapshot.state == State::Failed;
            }
            self.broken |= running.reader.broken();
        }
        if self.broken {
            self.state = State::Failed;
            self.heard.invalidate();
        }
    }

    fn stop(&mut self) {
        if let Some(running) = &mut self.running {
            running.driver.stop();
        }
        self.collect();
        self.running = None;
        if !self.broken {
            self.state = State::Paused;
        }
    }
}

impl RenderedOutput for OutputStream {
    fn control(&mut self, action: Action) -> Result<(), String> {
        self.collect();
        if action == Action::Stop {
            self.stop();
            return Ok(());
        }
        if self.broken {
            return Err(OUTPUT_ERROR.into());
        }
        let action = match action {
            Action::Seek { at } => {
                let relative = at
                    .checked_sub(self.origin)
                    .filter(|at| *at <= u64::try_from(self.source.len()).unwrap_or(0))
                    .ok_or("재생 위치가 현재 구간 밖입니다.")?;
                Action::Seek { at: relative }
            }
            other => other,
        };
        if self.running.is_none() {
            if let Action::Seek { at } = action {
                self.position = at;
                return Ok(());
            }
            if action == Action::Pause {
                return Ok(());
            }
            if self.position == u64::try_from(self.source.len()).unwrap_or(0) {
                self.position = 0;
            }
            match self.device.start(Arc::clone(&self.source), self.position) {
                Ok(running) => self.running = Some(running),
                Err(error) => {
                    self.broken = true;
                    self.state = State::Failed;
                    self.heard.invalidate();
                    return Err(error);
                }
            }
        }
        self.running
            .as_mut()
            .ok_or("재생 장치가 없습니다.")?
            .commands
            .push(action)
            .map_err(|_| "재생 조작이 밀렸습니다. 잠시 뒤 다시 조작하세요.".to_string())
    }

    fn observation(&mut self) -> Observation {
        self.collect();
        Observation {
            state: self.state,
            position: self.origin + self.position,
            heard: self.heard.complete(),
            error: self.broken.then(|| OUTPUT_ERROR.into()),
        }
    }

    #[cfg(test)]
    fn finish(&mut self) -> Result<(), String> {
        self.advance(self.source.len() * 3 + 4_800)
    }

    #[cfg(any(test, feature = "test-support"))]
    fn advance(&mut self, frames: usize) -> Result<(), String> {
        self.running
            .as_mut()
            .ok_or("멈춘 재생은 진행할 수 없습니다.")?
            .driver
            .advance(frames)?;
        self.collect();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> OutputStream {
        let source: Arc<[f32]> = vec![0.25; 16_000].into();
        let device = Box::new(super::super::fake::Device);
        let running = device.start(Arc::clone(&source), 0).unwrap();
        OutputStream {
            source,
            origin: 32_000,
            heard: Heard::new(16_000),
            state: State::Ready,
            position: 0,
            running: Some(running),
            device,
            broken: false,
        }
    }

    #[test]
    fn device_error_after_completion_revokes_coverage_without_another_callback() {
        let mut stream = fixture();
        stream.control(Action::Play).unwrap();
        stream.finish().unwrap();
        assert!(stream.observation().heard);
        stream.running.as_ref().unwrap().reader.inject_error();
        let failed = stream.observation();
        assert_eq!(failed.state, State::Failed);
        assert!(!failed.heard);
        assert!(failed.error.is_some());
        assert!(stream.control(Action::Play).is_err());
        stream.control(Action::Stop).unwrap();
        assert!(!stream.observation().heard);
    }

    struct Disconnected;

    impl Device for Disconnected {
        fn start(&self, _source: Arc<[f32]>, _at: u64) -> Result<Running, String> {
            Err("device disconnected".into())
        }
    }

    #[test]
    fn failing_to_reopen_the_device_invalidates_the_entire_session() {
        let mut stream = fixture();
        stream.control(Action::Play).unwrap();
        stream.finish().unwrap();
        stream.control(Action::Stop).unwrap();
        assert!(stream.observation().heard);
        stream.device = Box::new(Disconnected);
        assert!(stream.control(Action::Play).is_err());
        let failed = stream.observation();
        assert_eq!(failed.state, State::Failed);
        assert!(!failed.heard);
    }

    #[test]
    fn stopping_closes_device_and_resume_keeps_only_confirmed_coverage() {
        let mut stream = fixture();
        assert!(stream.control(Action::Seek { at: 31_999 }).is_err());
        assert!(stream.control(Action::Seek { at: 48_001 }).is_err());
        stream.control(Action::Play).unwrap();
        stream.advance(24_000).unwrap();
        let before = stream.observation();
        assert!(before.position > 32_000 && before.position < 48_000);
        assert!(!before.heard);
        stream.control(Action::Stop).unwrap();
        assert!(stream.running.is_none());
        assert_eq!(stream.observation().position, before.position);
        assert!(stream.advance(1_000).is_err());
        stream.control(Action::Play).unwrap();
        stream.finish().unwrap();
        let end = stream.observation();
        assert_eq!(end.position, 48_000);
        assert!(end.heard);
    }
}
