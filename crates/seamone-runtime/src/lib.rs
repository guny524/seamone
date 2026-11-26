//! 기기 안 보관함의 작업 경계. 저장과 검수 증거는 `LibraryService`만 바꾼다.
//!
//! 공개 서비스는 보관함을 열고 읽는 경로를 제공한다.
//! ```
//! use seamone_runtime::LibraryService;
//! fn library(service: &LibraryService) { let _ = service.library(); }
//! ```
//! 저장소 내부에는 crate 밖에서 접근할 수 없다.
//! ```compile_fail,E0603
//! use seamone_runtime::store::Store;
//! ```
//! ```
//! use seamone_runtime::LibraryService;
//! fn open(root: &std::path::Path) { let _ = LibraryService::open(root); }
//! ```
//! ```compile_fail,E0603
//! use seamone_runtime::audio::AudioLibrary;
//! ```
//! ```
//! use seamone_runtime::LibraryService;
//! fn audio(service: &LibraryService) { let _ = service.capture_status(); }
//! ```
//! ```compile_fail,E0432
//! use seamone_runtime::Connection;
//! ```
//! ```
//! use seamone_runtime::LibraryService;
//! fn read(service: &LibraryService) { let _ = service.export_text("recording-id"); }
//! ```
//! ```compile_fail,E0624
//! use seamone_runtime::LibraryService;
//! use seamone_core::review::ReviewRequest;
//! fn mutate(service: &LibraryService, request: &ReviewRequest) {
//!     service.store().unwrap().mutate(request);
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
mod store;
#[cfg(test)]
mod store_tests;
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
    collections::HashMap,
    fs::File,
    io::Read,
    path::Path,
    sync::{Mutex, MutexGuard},
};
use store::Store;
pub use training::LocalTrainer;
use uuid::Uuid;

/// Whether this target supports native microphone capture.
pub const CAPTURE_SUPPORTED: bool = cfg!(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
));

struct Listening {
    segment_id: String,
    revision: u64,
    complete: bool,
}

/// Owns recording, playback evidence, verified revisions and local work.
pub struct LibraryService {
    store: std::sync::Arc<Mutex<Store>>,
    background: crate::training::BackgroundWork,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    recorder: crate::capture::Recorder,
    listening: Mutex<HashMap<String, Listening>>,
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
            listening: Mutex::default(),
        }
    }
    fn store(&self) -> Result<MutexGuard<'_, Store>, String> {
        self.store
            .lock()
            .map_err(|_| "보관함을 다시 여세요.".into())
    }

    pub fn playback(&self, id: &str, revision: u64) -> Result<PlaybackReply, String> {
        let store = self.store()?;
        let segment = store.segment(id)?;
        if segment.revision != revision {
            return Err(seamone_core::review::ReviewError::StaleRevision.to_string());
        }
        let recording = store.recording(&segment.recording_id)?;
        let wav = store
            .audio
            .range_wav(&segment.recording_id, segment.range, recording.samples)?;
        let token = Uuid::new_v4().to_string();
        let mut listening = self.listening.lock().map_err(|e| e.to_string())?;
        if listening.len() >= 128 {
            listening.clear();
        }
        listening.insert(
            token.clone(),
            Listening {
                segment_id: id.into(),
                revision,
                complete: false,
            },
        );
        Ok(PlaybackReply {
            segment_id: id.into(),
            revision,
            wav_base64: STANDARD.encode(wav),
            token,
        })
    }

    pub fn playback_ended(&self, token: &str) -> Result<(), String> {
        let mut listening = self.listening.lock().map_err(|e| e.to_string())?;
        let item = listening
            .get_mut(token)
            .ok_or("재생이 바뀌었습니다. 이 조각을 다시 들으세요.")?;
        item.complete = true;
        Ok(())
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
        if request.action.required_evidence().requires_playback() {
            let listening = self.listening.lock().map_err(|e| e.to_string())?;
            let valid = proof.and_then(|p| listening.get(p)).is_some_and(|l| {
                l.complete
                    && l.segment_id == request.segment_id
                    && l.revision == request.expected_revision
            });
            if !valid {
                return Err("현재 수정본의 음성을 끝까지 들은 뒤 확인하세요.".into());
            }
        }
        let saved = store.mutate(request)?;
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
        name: String,
        input: File,
        can_commit: impl Fn() -> bool,
    ) -> Result<RecordingInfo, String> {
        let _foreground = self.background.foreground.enter();
        let library = self.store()?.audio.clone();
        let mut audio = audio_ingest::decode(&library, input)?;
        self.store()?.save_audio(name, &mut audio, can_commit)
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
