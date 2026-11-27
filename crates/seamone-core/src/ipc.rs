//! Command names shared by the UI, native handler and Tauri permission generator.

use crate::review::{Request, Segment};
use alloc::string::String;
use serde::{Deserialize, Serialize};

/// The existing top-level argument object for saving a review.
///
/// ```compile_fail,E0308
/// use seamone_core::ipc::SaveReview;
/// fn wrong_request() -> SaveReview {
///     SaveReview { request: "a segment ID", proof: None }
/// }
/// ```
///
/// ```compile_fail,E0599
/// use seamone_core::ipc::Command;
/// let command = Command::SaveReview;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveReview {
    pub request: Request,
    pub proof: Option<String>,
}

pub type SaveReviewReply = Segment;

/// Supplies command variants and their native function identifiers to a consumer macro.
#[macro_export]
macro_rules! with_ipc_commands {
    ($consumer:ident) => {
        $consumer! {
            shared {
                GetAppInfo => get_app_info,
                GetLibrary => get_library,
                GetSegments => get_segments,
                ImportAudio => import_audio,
                GetCaptureStatus => get_capture_status,
                TranscribeRecording => transcribe_recording,
                SelectModel => select_model,
                GetPlayback => get_playback,
                GetRecordingPlayback => get_recording_playback,
                GetErrorMarks => get_error_marks,
                SaveErrorMark => save_error_mark,
                SaveErrorMarks => save_error_marks,
                ControlPlayback => control_playback,
                GetPlaybackStatus => get_playback_status,
                PlayRecording => play_recording,
                DismissInfo => dismiss_info,
                ExportText => export_text,
                CopySavedText => copy_saved_text,
                SaveTextFile => save_text_file,
            }
            typed {
                SaveReview => save_review,
            }
            desktop_capture {
                StartRecording => start_recording,
                FinishRecording => finish_recording,
            }
        }
    };
}

macro_rules! define_commands {
    (shared { $($shared:ident => $handler:ident,)* }
     typed { $($typed:ident => $typed_handler:ident,)* }
     desktop_capture { $($capture:ident => $capture_handler:ident,)* }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Command {
            $($shared,)*
            $($capture,)*
        }

        impl Command {
            pub const NAMES: &[&str] = &[
                $(Self::$shared.name(),)*
                $($typed::NAME,)*
                $(Self::$capture.name(),)*
            ];

            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$shared => stringify!($handler),)*
                    $(Self::$capture => stringify!($capture_handler),)*
                }
            }
        }

        $(impl $typed {
            pub const NAME: &'static str = stringify!($typed_handler);
        })*
    };
}

with_ipc_commands!(define_commands);

#[cfg(test)]
mod tests {
    use super::SaveReview;
    use serde_json::{from_value, json, to_value};

    #[test]
    fn save_review_preserves_top_level_arguments_and_optional_proof() {
        let request = json!({
            "event_id": "change",
            "segment_id": "segment",
            "expected_revision": 1,
            "action": {"kind": "save_draft", "text": "draft"}
        });
        for proof in [json!(null), json!("completed-playback")] {
            let body = json!({"request":request,"proof":proof});
            let args: SaveReview = from_value(body.clone()).unwrap();
            assert_eq!(to_value(args).unwrap(), body);
        }
        let omitted: SaveReview = from_value(json!({"request":request})).unwrap();
        assert!(omitted.proof.is_none());
    }
}
