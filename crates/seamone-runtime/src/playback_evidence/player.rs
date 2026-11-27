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

enum Device {
    Platform,
    #[cfg(any(test, feature = "test-support"))]
    Fake,
}

pub(super) struct Player {
    session: Option<Session>,
    device: Device,
}

impl Player {
    pub(super) fn new() -> Self {
        Self {
            session: None,
            device: Device::Platform,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn fake() -> Self {
        Self {
            session: None,
            device: Device::Fake,
        }
    }

    pub(super) fn load(
        &mut self,
        samples: Vec<f32>,
        origin: u64,
        scope: Option<(String, u64)>,
    ) -> Result<String, String> {
        self.session = None;
        let source: Arc<[f32]> = samples.into();
        let output = match self.device {
            Device::Platform => Stream::create(source, origin)?,
            #[cfg(any(test, feature = "test-support"))]
            Device::Fake => Stream::fake(source, origin)?,
        };
        let id = Uuid::new_v4().to_string();
        self.session = Some(Session {
            id: id.clone(),
            scope: scope.map(|(segment, revision)| Scope { segment, revision }),
            output,
        });
        Ok(id)
    }

    pub(super) fn control(&mut self, id: &str, action: Action) -> Result<Status, String> {
        let session = self
            .session
            .as_mut()
            .filter(|session| session.id == id)
            .ok_or("재생이 바뀌었습니다. 현재 구간을 다시 여세요.")?;
        session.output.control(action)?;
        Ok(self.status())
    }

    pub(super) fn status(&mut self) -> Status {
        let Some(session) = &mut self.session else {
            return Status::default();
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
        let Some(session) = &mut self.session else {
            return false;
        };
        id == Some(session.id.as_str())
            && session.scope.as_ref().is_some_and(|scope| {
                scope.segment == request.segment_id && scope.revision == request.expected_revision
            })
            && session.output.observation().heard
    }

    pub(super) fn close(&mut self) {
        self.session = None;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn advance(&mut self, frames: usize) -> Result<(), String> {
        self.session
            .as_mut()
            .ok_or("진행할 재생 session이 없습니다.")?
            .output
            .advance(frames)
    }

    #[cfg(test)]
    pub(super) fn finish(&mut self) -> Result<(), String> {
        self.session
            .as_mut()
            .ok_or("진행할 재생 session이 없습니다.")?
            .output
            .finish()
    }
}
