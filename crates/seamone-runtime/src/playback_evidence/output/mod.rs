//! Only device output can produce playback ranges used for review authority.

use seamone_core::playback::{Action, State};
use std::sync::Arc;

#[cfg(any(
    test,
    feature = "test-support",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
mod clock;
#[cfg(any(
    test,
    feature = "test-support",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
mod engine;
#[cfg(any(test, feature = "test-support"))]
mod fake;
#[cfg(any(
    test,
    feature = "test-support",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
mod heard;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod native;
#[cfg(any(
    test,
    feature = "test-support",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
mod renderer;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
const UNAVAILABLE: &str = "이 기기에서는 아직 재생을 지원하지 않습니다";
#[cfg(any(
    test,
    feature = "test-support",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
const OUTPUT_ERROR: &str =
    "출력 장치나 장치 시계에 오류가 생겼습니다. 음성을 처음부터 다시 여세요.";

pub(super) struct Observation {
    pub(super) state: State,
    pub(super) position: u64,
    pub(super) heard: bool,
    pub(super) error: Option<String>,
}

trait RenderedOutput: Send {
    fn control(&mut self, action: Action) -> Result<(), String>;
    fn observation(&mut self) -> Observation;
    #[cfg(any(test, feature = "test-support"))]
    fn advance(&mut self, frames: usize) -> Result<(), String>;
    #[cfg(test)]
    fn finish(&mut self) -> Result<(), String>;
}

trait Output {
    fn create_stream(&self, source: Arc<[f32]>, origin: u64) -> Result<Stream, String>;
}

struct Platform;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
impl Output for Platform {
    fn create_stream(&self, source: Arc<[f32]>, origin: u64) -> Result<Stream, String> {
        engine::create(source, origin, Box::new(native::Device))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl Output for Platform {
    fn create_stream(&self, _source: Arc<[f32]>, _origin: u64) -> Result<Stream, String> {
        Err(UNAVAILABLE.into())
    }
}

pub(super) struct Stream {
    inner: Box<dyn RenderedOutput>,
}

impl Stream {
    pub(super) fn create(source: Arc<[f32]>, origin: u64) -> Result<Self, String> {
        Platform.create_stream(source, origin)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn fake(source: Arc<[f32]>, origin: u64) -> Result<Self, String> {
        engine::create(source, origin, Box::new(fake::Device))
    }

    pub(super) fn control(&mut self, action: Action) -> Result<(), String> {
        self.inner.control(action)
    }

    pub(super) fn observation(&mut self) -> Observation {
        self.inner.observation()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn advance(&mut self, frames: usize) -> Result<(), String> {
        self.inner.advance(frames)
    }

    #[cfg(test)]
    pub(super) fn finish(&mut self) -> Result<(), String> {
        self.inner.finish()
    }
}
