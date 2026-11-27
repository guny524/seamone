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
//! use seamone_core::review::Request;
//! fn review(service: &LibraryService, request: &Request) {
//!     let _ = service.store().unwrap().mutate(request);
//! }
//! ```
//! ```
//! use seamone_runtime::LibraryService;
//! use seamone_core::review::Request;
//! fn review(service: &LibraryService, request: &Request) {
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
mod write_guard;

use base64::{Engine, engine::general_purpose::STANDARD};
use seamone_core::{
    capture::Status,
    error_marks::{ErrorMark, ErrorMarkBatch, ErrorMarkPage},
    review::{
        Error, LibraryPage, PlaybackReply, RecordingInfo, Request, SampleRange, Segment,
        SegmentPage,
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

macro_rules! evidence_example {
    ($description:literal, $kind:literal, $body:literal) => {
        concat!(
            $description,
            "\n",
            "```",
            $kind,
            "\n",
            "# #[path = r#\"",
            env!("CARGO_MANIFEST_DIR"),
            "/src/audio.rs\"#] mod audio;\n",
            "# #[path = r#\"",
            env!("CARGO_MANIFEST_DIR"),
            "/src/normalize.rs\"#] mod normalize;\n",
            "# #[path = r#\"",
            env!("CARGO_MANIFEST_DIR"),
            "/src/audio_ingest.rs\"#] mod audio_ingest;\n",
            "# #[path = r#\"",
            env!("CARGO_MANIFEST_DIR"),
            "/src/playback_evidence/mod.rs\"#] mod playback_evidence;\n",
            "# #[path = r#\"",
            env!("CARGO_MANIFEST_DIR"),
            "/src/store.rs\"#] mod store;\n",
            "# #[path = r#\"",
            env!("CARGO_MANIFEST_DIR"),
            "/src/write_guard.rs\"#] mod write_guard;\n",
            $body,
            "\n",
            "```",
            "\n"
        )
    };
}

/// Owns recording, device playback evidence, verified revisions and local work.
#[doc = evidence_example!("A sibling submits only the immutable request authorized by runtime output.", "compile_fail,E0308", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = store.mutate(request);\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("A sibling submits only the immutable request authorized by runtime output.", "", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = store.mutate(playbacks.authorize_review(request, Some(&session)).unwrap());\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("A sibling cannot construct private request authority.", "compile_fail,E0451", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = super::playback_evidence::PlaybackProof { request };\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("A sibling cannot construct private request authority.", "", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = playbacks.authorize_review(request, Some(&session)).unwrap();\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("A sibling cannot construct proof-free authorization.", "compile_fail,E0451", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = super::playback_evidence::AuthorizedReview { authority: todo!() };\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("A sibling cannot construct proof-free authorization.", "", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = playbacks.authorize_review(request, Some(&session)).unwrap();\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("Playback state has no default proof constructor.", "compile_fail,E0599", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = super::playback_evidence::Playbacks::default();\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("Playback state has no default proof constructor.", "", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = super::playback_evidence::Playbacks::new();\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("Only the output module can construct a played run.", "compile_fail,E0603", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = super::playback_evidence::output::renderer::PlayedRun { start: 0, end: 16 };\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("Only the output module can construct a played run.", "", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = playbacks.status().unwrap();\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("Only the output module can construct or mutate listening coverage.", "compile_fail,E0603", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = super::playback_evidence::output::heard::Heard { len: 16, runs: vec![0..16], broken: false };\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("Only the output module can construct or mutate listening coverage.", "", "mod sibling {\nfn checked(store: &mut super::store::Store, request: &seamone_core::review::Request) {\nlet playbacks = super::playback_evidence::Playbacks::new();\nlet session = playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision).unwrap().session;\nlet _ = playbacks.status().unwrap();\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("A sibling receives only completed writes and cannot retain a private SQL permit.", "compile_fail,E0603", "mod sibling {\nfn checked(connection: &rusqlite::Transaction<'_>) {\nlet _: Option<super::write_guard::WritePermit<'_>> = None;\n}\n}\n# fn main() {}")]
#[doc = evidence_example!("A sibling receives only completed writes and cannot retain a private SQL permit.", "", "mod sibling {\nfn checked(connection: &rusqlite::Transaction<'_>) {\nlet writes = super::write_guard::WriteAccess::attach(connection).unwrap();\nlet _: () = writes.clear_verification(connection, Vec::new()).unwrap();\n}\n}\n# fn main() {}")]
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
            #[cfg(test)]
            listening: playback_evidence::Playbacks::fake(),
            #[cfg(not(test))]
            listening: playback_evidence::Playbacks::new(),
        }
    }
    fn store(&self) -> Result<MutexGuard<'_, Store>, String> {
        self.store
            .lock()
            .map_err(|_| "보관함을 다시 여세요.".into())
    }

    /// # Errors
    /// 현재 조각이나 revision이 없거나 음성, 출력 장치, 장치 지연을 읽지 못하면 오류를 돌려준다
    pub fn playback(&self, id: &str, revision: u64) -> Result<PlaybackReply, String> {
        let store = self.store()?;
        self.listening.prepare_review(store, id, revision)
    }

    /// # Errors
    /// session이나 위치가 유효하지 않거나 장치 조작이 실패하면 오류를 돌려준다
    pub fn control_playback(
        &self,
        session: &str,
        action: seamone_core::playback::Action,
    ) -> Result<seamone_core::playback::Status, String> {
        self.listening.control(session, action)
    }

    /// # Errors
    /// 재생 상태 lock을 얻지 못하면 오류를 돌려준다
    pub fn playback_status(&self) -> Result<seamone_core::playback::Status, String> {
        self.listening.status()
    }

    /// # Errors
    /// 녹음 revision, 재생 범위나 원본이 유효하지 않거나 장치를 열지 못하면 오류를 돌려준다
    pub fn play_recording(
        &self,
        id: &str,
        revision: u64,
        range: SampleRange,
    ) -> Result<seamone_core::playback::Status, String> {
        let store = self.store()?;
        self.listening.play_recording(store, id, revision, range)
    }

    /// # Errors
    /// 재생 증거, revision, 검수 전이, 전사문이 유효하지 않거나 저장에 실패하면 오류를 돌려준다
    pub fn review(&self, request: &Request, proof: Option<&str>) -> Result<Segment, String> {
        if let Some(replayed) = self.store()?.replay(request)? {
            return Ok(replayed);
        }
        let authorized = self.listening.authorize_review(request, proof);
        let mut store = self.store()?;
        if let Some(replayed) = store.replay(request)? {
            return Ok(replayed);
        }
        let saved = store.mutate(authorized?)?;
        drop(store);
        if saved.verified.is_some() {
            self.background.notify();
        }
        Ok(saved)
    }
}

impl LibraryService {
    /// # Errors
    /// 보관함 경로 생성, DB 열기나 migration이 실패하면 오류를 돌려준다
    pub fn open(root: &Path) -> Result<Self, String> {
        Store::open(root).map(Self::new)
    }

    #[must_use]
    pub const fn inference_available() -> bool {
        inference::available()
    }

    /// # Errors
    /// 보관함 lock이나 녹음 목록 조회가 실패하면 오류를 돌려준다
    pub fn library(&self) -> Result<LibraryPage, String> {
        self.store()?.library(Self::inference_available())
    }
    /// # Errors
    /// 보관함 lock이나 해당 녹음의 조각 조회가 실패하면 오류를 돌려준다
    pub fn segments(&self, id: &str, offset: u32) -> Result<SegmentPage, String> {
        self.store()?.segment_page(id, offset)
    }
    /// # Errors
    /// 보관함 lock이나 해당 녹음의 오류 구간 조회가 실패하면 오류를 돌려준다
    pub fn error_marks(&self, id: &str, offset: u32) -> Result<ErrorMarkPage, String> {
        self.store()?.error_mark_page(id, offset)
    }
    /// # Errors
    /// 오류 구간, 녹음 revision이 유효하지 않거나 저장에 실패하면 오류를 돌려준다
    pub fn save_error_mark(&self, mark: &ErrorMark) -> Result<(), String> {
        self.store()?.save_error_mark(mark)
    }
    /// # Errors
    /// batch나 오류 구간, 녹음 revision이 유효하지 않거나 저장에 실패하면 오류를 돌려준다
    pub fn save_error_marks(&self, batch: &ErrorMarkBatch) -> Result<(), String> {
        self.store()?.save_error_marks(batch)
    }
    /// # Errors
    /// 보관함 lock이나 안내 확인 설정 저장이 실패하면 오류를 돌려준다
    pub fn dismiss_info(&self) -> Result<(), String> {
        self.store()?.set_setting("info_seen", "1")
    }
    /// # Errors
    /// 보관함 lock이나 녹음의 전사문 조회가 실패하면 오류를 돌려준다
    pub fn export_text(&self, id: &str) -> Result<String, String> {
        self.store()?.export_text(id)
    }
    /// # Errors
    /// 녹음 revision이나 재생 범위가 유효하지 않거나 원본 읽기가 실패하면 오류를 돌려준다
    pub fn recording_playback(
        &self,
        id: &str,
        revision: u64,
        range: SampleRange,
    ) -> Result<String, String> {
        let store = self.store()?;
        let recording = store.recording(id)?;
        if recording.revision != revision {
            return Err(Error::StaleRevision.to_string());
        }
        let wav = store.audio.range_wav(id, range, recording.samples)?;
        Ok(STANDARD.encode(wav))
    }
    /// # Errors
    /// 파일 접근, 디코딩, 크기와 길이 검사, 저장에 실패하거나 commit이 취소되면 오류를 돌려준다
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
    /// # Errors
    /// 파일 디코딩, 크기와 길이 검사, 저장에 실패하거나 commit이 취소되면 오류를 돌려준다
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
    /// # Errors
    /// 스트림 복사, 크기와 길이 검사, 디코딩, 저장에 실패하거나 commit이 취소되면 오류를 돌려준다
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
    /// # Errors
    /// 이 플랫폼에서 엔진을 사용할 수 없거나 모델 읽기와 검증이 실패하면 오류를 돌려준다
    pub fn select_model(&self, path: &Path) -> Result<(), String> {
        inference::select(self, path)
    }
    /// # Errors
    /// 엔진이나 녹음이 없거나 이미 받아썼거나 디코딩, 추론, revision 검사, 저장에 실패하면 오류를 돌려준다
    pub fn transcribe(&self, id: &str) -> Result<Vec<Segment>, String> {
        inference::transcribe(self, id)
    }

    #[must_use]
    pub fn capture_status(&self) -> Status {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.recorder.status()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Status::Unavailable
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    /// # Errors
    /// 마이크 권한, 입력 장치, 설정, 녹음 세션 시작이 실패하면 오류를 돌려준다
    pub fn start_recording(&self) -> Result<Status, String> {
        let library = self.store()?.audio.clone();
        self.recorder
            .start(library, self.background.foreground.enter())
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    /// # Errors
    /// 녹음 id가 다르거나 입력, 길이 검사, 파일과 DB 저장이 실패하면 오류를 돌려준다
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
    pub fn close(&self) {
        self.listening.close();
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        self.recorder.close();
    }
}

#[cfg(test)]
mod service_tests;
