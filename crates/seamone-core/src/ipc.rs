//! Command names shared by the UI, native handler and Tauri permission generator.

use crate::{
    AppInfo,
    error_marks::{ErrorMark, ErrorMarkBatch, ErrorMarkPage},
    playback::{Action, Status},
    review::{
        LibraryPage, PlaybackReply, RecordingInfo, Request, SampleRange, Segment, SegmentPage,
    },
};
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The existing top-level argument object for saving a review.
///
/// ```compile_fail,E0308
/// use seamone_core::ipc::SaveReview;
/// fn wrong_request() -> SaveReview {
///     SaveReview { request: "a segment ID", proof: None }
/// }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingArgs {
    pub recording_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingPageArgs {
    pub recording_id: String,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishRecordingArgs {
    pub session_id: String,
    pub save: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveErrorMarkArgs {
    pub mark: ErrorMark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveErrorMarksArgs {
    pub batch: ErrorMarkBatch,
}

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
///
/// ```compile_fail,E0277
/// use seamone_core::ipc::{EmptyArgs, TypedCommand};
/// struct UnregisteredCommand;
/// impl TypedCommand for UnregisteredCommand {
///     const NAME: &'static str = "unregistered_command";
///     type Args = EmptyArgs;
///     type Reply = ();
/// }
/// ```
///
/// ```compile_fail,E0308
/// use seamone_core::ipc::{TypedCommand, command::FinishRecording};
/// fn call<C: TypedCommand>(_: C::Args) {}
/// call::<FinishRecording>(serde_json::json!({"sessionId":"recording","save":true}));
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
                GetAppInfo => get_app_info: EmptyArgs, AppInfo;
                GetLibrary => get_library: EmptyArgs, LibraryPage;
                GetSegments => get_segments: RecordingPageArgs, SegmentPage;
                ImportAudio => import_audio: EmptyArgs, Option<RecordingInfo>;
                GetCaptureStatus => get_capture_status: EmptyArgs, $crate::capture::Status;
                TranscribeRecording => transcribe_recording: RecordingArgs, Vec<Segment>;
                SelectModel => select_model: EmptyArgs, bool;
                GetErrorMarks => get_error_marks: RecordingPageArgs, ErrorMarkPage;
                SaveErrorMark => save_error_mark: SaveErrorMarkArgs, ();
                SaveErrorMarks => save_error_marks: SaveErrorMarksArgs, ();
                DismissInfo => dismiss_info: EmptyArgs, ();
                ExportText => export_text: RecordingArgs, String;
                CopySavedText => copy_saved_text: RecordingArgs, ();
                SaveTextFile => save_text_file: RecordingArgs, bool;
                SaveReview => save_review: SaveReview, Segment;
                GetPlayback => get_playback: SegmentPlaybackArgs, PlaybackReply;
                GetRecordingPlayback => get_recording_playback: RecordingPlaybackArgs, String;
                ControlPlayback => control_playback: ControlPlaybackArgs, Status;
                GetPlaybackStatus => get_playback_status: EmptyArgs, Status;
                PlayRecording => play_recording: RecordingPlaybackArgs, Status;
            }
            desktop_capture {
                StartRecording => start_recording: EmptyArgs, $crate::capture::Status;
                FinishRecording => finish_recording: FinishRecordingArgs, Option<RecordingInfo>;
            }
        }
    };
}

macro_rules! define_commands {
    (shared { $($shared:ident => $handler:ident: $args:ty, $reply:ty;)* }
     desktop_capture { $($capture:ident => $capture_handler:ident: $capture_args:ty, $capture_reply:ty;)* }) => {
        pub const COMMAND_NAMES: &[&str] = &[
            $(<command::$shared as TypedCommand>::NAME,)*
            $(<command::$capture as TypedCommand>::NAME,)*
        ];

        pub mod command {
            $(pub enum $shared {})*
            $(pub enum $capture {})*
        }

        $(impl private::Sealed for command::$shared {}
        impl TypedCommand for command::$shared {
            const NAME: &'static str = stringify!($handler);
            type Args = $args;
            type Reply = $reply;
        })*

        $(impl private::Sealed for command::$capture {}
        impl TypedCommand for command::$capture {
            const NAME: &'static str = stringify!($capture_handler);
            type Args = $capture_args;
            type Reply = $capture_reply;
        })*
    };
}

with_ipc_commands!(define_commands);

#[cfg(test)]
mod tests {
    use super::{
        ControlPlaybackArgs, EmptyArgs, FinishRecordingArgs, RecordingArgs, RecordingPageArgs,
        RecordingPlaybackArgs, SaveErrorMarkArgs, SaveErrorMarksArgs, SaveReview,
        SegmentPlaybackArgs, TypedCommand, command,
    };
    use crate::{
        error_marks::{ErrorMark, ErrorMarkBatch},
        playback::Action,
        review::SampleRange,
    };
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

    #[test]
    fn library_and_capture_contracts_preserve_the_existing_json() {
        assert_arguments::<command::GetLibrary>(EmptyArgs {}, json!({}));
        assert_arguments::<command::FinishRecording>(
            FinishRecordingArgs {
                session_id: "session".into(),
                save: true,
            },
            json!({"sessionId":"session","save":true}),
        );
        assert_arguments::<command::GetSegments>(
            RecordingPageArgs {
                recording_id: "recording".into(),
                offset: 32,
            },
            json!({"recordingId":"recording","offset":32}),
        );
        assert_arguments::<command::SaveTextFile>(
            RecordingArgs {
                recording_id: "recording".into(),
            },
            json!({"recordingId":"recording"}),
        );
    }

    #[test]
    fn error_mark_arguments_preserve_the_existing_nested_json() {
        let range = SampleRange { start: 8, end: 16 };
        assert_arguments::<command::SaveErrorMark>(
            SaveErrorMarkArgs {
                mark: ErrorMark {
                    id: "event".into(),
                    recording_id: "recording".into(),
                    recording_revision: 3,
                    range,
                },
            },
            json!({"mark":{"id":"event","recording_id":"recording","recording_revision":3,"range":{"start":8,"end":16}}}),
        );
        assert_arguments::<command::SaveErrorMarks>(
            SaveErrorMarksArgs {
                batch: ErrorMarkBatch {
                    id: "event".into(),
                    recording_id: "recording".into(),
                    recording_revision: 3,
                    ranges: vec![range],
                },
            },
            json!({"batch":{"id":"event","recording_id":"recording","recording_revision":3,"ranges":[{"start":8,"end":16}]}}),
        );
    }
}
