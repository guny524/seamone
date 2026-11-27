//! Command names shared by the UI, native handler and Tauri permission generator.

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
                SaveReview => save_review,
                DismissInfo => dismiss_info,
                ExportText => export_text,
                CopySavedText => copy_saved_text,
                SaveTextFile => save_text_file,
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
     desktop_capture { $($capture:ident => $capture_handler:ident,)* }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Command {
            $($shared,)*
            $($capture,)*
        }

        impl Command {
            pub const NAMES: &[&str] = &[
                $(Self::$shared.name(),)*
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
    };
}

with_ipc_commands!(define_commands);
