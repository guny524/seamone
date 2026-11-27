//! Runtime output controls and device-confirmed source positions.

use alloc::string::String;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Play,
    Pause,
    Seek { at: u64 },
    Stop,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    #[default]
    Idle,
    Ready,
    Playing,
    Paused,
    Finished,
    Failed,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub session: Option<String>,
    pub state: State,
    pub position: u64,
    pub heard: bool,
    pub error: Option<String>,
}
