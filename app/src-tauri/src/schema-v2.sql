ALTER TABLE recordings ADD COLUMN transcription_state TEXT NOT NULL DEFAULT 'pending'
    CHECK (transcription_state IN ('pending','complete'));
UPDATE recordings SET transcription_state='complete'
    WHERE json_extract(info,'$.engine') != '아직 받아쓰지 않음';
ALTER TABLE training_queue RENAME TO queue_v1;
CREATE TABLE training_queue (
    segment_id TEXT PRIMARY KEY REFERENCES segments(id),
    revision INTEGER NOT NULL,
    recording_revision INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending','retry','running','stale','complete')),
    adapter_id TEXT,
    claim_owner TEXT,
    lease_until INTEGER,
    last_error TEXT
);
INSERT INTO training_queue(segment_id,revision,recording_revision,status,adapter_id)
    SELECT segment_id,revision,recording_revision,
        CASE WHEN status='running' THEN 'pending' ELSE status END,adapter_id FROM queue_v1;
DROP TABLE queue_v1;
PRAGMA user_version = 2;
