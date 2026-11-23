use crate::{api, quick_review::QuickReview};
use leptos::{prelude::*, task::spawn_local};
use seamone_core::{
    error_marks::{ErrorMark, ErrorMarkPage},
    review::{
        LibraryPage, PlaybackReply, RecordingInfo, ReviewAction, ReviewRequest, ReviewSegment,
    },
    review_flow::{ReviewFlow, ReviewMode, Ticket},
};
use serde_json::json;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct Playback {
    pub ticket: Ticket,
    pub token: String,
    pub url: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionState {
    Idle,
    SelectingModel,
    Running,
    Failed,
}

#[derive(Clone, PartialEq, Eq)]
pub enum LibraryState {
    Loading,
    Ready,
    Failed(String),
}

#[derive(Clone, Copy)]
pub struct Workspace {
    pub library: RwSignal<Option<LibraryPage>>,
    pub library_state: RwSignal<LibraryState>,
    library_request: RwSignal<u64>,
    pub saved_recording: RwSignal<Option<RecordingInfo>>,
    pub recording: RwSignal<Option<String>>,
    pub segments: RwSignal<Vec<ReviewSegment>>,
    pub marks: RwSignal<Vec<ErrorMark>>,
    pub quick: QuickReview,
    pub flow: RwSignal<ReviewFlow>,
    pub draft: RwSignal<String>,
    pub playback: RwSignal<Option<Playback>>,
    pub proof: RwSignal<Option<String>>,
    pub busy: RwSignal<bool>,
    pub transcription: RwSignal<TranscriptionState>,
    pub review_loaded: RwSignal<bool>,
    completed_recordings: RwSignal<HashSet<String>>,
    pub error: RwSignal<String>,
    pub status: RwSignal<String>,
    pub show_info: RwSignal<bool>,
    info_dismissed: RwSignal<bool>,
    navigation: RwSignal<u64>,
}

async fn load_segments(recording: &str) -> Result<Vec<ReviewSegment>, String> {
    let mut offset = 0;
    let mut segments = Vec::new();
    loop {
        let page = api::call::<seamone_core::review::SegmentPage>(
            "get_segments",
            json!({"recordingId":recording,"offset":offset}),
        )
        .await?;
        segments.extend(page.segments);
        let Some(next) = page.next_offset else {
            return Ok(segments);
        };
        if next <= offset || next > 10_000 {
            return Err("목록 위치가 잘못됐습니다.".into());
        }
        offset = next;
    }
}

impl Workspace {
    pub fn new() -> Self {
        let error = RwSignal::new(String::new());
        let status = RwSignal::new(String::new());
        Self {
            library: RwSignal::new(None),
            library_state: RwSignal::new(LibraryState::Loading),
            library_request: RwSignal::new(0),
            saved_recording: RwSignal::new(None),
            recording: RwSignal::new(None),
            segments: RwSignal::new(Vec::new()),
            marks: RwSignal::new(Vec::new()),
            quick: QuickReview::new(error, status),
            flow: RwSignal::new(ReviewFlow::default()),
            draft: RwSignal::new(String::new()),
            playback: RwSignal::new(None),
            proof: RwSignal::new(None),
            busy: RwSignal::new(false),
            transcription: RwSignal::new(TranscriptionState::Idle),
            review_loaded: RwSignal::new(false),
            completed_recordings: RwSignal::new(HashSet::new()),
            error,
            status,
            show_info: RwSignal::new(false),
            info_dismissed: RwSignal::new(false),
            navigation: RwSignal::new(0),
        }
    }

    pub fn refresh(self) {
        self.library_request.update(|n| *n = n.wrapping_add(1));
        let request = self.library_request.get_untracked();
        self.library_state.set(LibraryState::Loading);
        spawn_local(async move {
            let result = api::call::<LibraryPage>("get_library", json!({})).await;
            if self.library_request.get_untracked() != request {
                return;
            }
            match result {
                Ok(page) => {
                    if !page.info_seen && !self.info_dismissed.get_untracked() {
                        self.show_info.set(true);
                    }
                    self.library.set(Some(page));
                    self.library_state.set(LibraryState::Ready);
                }
                Err(e) => self.library_state.set(LibraryState::Failed(e)),
            }
        });
    }

    pub fn dismiss_info(self) {
        self.info_dismissed.set(true);
        self.show_info.set(false);
        spawn_local(async move {
            if let Err(error) = api::call::<()>("dismiss_info", json!({})).await {
                self.error.set(error);
            }
        });
    }

    pub fn needs_intro(self) -> bool {
        !self.info_dismissed.get()
            && self
                .library
                .with(|page| page.as_ref().is_some_and(|page| !page.info_seen))
    }

    pub fn inference_available(self) -> bool {
        self.library
            .with(|page| page.as_ref().is_some_and(|page| page.inference_available))
    }

    pub fn selected_recording(self) -> Option<RecordingInfo> {
        let id = self.recording.get()?;
        self.library
            .with(|page| {
                page.as_ref()?
                    .recordings
                    .iter()
                    .find(|r| r.id == id)
                    .cloned()
            })
            .or_else(|| {
                self.saved_recording
                    .get()
                    .filter(|recording| recording.id == id)
            })
    }

    pub fn has_transcription(self) -> bool {
        self.recording
            .get()
            .is_some_and(|id| self.completed_recordings.with(|ids| ids.contains(&id)))
            || self.segments.with(|segments| !segments.is_empty())
            || self
                .selected_recording()
                .is_some_and(|r| r.engine != "아직 받아쓰지 않음")
    }

    pub fn can_transcribe(self) -> bool {
        !self.busy.get()
            && self.inference_available()
            && self.review_loaded.get()
            && self.recording.with(Option::is_some)
            && !self.has_transcription()
    }

    pub fn open(self, id: String) {
        if self.busy.get_untracked() {
            return;
        }
        self.exit();
        if self.recording.get_untracked().as_ref() != Some(&id) {
            self.transcription.set(TranscriptionState::Idle);
        }
        self.status.set(String::new());
        self.navigation.update(|n| *n = n.wrapping_add(1));
        let navigation = self.navigation.get_untracked();
        self.recording.set(Some(id.clone()));
        self.segments.set(Vec::new());
        self.marks.set(Vec::new());
        self.busy.set(true);
        self.error.set(String::new());
        self.load_review_data(id, navigation);
    }

    fn load_review_data(self, id: String, navigation: u64) {
        self.review_loaded.set(false);
        spawn_local(async move {
            let result =
                async { Ok::<_, String>((load_segments(&id).await?, load_marks(&id).await?)) }
                    .await;
            if self.navigation.get_untracked() != navigation {
                return;
            }
            self.busy.set(false);
            match result {
                Ok((segments, marks)) => {
                    self.review_loaded.set(true);
                    self.segments.set(segments);
                    self.marks.set(marks);
                }
                Err(e) => self.error.set(e),
            }
        });
    }

    pub fn refresh_quick_results(self) {
        let Some(id) = self.recording.get_untracked() else {
            return;
        };
        self.navigation.update(|n| *n = n.wrapping_add(1));
        self.busy.set(true);
        self.load_review_data(id, self.navigation.get_untracked());
    }

    pub fn open_quick_correction(self) {
        if self.busy.get_untracked()
            || self.quick.saving.get_untracked()
            || self.quick.narrowing.with_untracked(Option::is_some)
        {
            return;
        }
        self.quick.exit();
        self.status.set(String::new());
        self.start(ReviewMode::Correction, &self.segments.get_untracked());
    }

    pub fn import(self) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        self.error.set(String::new());
        self.status.set(String::new());
        spawn_local(async move {
            let result = api::call::<Option<RecordingInfo>>("import_audio", json!({})).await;
            self.busy.set(false);
            match result {
                Ok(Some(info)) => {
                    self.open_ingested(info);
                }
                Ok(None) => {}
                Err(e) => self.error.set(e),
            }
        });
    }

    pub fn open_ingested(self, info: RecordingInfo) {
        self.saved_recording.set(Some(info.clone()));
        self.refresh();
        let editing = self
            .flow
            .with_untracked(|flow| flow.frame().is_some() || flow.mode() != ReviewMode::Browse);
        if !self.busy.get_untracked()
            && !editing
            && self.quick.recording.with_untracked(Option::is_none)
        {
            self.open(info.id);
        }
    }

    pub fn transcribe(self) {
        if !untrack(|| self.can_transcribe()) {
            return;
        }
        let Some(id) = self.recording.get_untracked() else {
            return;
        };
        let navigation = self.navigation.get_untracked();
        self.busy.set(true);
        self.transcription.set(TranscriptionState::Running);
        self.error.set(String::new());
        self.status
            .set("기기 안에서 받아쓰는 중입니다. 녹음 길이에 따라 시간이 걸릴 수 있습니다.".into());
        spawn_local(async move {
            let result =
                api::call::<Vec<ReviewSegment>>("transcribe_recording", json!({"recordingId":id}))
                    .await;
            if self.navigation.get_untracked() != navigation {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(segments) => {
                    self.completed_recordings.update(|ids| {
                        ids.insert(id);
                    });
                    self.transcription.set(TranscriptionState::Idle);
                    self.review_loaded.set(true);
                    self.status.set(if segments.is_empty() {
                        "받아쓰기를 마쳤습니다. 인식된 조각은 0개입니다.".into()
                    } else {
                        format!(
                            "받아쓰기를 마쳤습니다. {}개 조각을 이 기기에 저장했습니다.",
                            segments.len()
                        )
                    });
                    self.segments.set(segments);
                    self.refresh();
                }
                Err(e) => {
                    self.transcription.set(TranscriptionState::Failed);
                    self.error.set(e);
                    self.status.set(
                        "오류 안내를 확인하세요. 모델 파일 문제라면 정상 파일을 선택한 뒤 같은 녹음에서 다시 시도하세요."
                            .into(),
                    );
                }
            }
        });
    }

    pub fn select_model(self) {
        if self.busy.get_untracked() || !untrack(|| self.inference_available()) {
            return;
        }
        let previous = self.transcription.get_untracked();
        self.busy.set(true);
        self.transcription.set(TranscriptionState::SelectingModel);
        self.error.set(String::new());
        self.status.set("모델 파일을 확인하는 중입니다.".into());
        spawn_local(async move {
            let result = api::call::<bool>("select_model", json!({})).await;
            self.busy.set(false);
            self.transcription.set(previous);
            match result {
                Ok(true) => self.status.set(
                    "공식 Whisper small 모델의 무결성을 확인했습니다. 선택한 녹음에서 받아쓰기를 시작하세요.".into(),
                ),
                Ok(false) => self.status.set("모델 파일 선택을 취소했습니다. 저장한 녹음은 그대로입니다.".into()),
                Err(e) => {
                    self.status.set(String::new());
                    self.error.set(e);
                }
            }
        });
    }

    pub fn start(self, mode: ReviewMode, segments: &[ReviewSegment]) {
        if self.busy.get_untracked() {
            return;
        }
        self.playback.set(None);
        self.proof.set(None);
        self.error.set(String::new());
        self.flow.update(|flow| flow.start(mode, segments));
        self.sync_draft();
    }

    pub fn skip(self) {
        let mut skipped = false;
        self.flow.update(|f| skipped = f.skip());
        if skipped {
            self.playback.set(None);
            self.proof.set(None);
            self.error.set(String::new());
            self.sync_draft();
            self.status
                .set("이 조각은 판정하지 않고 남겼습니다.".into());
        }
    }

    pub fn exit(self) {
        self.flow.update(ReviewFlow::exit);
        self.playback.set(None);
        self.proof.set(None);
        self.error.set(String::new());
    }

    pub fn quick_recording(self) -> Option<RecordingInfo> {
        self.selected_recording()
            .filter(|_| self.has_transcription())
    }

    pub fn start_quick(self) {
        if !self.busy.get_untracked()
            && let Some(recording) = untrack(|| self.quick_recording())
        {
            self.exit();
            self.quick.start(recording);
        }
    }

    pub fn confirm_quick(self) {
        self.quick
            .confirm(Callback::new(move |()| self.refresh_quick_results()));
    }

    pub fn leave_quick(self) {
        if self.quick.saving.get_untracked() {
            return;
        }
        self.quick.exit();
        if let Some(id) = self.recording.get_untracked() {
            self.open(id);
            self.refresh();
        }
    }

    pub fn current(self) -> Option<ReviewSegment> {
        let frame = self.flow.get().frame()?.clone();
        self.segments
            .get()
            .into_iter()
            .find(|s| s.id == frame.id && s.revision == frame.revision)
    }

    fn sync_draft(self) {
        self.draft
            .set(self.current().map_or_else(String::new, |s| s.text));
    }

    pub fn play(self) {
        let mut ticket = None;
        self.flow.update(|flow| ticket = flow.play());
        let Some(ticket) = ticket else {
            return;
        };
        self.proof.set(None);
        self.playback.set(None);
        self.error.set(String::new());
        spawn_local(async move {
            let result = api::call::<PlaybackReply>(
                "get_playback",
                json!({"segmentId":ticket.frame.id,"revision":ticket.frame.revision}),
            )
            .await;
            match result {
                Ok(reply) => {
                    let mut active = false;
                    self.flow.update(|flow| active = flow.loaded(&ticket));
                    if active {
                        self.playback.set(Some(Playback {
                            ticket,
                            token: reply.token,
                            url: format!("data:audio/wav;base64,{}", reply.wav_base64),
                        }));
                    }
                }
                Err(e) => self.playback_error(&ticket, e),
            }
        });
    }

    pub fn ended(self, playback: Playback) {
        // The DOM audio node captures its own ticket, including session and revision.
        if !self
            .flow
            .with_untracked(|flow| flow.playing(&playback.ticket))
        {
            return;
        }
        spawn_local(async move {
            match api::call::<()>("playback_ended", json!({"token":playback.token})).await {
                Ok(()) => {
                    let mut active = false;
                    self.flow
                        .update(|flow| active = flow.ended(&playback.ticket));
                    if active {
                        self.proof.set(Some(playback.token));
                    }
                }
                Err(e) => self.playback_error(&playback.ticket, e),
            }
        });
    }

    pub fn playback_error(self, ticket: &Ticket, message: String) {
        let mut active = false;
        self.flow
            .update(|flow| active = flow.playback_failed(ticket));
        if active {
            self.error.set(message);
            self.playback.set(None);
            self.proof.set(None);
        }
    }

    pub fn save(self, action: ReviewAction) {
        let Some(segment) = untrack(|| self.current()) else {
            return;
        };
        match &action {
            ReviewAction::CorrectAndConfirm { text } if !segment.can_confirm_correction(text) => {
                return;
            }
            // A saved-revision confirmation also requires an unchanged editor.
            ReviewAction::ConfirmCorrection
                if segment.text != self.draft.get_untracked()
                    || !segment.can_confirm_correction(&segment.text) =>
            {
                return;
            }
            _ => {}
        }
        let reset_rejected = matches!(action, ReviewAction::Invalidate)
            && segment.rejected_text_unchanged(&segment.text);
        let dismiss_mark = matches!(action, ReviewAction::DismissErrorMark);
        let confirms_correction = matches!(
            action,
            ReviewAction::ConfirmCorrection | ReviewAction::CorrectAndConfirm { .. }
        );
        let requires_listening = matches!(
            action,
            ReviewAction::Judge { .. }
                | ReviewAction::ConfirmCorrection
                | ReviewAction::CorrectAndConfirm { .. }
        );
        let mut ticket = None;
        self.flow
            .update(|flow| ticket = flow.submit(requires_listening));
        let Some(ticket) = ticket else {
            return;
        };
        self.playback.set(None);
        let advance = matches!(
            action,
            ReviewAction::Judge { .. }
                | ReviewAction::ConfirmCorrection
                | ReviewAction::CorrectAndConfirm { .. }
        );
        let request = ReviewRequest {
            event_id: api::event_id(),
            segment_id: ticket.frame.id.clone(),
            expected_revision: ticket.frame.revision,
            action,
        };
        let proof = self.proof.get_untracked();
        spawn_local(async move {
            let result =
                api::call::<ReviewSegment>("save_review", json!({"request":request,"proof":proof}))
                    .await;
            match result {
                Ok(saved) => {
                    let mut active = false;
                    self.flow.update(|flow| {
                        let removed_from_queue = dismiss_mark
                            && flow.mode() == ReviewMode::Correction
                            && !saved.needs_review();
                        active = flow.saved(&ticket, &saved, advance || removed_from_queue);
                    });
                    self.segments.update(|segments| {
                        if let Some(segment) = segments
                            .iter_mut()
                            .find(|s| s.id == saved.id && s.revision <= saved.revision)
                        {
                            *segment = saved.clone();
                        }
                    });
                    self.refresh();
                    if active {
                        self.proof.set(None);
                        self.sync_draft();
                        if reset_rejected && !saved.needs_review() {
                            self.start(ReviewMode::Binary, std::slice::from_ref(&saved));
                        }
                        self.status.set(
                            if dismiss_mark {
                                "이 조각의 오류 판정을 해제했습니다."
                            } else if confirms_correction {
                                "교정을 확정했습니다."
                            } else {
                                "기기에 저장했습니다."
                            }
                            .into(),
                        );
                    }
                }
                Err(e) => {
                    let mut active = false;
                    self.flow.update(|flow| active = flow.save_failed(&ticket));
                    if active {
                        self.error.set(e);
                        self.proof.set(None);
                    }
                }
            }
        });
    }

    pub fn export(self, copy: bool) {
        let Some(id) = self.recording.get_untracked() else {
            return;
        };
        spawn_local(async move {
            let result: Result<(), String> = if copy {
                api::call::<()>("copy_saved_text", json!({"recordingId":id})).await
            } else {
                api::call::<bool>("save_text_file", json!({"recordingId":id}))
                    .await
                    .map(|_| ())
            };
            match result {
                Ok(()) => self.status.set(
                    if copy {
                        "저장한 글을 복사했습니다."
                    } else {
                        "텍스트 저장 대화상자를 닫았습니다."
                    }
                    .into(),
                ),
                Err(e) => self.error.set(e),
            }
        });
    }
}

async fn load_marks(recording: &str) -> Result<Vec<ErrorMark>, String> {
    let mut offset = 0;
    let mut marks = Vec::new();
    loop {
        let page = api::call::<ErrorMarkPage>(
            "get_error_marks",
            json!({"recordingId":recording,"offset":offset}),
        )
        .await?;
        marks.extend(page.marks);
        let Some(next) = page.next_offset else {
            return Ok(marks);
        };
        if next <= offset {
            return Err("오류 구간 목록 위치가 잘못됐습니다.".into());
        }
        offset = next;
    }
}
