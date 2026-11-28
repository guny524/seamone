//! Command names shared by the UI, native handler and Tauri permission generator.

use crate::{
    playback::{Action, Status},
    review::{PlaybackReply, Request, SampleRange, Segment},
};
use alloc::string::String;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentPlaybackArgs {
    pub segment_id: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingPlaybackArgs {
    pub recording_id: String,
    pub revision: u64,
    pub range: SampleRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlPlaybackArgs {
    pub session: String,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EmptyArgs {}

mod private {
    pub trait Sealed {}
}

/// A registered command whose request and response types are chosen by the catalog.
///
/// ```compile_fail,E0308
/// use seamone_core::ipc::{TypedCommand, command::PlayRecording, SegmentPlaybackArgs};
/// fn call<C: TypedCommand>(_: C::Args) {}
/// fn wrong_args(args: SegmentPlaybackArgs) {
///     call::<PlayRecording>(args);
/// }
/// ```
///
/// ```compile_fail,E0308
/// use seamone_core::{ipc::{TypedCommand, command::PlayRecording}, review::PlaybackReply};
/// fn wrong_reply(reply: <PlayRecording as TypedCommand>::Reply) -> PlaybackReply {
///     reply
/// }
/// ```
pub trait TypedCommand: private::Sealed {
    const NAME: &'static str;
    type Args: Serialize + DeserializeOwned;
    type Reply: Serialize + DeserializeOwned;
}

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
                GetErrorMarks => get_error_marks,
                SaveErrorMark => save_error_mark,
                SaveErrorMarks => save_error_marks,
                DismissInfo => dismiss_info,
                ExportText => export_text,
                CopySavedText => copy_saved_text,
                SaveTextFile => save_text_file,
            }
            typed {
                SaveReview => save_review: SaveReview, Segment;
                GetPlayback => get_playback: SegmentPlaybackArgs, PlaybackReply;
                GetRecordingPlayback => get_recording_playback: RecordingPlaybackArgs, String;
                ControlPlayback => control_playback: ControlPlaybackArgs, Status;
                GetPlaybackStatus => get_playback_status: EmptyArgs, Status;
                PlayRecording => play_recording: RecordingPlaybackArgs, Status;
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
     typed { $($typed:ident => $typed_handler:ident: $args:ty, $reply:ty;)* }
     desktop_capture { $($capture:ident => $capture_handler:ident,)* }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Command {
            $($shared,)*
            $($capture,)*
        }

        impl Command {
            pub const NAMES: &[&str] = &[
                $(Self::$shared.name(),)*
                $(<command::$typed as TypedCommand>::NAME,)*
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

        pub mod command {
            $(pub enum $typed {})*
        }

        $(impl private::Sealed for command::$typed {}
        impl TypedCommand for command::$typed {
            const NAME: &'static str = stringify!($typed_handler);
            type Args = $args;
            type Reply = $reply;
        })*
    };
}

with_ipc_commands!(define_commands);

#[cfg(test)]
mod tests {
    use super::{
        ControlPlaybackArgs, EmptyArgs, RecordingPlaybackArgs, SaveReview, SegmentPlaybackArgs,
        TypedCommand, command,
    };
    use crate::{playback::Action, review::SampleRange};
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

    fn assert_arguments<C: TypedCommand>(args: C::Args, expected: serde_json::Value) {
        assert_eq!(to_value(&args).unwrap(), expected);
        let decoded: C::Args = from_value(expected).unwrap();
        assert_eq!(to_value(decoded).unwrap(), to_value(args).unwrap());
    }

    #[test]
    fn playback_contracts_use_the_existing_json_keys_and_actions() {
        assert_arguments::<command::GetPlayback>(
            SegmentPlaybackArgs {
                segment_id: "segment".into(),
                revision: 3,
            },
            json!({"segmentId":"segment","revision":3}),
        );
        let recording = RecordingPlaybackArgs {
            recording_id: "recording".into(),
            revision: 3,
            range: SampleRange { start: 8, end: 16 },
        };
        let recording_json =
            json!({"recordingId":"recording","revision":3,"range":{"start":8,"end":16}});
        assert_arguments::<command::GetRecordingPlayback>(
            recording.clone(),
            recording_json.clone(),
        );
        assert_arguments::<command::PlayRecording>(recording, recording_json);
        assert_arguments::<command::ControlPlayback>(
            ControlPlaybackArgs {
                session: "playback".into(),
                action: Action::Seek { at: 10 },
            },
            json!({"session":"playback","action":{"kind":"seek","at":10}}),
        );
        assert_arguments::<command::GetPlaybackStatus>(EmptyArgs {}, json!({}));
    }
}
