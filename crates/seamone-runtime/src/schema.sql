CREATE TABLE recordings (
    id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK (revision > 0),
    samples INTEGER NOT NULL CHECK (samples > 0),
    sha256 TEXT NOT NULL,
    info TEXT NOT NULL
);
CREATE TABLE segments (
    id TEXT PRIMARY KEY,
    recording_id TEXT NOT NULL REFERENCES recordings(id),
    revision INTEGER NOT NULL CHECK (revision > 0),
    body TEXT NOT NULL
);
CREATE INDEX segments_recording ON segments(recording_id);
CREATE TABLE review_events (id TEXT PRIMARY KEY, request TEXT NOT NULL, response TEXT NOT NULL);
CREATE TABLE training_queue (
    segment_id TEXT PRIMARY KEY REFERENCES segments(id),
    revision INTEGER NOT NULL,
    recording_revision INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending','running','stale','complete')),
    adapter_id TEXT
);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
PRAGMA user_version = 1;
