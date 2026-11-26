//! The private permit owns fixed SQL and serializes only the values it validates.

use crate::playback_evidence::AuthorizedReview;
use rusqlite::{
    Connection, Transaction,
    hooks::{AuthAction, AuthContext, Authorization},
    params,
};
use seamone_core::review::{ReviewError, ReviewSegment};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

pub(super) struct WriteAccess(Arc<AtomicU8>);

struct WritePermit<'a> {
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
                | AuthAction::CreateTempView { .. }
                | AuthAction::Update {
                    table_name: "training_queue",
                    column_name: "segment_id" | "revision" | "recording_revision",
                } => Authorization::Deny,
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

    pub(super) fn insert_unverified(
        &self,
        connection: &Transaction<'_>,
        segments: &[ReviewSegment],
    ) -> Result<(), String> {
        WritePermit::insert_unverified(self, connection, segments)
    }

    pub(super) fn clear_verification(
        &self,
        connection: &Transaction<'_>,
        segments: Vec<ReviewSegment>,
    ) -> Result<(), String> {
        WritePermit::clear_verification(self, connection, segments)
    }

    pub(super) fn review(
        &self,
        connection: &Transaction<'_>,
        authorization: AuthorizedReview<'_>,
        next: &ReviewSegment,
    ) -> Result<(), String> {
        WritePermit::review(self, connection, authorization, next)
    }
}

impl WritePermit<'_> {
    fn enter<'a>(
        access: &WriteAccess,
        connection: &'a Connection,
        permission: u8,
    ) -> Result<WritePermit<'a>, String> {
        access
            .0
            .compare_exchange(0, permission, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "저장 작업의 쓰기 권한이 겹쳤습니다.".to_string())?;
        if let Err(error) = install(connection, Arc::clone(&access.0)) {
            access.0.store(0, Ordering::Release);
            return Err(error);
        }
        Ok(WritePermit {
            connection,
            state: Arc::clone(&access.0),
        })
    }

    fn insert_unverified(
        access: &WriteAccess,
        connection: &Transaction<'_>,
        segments: &[ReviewSegment],
    ) -> Result<(), String> {
        if segments.iter().any(|segment| segment.verified.is_some()) {
            return Err("미검증 쓰기에 확인 전사가 포함됐습니다.".into());
        }
        let permit = Self::enter(access, connection, 1)?;
        for segment in segments {
            let body = serde_json::to_string(segment).map_err(|error| error.to_string())?;
            permit
                .connection
                .execute(
                    "INSERT INTO segments(id,recording_id,revision,body) VALUES (?1,?2,?3,?4)",
                    params![segment.id, segment.recording_id, segment.revision, body],
                )
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn clear_verification(
        access: &WriteAccess,
        connection: &Transaction<'_>,
        segments: Vec<ReviewSegment>,
    ) -> Result<(), String> {
        let permit = Self::enter(access, connection, 1)?;
        for mut segment in segments {
            segment.verified = None;
            segment.revision = segment
                .revision
                .checked_add(1)
                .ok_or_else(|| ReviewError::StaleRevision.to_string())?;
            segment.error_marked_text = Some(segment.text.clone());
            let body = serde_json::to_string(&segment).map_err(|error| error.to_string())?;
            permit
                .connection
                .execute(
                    "UPDATE segments SET revision=?1,body=?2 WHERE id=?3",
                    params![segment.revision, body, segment.id],
                )
                .map_err(|error| error.to_string())?;
            permit
                .connection
                .execute(
                    "DELETE FROM training_queue WHERE segment_id=?1",
                    [&segment.id],
                )
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn review(
        access: &WriteAccess,
        connection: &Transaction<'_>,
        authorization: AuthorizedReview<'_>,
        next: &ReviewSegment,
    ) -> Result<(), String> {
        let request = authorization.request();
        let persisted: String = connection
            .query_row(
                "SELECT body FROM segments WHERE id=?1",
                [&request.segment_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let current: ReviewSegment =
            serde_json::from_str(&persisted).map_err(|error| error.to_string())?;
        let samples: u64 = connection
            .query_row(
                "SELECT samples FROM recordings WHERE id=?1",
                [&current.recording_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let expected = current
            .apply(request, samples)
            .map_err(|error| error.to_string())?;
        if next != &expected {
            return Err("검수 전이와 저장할 전사가 다릅니다.".into());
        }
        authorization.admit(&current, &expected)?;
        if next == &current {
            return Ok(());
        }
        let body = serde_json::to_string(next).map_err(|error| error.to_string())?;
        let permit = Self::enter(access, connection, 2)?;
        let changed = permit
            .connection
            .execute(
                "UPDATE segments SET revision=?1,body=?2 WHERE id=?3 AND revision=?4",
                params![next.revision, body, next.id, current.revision],
            )
            .map_err(|error| error.to_string())?;
        if changed != 1 {
            return Err(ReviewError::StaleRevision.to_string());
        }
        permit
            .connection
            .execute("DELETE FROM training_queue WHERE segment_id=?1", [&next.id])
            .map_err(|error| error.to_string())?;
        if next.verified.is_some() {
            permit.connection.execute(
                "INSERT INTO training_queue(segment_id,revision,recording_revision,status) VALUES (?1,?2,?3,'pending')",
                params![next.id,next.revision,next.recording_revision],
            ).map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

impl Drop for WritePermit<'_> {
    fn drop(&mut self) {
        self.state.store(0, Ordering::Release);
        // Expire prepared statements so a statement cannot outlive its private permit.
        let _ = install(self.connection, Arc::clone(&self.state));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_cannot_retain_authority_after_the_permit_returns() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE segments(id TEXT,body TEXT)")
            .unwrap();
        let access = WriteAccess::attach(&connection).unwrap();
        let permit = WritePermit::enter(&access, &connection, 1).unwrap();
        let mut prepared = connection.prepare("UPDATE segments SET body=body").unwrap();
        drop(permit);
        assert!(prepared.execute([]).is_err());
    }
}
