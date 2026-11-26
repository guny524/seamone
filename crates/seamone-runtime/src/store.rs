//! A single versioned database owns edits, verification snapshots and the durable work queue.
use crate::audio::{AudioLibrary, StagedAudio};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
#[cfg(any(test, feature = "test-support"))]
use seamone_asr::Transcriber;
use seamone_core::error_marks::{ErrorMark, ErrorMarkBatch, ErrorMarkPage, overlaps};
use seamone_core::review::{
    EligibleExample, LibraryPage, RecordingInfo, ReviewError, ReviewEvidence, ReviewRequest,
    ReviewSegment, SampleRange,
};
use std::{fs, path::Path, time::Duration};
use uuid::Uuid;

pub struct Store {
    db: Connection,
    owner: String,
    pub audio: AudioLibrary,
}

impl Store {
    pub fn open(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let mut db = Connection::open(root.join("seamone.sqlite3")).map_err(|e| e.to_string())?;
        db.busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        db.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )
        .map_err(|e| e.to_string())?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let version: u32 = tx
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if version > 4 {
            return Err("이 앱보다 새 version의 보관함입니다. 앱을 업데이트하세요.".into());
        }
        if version == 0 {
            tx.execute_batch(include_str!("schema.sql"))
                .map_err(|e| e.to_string())?;
        }
        if version < 2 {
            tx.execute_batch(include_str!("schema-v2.sql"))
                .map_err(|e| e.to_string())?;
        }
        if version < 3 {
            tx.execute_batch(include_str!("schema-v3.sql"))
                .map_err(|e| e.to_string())?;
        }
        if version < 4 {
            tx.execute_batch(include_str!("schema-v4.sql"))
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        // Recovery is by expiring a claim lease, never by merely opening another connection.
        Ok(Self {
            db,
            owner: Uuid::new_v4().to_string(),
            audio: AudioLibrary::new(&root.join("audio"))?,
        })
    }

    pub fn library(&self, inference_available: bool) -> Result<LibraryPage, String> {
        let mut stmt = self
            .db
            .prepare("SELECT info FROM recordings ORDER BY rowid DESC LIMIT 200")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let recordings = rows
            .map(|r| decode(&r.map_err(|e| e.to_string())?))
            .collect::<Result<_, _>>()?;
        let pending_training = self
            .db
            .query_row(
                "SELECT count(*) FROM training_queue WHERE status IN ('pending','retry','running')",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let retry_training = self
            .db
            .query_row(
                "SELECT count(*) FROM training_queue WHERE status='retry'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(LibraryPage {
            retry_training,
            recordings,
            info_seen: self.setting("info_seen")?.is_some(),
            pending_training,
            inference_available,
        })
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>, String> {
        self.db
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.db.execute("INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value]).map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(test)]
    pub fn import(&mut self, source: &Path) -> Result<RecordingInfo, String> {
        let input = self.audio.open_source(source)?;
        let mut audio = crate::audio_ingest::decode(&self.audio, input)?;
        self.save_audio(
            crate::audio_ingest::display_name(source),
            &mut audio,
            || true,
        )
    }

    pub(crate) fn save_audio(
        &self,
        name: String,
        audio: &mut StagedAudio,
        can_commit: impl Fn() -> bool,
    ) -> Result<RecordingInfo, String> {
        let info = RecordingInfo {
            id: Uuid::new_v4().to_string(),
            name,
            samples: audio.samples,
            revision: 1,
            engine: "아직 받아쓰지 않음".into(),
            model: String::new(),
        };
        let encoded = encode(&info)?;
        let hash = self.audio.publish(&info.id, audio)?;
        let result = if can_commit() {
            self.db.execute(
                    "INSERT INTO recordings(id,revision,samples,sha256,info) VALUES (?1,?2,?3,?4,?5)",
                    params![info.id, info.revision, info.samples, hash, encoded],
                )
                .map_err(|e| e.to_string())
        } else {
            Err("창이 닫혀 음성을 저장하지 않았습니다.".into())
        };
        if let Err(error) = result {
            let _ = fs::remove_file(self.audio.path(&info.id)?);
            return Err(error);
        }
        Ok(info)
    }

    pub fn recording(&self, id: &str) -> Result<RecordingInfo, String> {
        let text: String = self
            .db
            .query_row("SELECT info FROM recordings WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        decode(&text)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn transcribe(
        &mut self,
        id: &str,
        engine: &mut dyn Transcriber,
        engine_name: &str,
        model: &str,
    ) -> Result<Vec<ReviewSegment>, String> {
        let (info, audio) = self.prepare_transcription(id)?;
        let segments = engine
            .transcribe(&audio, Some("ko"))
            .map_err(|e| e.to_string())?;
        self.commit_transcription(&info, segments, engine_name, model)
    }

    #[cfg(any(
        test,
        feature = "test-support",
        all(
            feature = "desktop-whisper",
            not(any(target_os = "android", target_os = "ios"))
        )
    ))]
    pub fn prepare_transcription(&self, id: &str) -> Result<(RecordingInfo, Vec<f32>), String> {
        let info = self.recording(id)?;
        let state: String = self
            .db
            .query_row(
                "SELECT transcription_state FROM recordings WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if state != "pending" {
            return Err("이미 받아쓴 녹음입니다. 기존 수정본을 그대로 사용하세요.".into());
        }
        Ok((info, self.audio.read_all(id)?))
    }

    #[cfg(any(
        test,
        feature = "test-support",
        all(
            feature = "desktop-whisper",
            not(any(target_os = "android", target_os = "ios"))
        )
    ))]
    pub fn commit_transcription(
        &mut self,
        info: &RecordingInfo,
        segments: Vec<seamone_core::TranscriptSegment>,
        engine: &str,
        model: &str,
    ) -> Result<Vec<ReviewSegment>, String> {
        if segments.len() > 10_000
            || segments.iter().map(|s| s.text.len()).sum::<usize>()
                > seamone_core::review::MAX_RECORDING_TEXT_BYTES
        {
            return Err("전사문 크기가 허용 범위를 넘었습니다.".into());
        }
        let mut previous_start = 0;
        let segments = segments
            .into_iter()
            .map(|s| {
                if s.text.len() > seamone_core::review::MAX_TEXT_BYTES
                    || s.start_ms > s.end_ms
                    || s.start_ms < previous_start
                {
                    return Err("받아쓰기 엔진이 잘못된 조각을 돌려줬습니다.".into());
                }
                previous_start = s.start_ms;
                let range = SampleRange::from_millis(s.start_ms, s.end_ms, info.samples)
                    .map_err(|e| e.to_string())?;
                let machine_timing = s
                    .timing
                    .filter(|timing| seamone_core::valid_alignment(timing, &s.text, range));
                Ok(ReviewSegment {
                    id: Uuid::new_v4().to_string(),
                    recording_id: info.id.clone(),
                    recording_revision: info.revision,
                    revision: 1,
                    range,
                    machine_timing,
                    machine_text: s.text.clone(),
                    text: s.text,
                    verdict: None,
                    needs_correction: false,
                    error_marked_text: None,
                    verified: None,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let encoded = segments.iter().map(encode).collect::<Result<Vec<_>, _>>()?;
        let mut updated = info.clone();
        updated.engine = engine.into();
        updated.model = model.into();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let changed = tx.execute("UPDATE recordings SET transcription_state='complete',info=?1 WHERE id=?2 AND revision=?3 AND transcription_state='pending'",params![encode(&updated)?,info.id,info.revision]).map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err(
                "다른 요청이 이 녹음을 이미 받아썼습니다. 저장한 글을 새로 불러오세요.".into(),
            );
        }
        for (s, body) in segments.iter().zip(encoded) {
            tx.execute(
                "INSERT INTO segments(id,recording_id,revision,body) VALUES (?1,?2,?3,?4)",
                params![s.id, info.id, s.revision, body],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(segments)
    }

    pub fn segments(&self, recording: &str) -> Result<Vec<ReviewSegment>, String> {
        let mut stmt = self
            .db
            .prepare("SELECT body FROM segments WHERE recording_id=?1 ORDER BY rowid LIMIT 10000")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([recording], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| decode(&r.map_err(|e| e.to_string())?))
            .collect()
    }

    pub fn segment_page(
        &self,
        recording: &str,
        offset: u32,
    ) -> Result<seamone_core::review::SegmentPage, String> {
        if offset > 10_000 {
            return Err("목록 위치가 잘못됐습니다.".into());
        }
        let mut stmt = self
            .db
            .prepare(
                "SELECT body FROM segments WHERE recording_id=?1 ORDER BY rowid LIMIT 33 OFFSET ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![recording, offset], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut segments = rows
            .map(|r| decode(&r.map_err(|e| e.to_string())?))
            .collect::<Result<Vec<ReviewSegment>, String>>()?;
        let next_offset = if segments.len() > 32 {
            segments.pop();
            Some(offset + 32)
        } else {
            None
        };
        Ok(seamone_core::review::SegmentPage {
            segments,
            next_offset,
        })
    }

    pub fn error_mark_page(&self, recording: &str, offset: u32) -> Result<ErrorMarkPage, String> {
        let mut stmt = self
            .db
            .prepare(
                "SELECT id,recording_id,recording_revision,start_sample,end_sample
                FROM error_marks WHERE recording_id=?1 ORDER BY rowid LIMIT 33 OFFSET ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![recording, offset], read_error_mark)
            .map_err(|e| e.to_string())?;
        let mut marks = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let next_offset = if marks.len() > 32 {
            marks.pop();
            offset.checked_add(32)
        } else {
            None
        };
        Ok(ErrorMarkPage { marks, next_offset })
    }

    pub fn save_error_mark(&mut self, mark: &ErrorMark) -> Result<(), String> {
        if mark.id.is_empty() || mark.id.len() > 128 {
            return Err("요청 ID가 잘못됐습니다.".into());
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let existing = tx
            .query_row(
                "SELECT id,recording_id,recording_revision,start_sample,end_sample
                FROM error_marks WHERE id=?1",
                [&mark.id],
                read_error_mark,
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(existing) = existing {
            return if existing == *mark {
                Ok(())
            } else {
                Err("같은 요청 ID에 다른 내용이 들어왔습니다.".into())
            };
        }
        validate_error_ranges(
            &tx,
            &mark.recording_id,
            mark.recording_revision,
            &[mark.range],
        )?;
        flag_overlapping_segments(
            &tx,
            &mark.recording_id,
            mark.recording_revision,
            &[mark.range],
        )?;
        insert_error_mark(&tx, mark)?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn save_error_marks(&mut self, batch: &ErrorMarkBatch) -> Result<(), String> {
        if batch.id.is_empty() || batch.id.len() > 128 || !(1..=256).contains(&batch.ranges.len()) {
            return Err("오류 구간 요청이 잘못됐습니다.".into());
        }
        let mut request = batch.clone();
        request
            .ranges
            .sort_unstable_by_key(|range| (range.start, range.end));
        request.ranges.dedup();
        let encoded = encode(&request)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT request FROM error_mark_batches WHERE id=?1",
                [&batch.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(previous) = previous {
            return if previous == encoded {
                Ok(())
            } else {
                Err("같은 요청 ID에 다른 내용이 들어왔습니다.".into())
            };
        }
        validate_error_ranges(
            &tx,
            &batch.recording_id,
            batch.recording_revision,
            &request.ranges,
        )?;
        let covered = {
            let mut query = tx.prepare(
                "SELECT start_sample,end_sample FROM error_marks
                 WHERE recording_id=?1 AND recording_revision=?2 AND end_sample>?3 AND start_sample<?4
                 ORDER BY start_sample,end_sample",
            ).map_err(|e| e.to_string())?;
            let end = request
                .ranges
                .iter()
                .map(|range| range.end)
                .max()
                .unwrap_or_default();
            let rows = query
                .query_map(
                    params![
                        batch.recording_id,
                        batch.recording_revision,
                        request.ranges[0].start,
                        end
                    ],
                    |row| {
                        Ok(SampleRange {
                            start: row.get(0)?,
                            end: row.get(1)?,
                        })
                    },
                )
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };
        let ranges = uncovered_ranges(&request.ranges, &covered);
        flag_overlapping_segments(
            &tx,
            &batch.recording_id,
            batch.recording_revision,
            &request.ranges,
        )?;
        for range in ranges {
            insert_error_mark(
                &tx,
                &ErrorMark {
                    id: Uuid::new_v4().to_string(),
                    recording_id: batch.recording_id.clone(),
                    recording_revision: batch.recording_revision,
                    range,
                },
            )?;
        }
        tx.execute(
            "INSERT INTO error_mark_batches(id,request) VALUES (?1,?2)",
            params![batch.id, encoded],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn replay(&self, request: &ReviewRequest) -> Result<Option<ReviewSegment>, String> {
        let replay: Option<(String, String)> = self
            .db
            .query_row(
                "SELECT request,response FROM review_events WHERE id=?1",
                [&request.event_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match replay {
            Some((old, response)) if old == encode(request)? => decode(&response).map(Some),
            Some(_) => Err("같은 요청 ID에 다른 내용이 들어왔습니다.".into()),
            None => Ok(None),
        }
    }

    pub fn segment(&self, id: &str) -> Result<ReviewSegment, String> {
        let text: String = self
            .db
            .query_row("SELECT body FROM segments WHERE id=?1", [id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        decode(&text)
    }

    fn review_source_matches(&self, request: &ReviewRequest) -> Result<bool, String> {
        if request.action.required_evidence() != ReviewEvidence::PlaybackAndMatchingSource {
            return Ok(true);
        }
        let segment = self.segment(&request.segment_id)?;
        let hash: String = self
            .db
            .query_row(
                "SELECT sha256 FROM recordings WHERE id=?1",
                [&segment.recording_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(self.audio.matches(&segment.recording_id, &hash))
    }

    pub(super) fn mutate(
        &mut self,
        authorization: crate::playback_evidence::AuthorizedReview<'_>,
    ) -> Result<ReviewSegment, String> {
        let request = authorization.request();
        if request.event_id.is_empty() || request.event_id.len() > 128 {
            return Err("요청 ID가 잘못됐습니다.".into());
        }
        if let Some(replayed) = self.replay(request)? {
            return Ok(replayed);
        }
        let source_matches = self.review_source_matches(request)?;
        let canonical = encode(request)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let replay: Option<(String, String)> = tx
            .query_row(
                "SELECT request,response FROM review_events WHERE id=?1",
                [&request.event_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((old, response)) = replay {
            if old != canonical {
                return Err(
                    "같은 요청 ID에 다른 내용이 들어왔습니다. 목록을 새로 불러오세요.".into(),
                );
            }
            return decode(&response);
        }
        let body: String = tx
            .query_row(
                "SELECT body FROM segments WHERE id=?1",
                [&request.segment_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let current: ReviewSegment = decode(&body)?;
        let (samples, revision): (u64, u64) = tx
            .query_row(
                "SELECT samples,revision FROM recordings WHERE id=?1",
                [&current.recording_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        if revision != current.recording_revision {
            return Err("녹음 version이 바뀌었습니다. 다시 불러오세요.".into());
        }
        let next = current.apply(request, samples).map_err(|e| e.to_string())?;
        if next.verified.is_some() {
            EligibleExample::check(&next, samples, source_matches).map_err(|e| e.to_string())?;
        }
        if let seamone_core::review::ReviewAction::SaveDraft { text }
        | seamone_core::review::ReviewAction::CorrectAndConfirm { text } = &request.action
        {
            let total: usize = tx.query_row("SELECT COALESCE(SUM(length(CAST(json_extract(body,'$.text') AS BLOB))),0) FROM segments WHERE recording_id=?1", [&current.recording_id], |r| r.get(0)).map_err(|e| e.to_string())?;
            if total
                .saturating_sub(current.text.len())
                .saturating_add(text.len())
                > seamone_core::review::MAX_RECORDING_TEXT_BYTES
            {
                return Err("전체 전사문은 2MB를 넘길 수 없습니다.".into());
            }
        }
        let response = encode(&next)?;
        if next == current {
            tx.execute(
                "INSERT INTO review_events(id,request,response) VALUES (?1,?2,?3)",
                params![request.event_id, canonical, response],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(next);
        }
        let changed = tx
            .execute(
                "UPDATE segments SET revision=?1,body=?2 WHERE id=?3 AND revision=?4",
                params![next.revision, response, next.id, request.expected_revision],
            )
            .map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err(seamone_core::review::ReviewError::StaleRevision.to_string());
        }
        tx.execute("DELETE FROM training_queue WHERE segment_id=?1", [&next.id])
            .map_err(|e| e.to_string())?;
        if next.verified.is_some() {
            tx.execute("INSERT INTO training_queue(segment_id,revision,recording_revision,status) VALUES (?1,?2,?3,'pending')", params![next.id,next.revision,next.recording_revision]).map_err(|e| e.to_string())?;
        }
        tx.execute(
            "INSERT INTO review_events(id,request,response) VALUES (?1,?2,?3)",
            params![request.event_id, canonical, response],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(next)
    }

    /// Recheck the DB revision, label revision, recording revision and the source digest.
    pub fn eligible(&self, id: &str, revision: u64) -> Result<EligibleExample, String> {
        let segment = self.segment(id)?;
        let info = self.recording(&segment.recording_id)?;
        if segment.revision != revision || info.revision != segment.recording_revision {
            return Err(seamone_core::review::ReviewError::StaleRevision.to_string());
        }
        let hash: String = self
            .db
            .query_row(
                "SELECT sha256 FROM recordings WHERE id=?1",
                [&info.id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        EligibleExample::check(&segment, info.samples, self.audio.matches(&info.id, &hash))
            .map_err(|e| e.to_string())
    }

    pub fn next_job(&self) -> Result<Option<EligibleExample>, String> {
        let mut stmt = self.db.prepare("SELECT segment_id,revision FROM training_queue WHERE status IN ('pending','retry') OR (status='running' AND lease_until<=unixepoch()) ORDER BY CASE WHEN status='pending' THEN 0 ELSE 1 END,rowid LIMIT 128").map_err(|e| e.to_string())?;
        let jobs = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        for (id, revision) in jobs {
            let segment = self.segment(&id)?;
            let info = self.recording(&segment.recording_id)?;
            // Obsolete labels are permanent; source failures are retryable and stay visible.
            let label_current = segment.revision == revision
                && segment.recording_revision == info.revision
                && EligibleExample::check(&segment, info.samples, true).is_ok();
            if !label_current {
                self.db.execute("UPDATE training_queue SET status='stale',last_error='검증한 수정본이 바뀌었습니다' WHERE segment_id=?1 AND revision=?2 AND (status IN ('pending','retry') OR lease_until<=unixepoch())",params![id,revision]).map_err(|e| e.to_string())?;
                continue;
            }
            let example = match self.eligible(&id, revision) {
                Ok(example) => example,
                Err(error) => {
                    self.db.execute("UPDATE training_queue SET status='retry',last_error=?1,claim_owner=NULL,lease_until=NULL WHERE segment_id=?2 AND revision=?3 AND (status IN ('pending','retry') OR lease_until<=unixepoch())",params![error,id,revision]).map_err(|e| e.to_string())?;
                    continue;
                }
            };
            let changed = self.db.execute("UPDATE training_queue SET status='running',claim_owner=?1,lease_until=unixepoch()+300,last_error=NULL WHERE segment_id=?2 AND revision=?3 AND (status IN ('pending','retry') OR (status='running' AND lease_until<=unixepoch())) AND EXISTS (SELECT 1 FROM segments WHERE id=?2 AND revision=?3)",params![self.owner,id,revision]).map_err(|e| e.to_string())?;
            if changed == 1 {
                return Ok(Some(example));
            }
        }
        Ok(None)
    }

    /// Called immediately before making a trained output visible, even after a long optimizer step.
    pub fn publish(&mut self, example: &EligibleExample, adapter_id: &str) -> Result<(), String> {
        let s = example.segment();
        if self.eligible(&s.id, s.revision)? != *example {
            return Err(seamone_core::review::ReviewError::StaleRevision.to_string());
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let changed = tx.execute("UPDATE training_queue SET status='complete',adapter_id=?1 WHERE segment_id=?2 AND revision=?3 AND recording_revision=?4 AND status='running' AND claim_owner=?5 AND lease_until>unixepoch() AND EXISTS (SELECT 1 FROM segments WHERE id=?2 AND revision=?3)", params![adapter_id,s.id,s.revision,s.recording_revision,self.owner]).map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err(seamone_core::review::ReviewError::StaleRevision.to_string());
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn retry_job(&self, example: &EligibleExample, error: &str) -> Result<(), String> {
        let s = example.segment();
        self.db.execute("UPDATE training_queue SET status='retry',last_error=?1,claim_owner=NULL,lease_until=NULL WHERE segment_id=?2 AND revision=?3 AND status='running' AND claim_owner=?4",params![error,s.id,s.revision,self.owner]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn defer_job(&self, example: &EligibleExample) -> Result<(), String> {
        let s = example.segment();
        self.db.execute("UPDATE training_queue SET status='pending',claim_owner=NULL,lease_until=NULL WHERE segment_id=?1 AND revision=?2 AND status='running' AND claim_owner=?3", params![s.id,s.revision,self.owner]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn export_text(&self, recording: &str) -> Result<String, String> {
        Ok(self
            .segments(recording)?
            .into_iter()
            .map(|s| s.text)
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

fn validate_error_ranges(
    tx: &Transaction<'_>,
    recording_id: &str,
    revision: u64,
    ranges: &[SampleRange],
) -> Result<(), String> {
    let (samples, current, complete): (u64, u64, bool) = tx
        .query_row(
            "SELECT samples,revision,transcription_state='complete' FROM recordings WHERE id=?1",
            [recording_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;
    if revision != current {
        return Err(ReviewError::StaleRevision.to_string());
    }
    if !complete {
        return Err("받아쓰기를 마친 뒤 오류를 표시하세요.".into());
    }
    for range in ranges {
        range.validate(samples).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn uncovered_ranges(ranges: &[SampleRange], covered: &[SampleRange]) -> Vec<SampleRange> {
    let mut requested: Vec<SampleRange> = Vec::new();
    for range in ranges.iter().copied() {
        if let Some(previous) = requested.last_mut()
            && previous.start < range.end
            && range.start < previous.end
            && previous.end.max(range.end) - previous.start
                <= seamone_core::review::MAX_PLAYBACK_SAMPLES
        {
            previous.end = previous.end.max(range.end);
        } else {
            let start = requested
                .last()
                .map_or(range.start, |previous| range.start.max(previous.end));
            if start < range.end {
                requested.push(SampleRange {
                    start,
                    end: range.end,
                });
            }
        }
    }
    let mut result = Vec::new();
    for range in requested {
        let mut start = range.start;
        for old in covered
            .iter()
            .skip_while(|old| old.end <= range.start)
            .take_while(|old| old.start < range.end)
        {
            if start < old.start {
                result.push(SampleRange {
                    start,
                    end: old.start,
                });
            }
            start = start.max(old.end);
        }
        if start < range.end {
            result.push(SampleRange {
                start,
                end: range.end,
            });
        }
    }
    result
}

fn flag_overlapping_segments(
    tx: &Transaction<'_>,
    recording_id: &str,
    revision: u64,
    ranges: &[SampleRange],
) -> Result<(), String> {
    let mut stmt = tx
        .prepare("SELECT body FROM segments WHERE recording_id=?1 ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([recording_id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut affected = Vec::new();
    for row in rows {
        let segment: ReviewSegment = decode(&row.map_err(|e| e.to_string())?)?;
        if segment.recording_revision == revision
            && ranges.iter().any(|&range| overlaps(segment.range, range))
        {
            affected.push(segment);
        }
    }
    drop(stmt);
    for mut segment in affected {
        segment.revision = segment
            .revision
            .checked_add(1)
            .ok_or_else(|| ReviewError::StaleRevision.to_string())?;
        segment.verified = None;
        segment.error_marked_text = Some(segment.text.clone());
        tx.execute(
            "UPDATE segments SET revision=?1,body=?2 WHERE id=?3",
            params![segment.revision, encode(&segment)?, segment.id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM training_queue WHERE segment_id=?1",
            [&segment.id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn insert_error_mark(tx: &Transaction<'_>, mark: &ErrorMark) -> Result<(), String> {
    tx.execute(
        "INSERT INTO error_marks(id,recording_id,recording_revision,start_sample,end_sample)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            mark.id,
            mark.recording_id,
            mark.recording_revision,
            mark.range.start,
            mark.range.end
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn read_error_mark(row: &rusqlite::Row<'_>) -> rusqlite::Result<ErrorMark> {
    Ok(ErrorMark {
        id: row.get(0)?,
        recording_id: row.get(1)?,
        recording_revision: row.get(2)?,
        range: SampleRange {
            start: row.get(3)?,
            end: row.get(4)?,
        },
    })
}

fn encode<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| e.to_string())
}
fn decode<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}
