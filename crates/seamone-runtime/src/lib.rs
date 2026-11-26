//! 기기 안 보관함의 작업 경계, 저장과 검수 증거는 `LibraryService`를 거친다.
//!
//! A library is available through its service.
//! ```compile_fail,E0603
//! use seamone_runtime::store::Store;
//! ```
//! ```
//! use seamone_runtime::LibraryService as Store;
//! ```
//!
//! Storage is not exported at the crate root.
//! ```compile_fail,E0432
//! use seamone_runtime::Store;
//! ```
//! ```
//! use seamone_runtime::LibraryService as Store;
//! ```
//!
//! Audio storage stays inside the runtime.
//! ```compile_fail,E0603
//! use seamone_runtime::audio::AudioLibrary;
//! ```
//! ```
//! use seamone_runtime::LibraryService as AudioLibrary;
//! ```
//!
//! A raw connection is not exported.
//! ```compile_fail,E0432
//! use seamone_runtime::Connection;
//! ```
//! ```
//! use seamone_runtime::LibraryService as Connection;
//! ```
//!
//! Callers submit reviews through the service.
//! ```compile_fail,E0624
//! use seamone_runtime::LibraryService;
//! use seamone_core::review::ReviewRequest;
//! fn review(service: &LibraryService, request: &ReviewRequest) {
//!     let _ = service.store().unwrap().mutate(request);
//! }
//! ```
//! ```
//! use seamone_runtime::LibraryService;
//! use seamone_core::review::ReviewRequest;
//! fn review(service: &LibraryService, request: &ReviewRequest) {
//!     let _ = service.review(request, None);
//! }
//! ```
mod audio;
mod audio_ingest;
#[cfg(test)]
mod audio_input_tests;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod capture;
mod inference;
mod normalize;
mod playback_evidence;
mod store;
#[cfg(test)]
mod store_tests;
#[cfg(feature = "test-support")]
pub mod test_support;
mod training;

use base64::{Engine, engine::general_purpose::STANDARD};
use seamone_core::{
    capture::CaptureStatus,
    error_marks::{ErrorMark, ErrorMarkBatch, ErrorMarkPage},
    review::{
        LibraryPage, PlaybackReply, RecordingInfo, ReviewError, ReviewRequest, ReviewSegment,
        SampleRange, SegmentPage,
    },
};
use std::{
    fs::File,
    io::Read,
    path::Path,
    sync::{Mutex, MutexGuard},
};
use store::Store;
pub use training::LocalTrainer;

/// Whether this target supports native microphone capture.
pub const CAPTURE_SUPPORTED: bool = cfg!(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
));

/// Owns recording, playback evidence, verified revisions and local work.
#[doc = concat!(
    "A sibling must submit the immutable request authorized by the gate.\n",
    "```compile_fail,E0308\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet token = playbacks.issue(store, &request.segment_id, request.expected_revision).unwrap().token;\nplaybacks.ended(&token).unwrap();\nlet _ = store.mutate(request);\n}\n}\n# fn main() {}\n```\n",
    "```\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet token = playbacks.issue(store, &request.segment_id, request.expected_revision).unwrap().token;\nplaybacks.ended(&token).unwrap();\nlet _ = store.mutate(playbacks.authorize(request, Some(&token)).unwrap());\n}\n}\n# fn main() {}\n```\n",
    "A sibling cannot construct the proof's private fields.\n",
    "```compile_fail,E0451\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet token = playbacks.issue(store, &request.segment_id, request.expected_revision).unwrap().token;\nplaybacks.ended(&token).unwrap();\nlet _ = super::playback_evidence::PlaybackProof { request };\n}\n}\n# fn main() {}\n```\n",
    "```\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet token = playbacks.issue(store, &request.segment_id, request.expected_revision).unwrap().token;\nplaybacks.ended(&token).unwrap();\nlet _ = playbacks.authorize(request, Some(&token)).unwrap();\n}\n}\n# fn main() {}\n```\n",
    "A sibling cannot construct proof-free authorization.\n",
    "```compile_fail,E0451\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet token = playbacks.issue(store, &request.segment_id, request.expected_revision).unwrap().token;\nplaybacks.ended(&token).unwrap();\nlet _ = super::playback_evidence::AuthorizedReview { authority: todo!() };\n}\n}\n# fn main() {}\n```\n",
    "```\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet token = playbacks.issue(store, &request.segment_id, request.expected_revision).unwrap().token;\nplaybacks.ended(&token).unwrap();\nlet _ = playbacks.authorize(request, Some(&token)).unwrap();\n}\n}\n# fn main() {}\n```\n",
    "A sibling cannot use a Default constructor.\n",
    "```compile_fail,E0599\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::default();\nlet token = playbacks.issue(store, &request.segment_id, request.expected_revision).unwrap().token;\nplaybacks.ended(&token).unwrap();\nlet _ = store.mutate(playbacks.authorize(request, Some(&token)).unwrap());\n}\n}\n# fn main() {}\n```\n",
    "```\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet token = playbacks.issue(store, &request.segment_id, request.expected_revision).unwrap().token;\nplaybacks.ended(&token).unwrap();\nlet _ = store.mutate(playbacks.authorize(request, Some(&token)).unwrap());\n}\n}\n# fn main() {}\n```\n",
    "A sibling cannot issue a token without preparing the audio.\n",
    "```compile_fail,E0624\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet token = playbacks.begin(&request.segment_id, request.expected_revision).unwrap();\nplaybacks.ended(&token).unwrap();\nlet _ = store.mutate(playbacks.authorize(request, Some(&token)).unwrap());\n}\n}\n# fn main() {}\n```\n",
    "```\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio.rs\"#] mod audio;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/normalize.rs\"#] mod normalize;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/playback_evidence.rs\"#] mod playback_evidence;\n",
    "# #[path = r#\"", env!("CARGO_MANIFEST_DIR"), "/src/store.rs\"#] mod store;\n",
    "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::ReviewRequest) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet token = playbacks.issue(store, &request.segment_id, request.expected_revision).unwrap().token;\nplaybacks.ended(&token).unwrap();\nlet _ = store.mutate(playbacks.authorize(request, Some(&token)).unwrap());\n}\n}\n# fn main() {}\n```\n",
)]
pub struct LibraryService {
    store: std::sync::Arc<Mutex<Store>>,
    background: crate::training::BackgroundWork,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    recorder: crate::capture::Recorder,
    listening: playback_evidence::Playbacks,
}

impl LibraryService {
    fn new(store: Store) -> Self {
        let store = std::sync::Arc::new(Mutex::new(store));
        let background = crate::training::BackgroundWork::new(&store, None);
        Self {
            store,
            background,
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            recorder: crate::capture::Recorder::default(),
            listening: playback_evidence::Playbacks::new(),
        }
    }
    fn store(&self) -> Result<MutexGuard<'_, Store>, String> {
        self.store
            .lock()
            .map_err(|_| "보관함을 다시 여세요.".into())
    }

    pub fn playback(&self, id: &str, revision: u64) -> Result<PlaybackReply, String> {
        let store = self.store()?;
        self.listening.issue(&store, id, revision)
    }

    pub fn playback_ended(&self, token: &str) -> Result<(), String> {
        self.listening.ended(token)
    }

    pub fn review(
        &self,
        request: &ReviewRequest,
        proof: Option<&str>,
    ) -> Result<ReviewSegment, String> {
        let mut store = self.store()?;
        if let Some(replayed) = store.replay(request)? {
            return Ok(replayed);
        }
        let authorized = self.listening.authorize(request, proof)?;
        let saved = store.mutate(authorized)?;
        drop(store);
        if saved.verified.is_some() {
            self.background.notify();
        }
        Ok(saved)
    }
}

impl LibraryService {
    pub fn open(root: &Path) -> Result<Self, String> {
        Store::open(root).map(Self::new)
    }

    #[must_use]
    pub const fn inference_available() -> bool {
        inference::available()
    }

    pub fn library(&self) -> Result<LibraryPage, String> {
        self.store()?.library(Self::inference_available())
    }
    pub fn segments(&self, id: &str, offset: u32) -> Result<SegmentPage, String> {
        self.store()?.segment_page(id, offset)
    }
    pub fn error_marks(&self, id: &str, offset: u32) -> Result<ErrorMarkPage, String> {
        self.store()?.error_mark_page(id, offset)
    }
    pub fn save_error_mark(&self, mark: &ErrorMark) -> Result<(), String> {
        self.store()?.save_error_mark(mark)
    }
    pub fn save_error_marks(&self, batch: &ErrorMarkBatch) -> Result<(), String> {
        self.store()?.save_error_marks(batch)
    }
    pub fn dismiss_info(&self) -> Result<(), String> {
        self.store()?.set_setting("info_seen", "1")
    }
    pub fn export_text(&self, id: &str) -> Result<String, String> {
        self.store()?.export_text(id)
    }
    pub fn recording_playback(
        &self,
        id: &str,
        revision: u64,
        range: SampleRange,
    ) -> Result<String, String> {
        let store = self.store()?;
        let recording = store.recording(id)?;
        if recording.revision != revision {
            return Err(ReviewError::StaleRevision.to_string());
        }
        let wav = store.audio.range_wav(id, range, recording.samples)?;
        Ok(STANDARD.encode(wav))
    }
    pub fn import_path(
        &self,
        path: &Path,
        can_commit: impl Fn() -> bool,
    ) -> Result<RecordingInfo, String> {
        let _foreground = self.background.foreground.enter();
        let library = self.store()?.audio.clone();
        let mut audio = audio_ingest::decode(&library, library.open_source(path)?)?;
        self.store()?
            .save_audio(audio_ingest::display_name(path), &mut audio, can_commit)
    }
    pub fn import_file(
        &self,
        source: impl AsRef<Path>,
        input: File,
        can_commit: impl Fn() -> bool,
    ) -> Result<RecordingInfo, String> {
        let _foreground = self.background.foreground.enter();
        let library = self.store()?.audio.clone();
        let mut audio = audio_ingest::decode(&library, input)?;
        self.store()?.save_audio(
            audio_ingest::display_name(source.as_ref()),
            &mut audio,
            can_commit,
        )
    }
    /// Copies a document-provider stream to a bounded, seekable file owned by the library.
    pub fn import_stream(
        &self,
        name: String,
        input: impl Read,
        can_commit: impl Fn() -> bool,
    ) -> Result<RecordingInfo, String> {
        let _foreground = self.background.foreground.enter();
        let library = self.store()?.audio.clone();
        let mut input = input.take(audio_ingest::MAX_FILE_BYTES + 1);
        let mut scratch = library.scratch_file()?;
        let bytes = std::io::copy(&mut input, &mut scratch).map_err(|e| e.to_string())?;
        if bytes > audio_ingest::MAX_FILE_BYTES {
            return Err(audio_ingest::SIZE_ERROR.into());
        }
        self.import_file(name, scratch, can_commit)
    }
    pub fn select_model(&self, path: &Path) -> Result<(), String> {
        inference::select(self, path)
    }
    pub fn transcribe(&self, id: &str) -> Result<Vec<ReviewSegment>, String> {
        inference::transcribe(self, id)
    }

    #[must_use]
    pub fn capture_status(&self) -> CaptureStatus {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.recorder.status()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            CaptureStatus::Unavailable
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub fn start_recording(&self) -> Result<CaptureStatus, String> {
        let library = self.store()?.audio.clone();
        self.recorder
            .start(library, self.background.foreground.enter())
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub fn finish_recording(&self, id: &str, save: bool) -> Result<Option<RecordingInfo>, String> {
        let _foreground = self.background.foreground.enter();
        self.recorder.finish(id, save, |audio, active| {
            self.store()?
                .save_audio("마이크 녹음.wav".into(), audio, active)
        })
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[must_use]
    pub fn request_close(&self) -> bool {
        self.recorder.request_close()
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub fn close(&self) {
        self.recorder.close();
    }
}

#[cfg(test)]
mod service_tests;
