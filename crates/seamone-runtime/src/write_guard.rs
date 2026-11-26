//! Connection-local authorization for label and queue writes, independent of SQL spelling.

use crate::playback_evidence::AuthorizedReview;
use rusqlite::{
    Connection,
    hooks::{AuthAction, AuthContext, Authorization},
};
use seamone_core::review::ReviewSegment;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

pub(super) struct WriteAccess(Arc<AtomicU8>);

pub(super) struct WritePermit<'a> {
    connection: &'a Connection,
    state: Arc<AtomicU8>,
}

fn install(connection: &Connection, state: Arc<AtomicU8>) -> Result<(), String> {
    connection
        .authorizer(Some(move |context: AuthContext<'_>| {
            let permission = state.load(Ordering::Acquire);
            match context.action {
                AuthAction::CreateTrigger { .. }
                | AuthAction::CreateTempTrigger { .. }
                | AuthAction::CreateView { .. }
                | AuthAction::CreateTempView { .. } => Authorization::Deny,
                AuthAction::Insert {
                    table_name: "segments",
                }
                | AuthAction::Update {
                    table_name: "segments",
                    column_name: "body",
                } if permission == 0 || context.accessor.is_some() => Authorization::Deny,
                AuthAction::Insert {
                    table_name: "training_queue",
                } if permission != 2 || context.accessor.is_some() => Authorization::Deny,
                _ => Authorization::Allow,
            }
        }))
        .map_err(|error| error.to_string())
}

impl WriteAccess {
    pub(super) fn attach(connection: &Connection) -> Result<Self, String> {
        let state = Arc::new(AtomicU8::new(0));
        install(connection, Arc::clone(&state))?;
        Ok(Self(state))
    }

    fn enter<'a>(
        &self,
        connection: &'a Connection,
        permission: u8,
    ) -> Result<WritePermit<'a>, String> {
        self.0
            .compare_exchange(0, permission, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "저장 작업의 쓰기 권한이 겹쳤습니다.".to_string())?;
        if let Err(error) = install(connection, Arc::clone(&self.0)) {
            self.0.store(0, Ordering::Release);
            return Err(error);
        }
        Ok(WritePermit {
            connection,
            state: Arc::clone(&self.0),
        })
    }

    pub(super) fn review<'a>(
        &self,
        connection: &'a Connection,
        authorization: AuthorizedReview<'_>,
        current: &ReviewSegment,
        next: &ReviewSegment,
    ) -> Result<WritePermit<'a>, String> {
        authorization.admit(current, next)?;
        self.enter(connection, 2)
    }

    pub(super) fn unverified<'a>(
        &self,
        connection: &'a Connection,
        segments: &[ReviewSegment],
    ) -> Result<WritePermit<'a>, String> {
        if segments.iter().any(|segment| segment.verified.is_some()) {
            return Err("미검증 쓰기에 확인 전사가 포함됐습니다.".into());
        }
        self.enter(connection, 1)
    }
}

impl Drop for WritePermit<'_> {
    fn drop(&mut self) {
        self.state.store(0, Ordering::Release);
        // Resetting the callback expires prepared statements too, so a statement cannot outlive its permit.
        let _ = install(self.connection, Arc::clone(&self.state));
    }
}
