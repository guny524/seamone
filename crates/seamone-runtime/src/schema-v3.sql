CREATE TABLE error_marks (
    id TEXT PRIMARY KEY,
    recording_id TEXT NOT NULL REFERENCES recordings(id),
    recording_revision INTEGER NOT NULL CHECK (recording_revision > 0),
    start_sample INTEGER NOT NULL CHECK (start_sample >= 0),
    end_sample INTEGER NOT NULL CHECK (end_sample > start_sample)
);
CREATE INDEX error_marks_recording ON error_marks(recording_id);
PRAGMA user_version = 3;
