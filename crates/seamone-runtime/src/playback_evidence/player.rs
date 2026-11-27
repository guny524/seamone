//! One service owns one current output session and its immutable review scope.

use super::output::Stream;
use seamone_core::{
    playback::{Action, Status},
    review::Request,
};
use std::sync::Arc;
use uuid::Uuid;

struct Scope {
    segment: String,
    revision: u64,
}

struct Session {
    id: String,
    scope: Option<Scope>,
    output: Stream,
}

enum SessionState {
    Idle,
    Loaded(Session),
    Failed(String),
}

#[cfg(test)]
type OutputFactory = Box<dyn Fn(Arc<[f32]>, u64) -> Result<Stream, String> + Send>;

enum Device {
    Platform,
    #[cfg(any(test, feature = "test-support"))]
    Fake,
    #[cfg(test)]
    Testing(OutputFactory),
}

pub(super) struct Player {
    session: SessionState,
    device: Device,
}

impl Player {
    pub(super) fn new() -> Self {
        Self {
            session: SessionState::Idle,
            device: Device::Platform,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn fake() -> Self {
        Self {
            session: SessionState::Idle,
            device: Device::Fake,
        }
    }

    #[cfg(test)]
    pub(super) fn with_output(output: OutputFactory) -> Self {
        Self {
            session: SessionState::Idle,
            device: Device::Testing(output),
        }
    }

    pub(super) fn load(
        &mut self,
        samples: Vec<f32>,
        origin: u64,
        scope: Option<(String, u64)>,
    ) -> Result<String, String> {
        self.session = SessionState::Idle;
        let source: Arc<[f32]> = samples.into();
        let output = match &self.device {
            Device::Platform => Stream::create(source, origin),
            #[cfg(any(test, feature = "test-support"))]
            Device::Fake => Stream::fake(source, origin),
            #[cfg(test)]
            Device::Testing(create) => create(source, origin),
        };
        let output = match output {
            Ok(output) => output,
            Err(error) => {
                self.session = SessionState::Failed(error.clone());
                return Err(error);
            }
        };
        let id = Uuid::new_v4().to_string();
        self.session = SessionState::Loaded(Session {
            id: id.clone(),
            scope: scope.map(|(segment, revision)| Scope { segment, revision }),
            output,
        });
        Ok(id)
    }

    pub(super) fn control(&mut self, id: &str, action: Action) -> Result<Status, String> {
        let SessionState::Loaded(session) = &mut self.session else {
            return Err("재생이 바뀌었습니다. 현재 구간을 다시 여세요.".into());
        };
        if session.id != id {
            return Err("재생이 바뀌었습니다. 현재 구간을 다시 여세요.".into());
        }
        session.output.control(action)?;
        Ok(self.status())
    }

    pub(super) fn status(&mut self) -> Status {
        let session = match &mut self.session {
            SessionState::Idle => return Status::default(),
            SessionState::Failed(error) => {
                return Status {
                    state: seamone_core::playback::State::Failed,
                    error: Some(error.clone()),
                    ..Status::default()
                };
            }
            SessionState::Loaded(session) => session,
        };
        let observed = session.output.observation();
        Status {
            session: Some(session.id.clone()),
            state: observed.state,
            position: observed.position,
            heard: observed.heard,
            error: observed.error,
        }
    }

    pub(super) fn authorizes(&mut self, request: &Request, id: Option<&str>) -> bool {
        let SessionState::Loaded(session) = &mut self.session else {
            return false;
        };
        id == Some(session.id.as_str())
            && session.scope.as_ref().is_some_and(|scope| {
                scope.segment == request.segment_id && scope.revision == request.expected_revision
            })
            && session.output.observation().heard
    }

    pub(super) fn close(&mut self) {
        self.session = SessionState::Idle;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn advance(&mut self, frames: usize) -> Result<(), String> {
        let SessionState::Loaded(session) = &mut self.session else {
            return Err("진행할 재생 session이 없습니다.".into());
        };
        session.output.advance(frames)
    }

    #[cfg(test)]
    pub(super) fn finish(&mut self) -> Result<(), String> {
        let SessionState::Loaded(session) = &mut self.session else {
            return Err("진행할 재생 session이 없습니다.".into());
        };
        session.output.finish()
    }
}
