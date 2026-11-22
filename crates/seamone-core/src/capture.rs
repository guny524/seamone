use serde::{Deserialize, Serialize};

pub const MAX_RECORDING_SECONDS: u32 = 30 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CaptureStatus {
    Unavailable,
    Idle,
    Starting {
        session_id: String,
    },
    Recording {
        session_id: String,
        elapsed_seconds: u32,
    },
    LimitReached {
        session_id: String,
    },
    Interrupted {
        session_id: String,
        elapsed_seconds: u32,
        reason: String,
    },
    Finishing {
        session_id: String,
    },
    Failed {
        message: String,
    },
}
