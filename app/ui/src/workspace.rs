use crate::api;
use leptos::{prelude::*, task::spawn_local};
use seamone_core::{
    review::{
        LibraryPage, PlaybackReply, RecordingInfo, ReviewAction, ReviewRequest, ReviewSegment,
    },
    review_flow::{ReviewFlow, ReviewMode, Ticket},
};
use serde_json::json;

#[derive(Clone, Debug)]
pub struct Playback {
    pub ticket: Ticket,
    pub token: String,
    pub url: String,
}

#[derive(Clone, Copy)]
pub struct Workspace {
    pub library: RwSignal<Option<LibraryPage>>,
    pub recording: RwSignal<Option<String>>,
    pub segments: RwSignal<Vec<ReviewSegment>>,
    pub flow: RwSignal<ReviewFlow>,
    pub draft: RwSignal<String>,
    pub playback: RwSignal<Option<Playback>>,
    pub proof: RwSignal<Option<String>>,
    pub busy: RwSignal<bool>,
    pub error: RwSignal<String>,
    pub status: RwSignal<String>,
    pub show_info: RwSignal<bool>,
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
        Self {
            library: RwSignal::new(None),
            recording: RwSignal::new(None),
            segments: RwSignal::new(Vec::new()),
            flow: RwSignal::new(ReviewFlow::default()),
            draft: RwSignal::new(String::new()),
            playback: RwSignal::new(None),
            proof: RwSignal::new(None),
            busy: RwSignal::new(false),
            error: RwSignal::new(String::new()),
            status: RwSignal::new(String::new()),
            show_info: RwSignal::new(false),
            navigation: RwSignal::new(0),
        }
    }

    pub fn refresh(self) {
        spawn_local(async move {
            match api::call::<LibraryPage>("get_library", json!({})).await {
                Ok(page) => {
                    if !page.info_seen {
                        self.show_info.set(true);
                    }
                    self.library.set(Some(page));
                }
                Err(e) => self.error.set(e),
            }
        });
    }

    pub fn open(self, id: String) {
        self.exit();
        self.navigation.update(|n| *n = n.wrapping_add(1));
        let navigation = self.navigation.get_untracked();
        self.recording.set(Some(id.clone()));
        self.segments.set(Vec::new());
        self.busy.set(true);
        self.error.set(String::new());
        spawn_local(async move {
            let result = load_segments(&id).await;
            if self.navigation.get_untracked() != navigation {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(segments) => self.segments.set(segments),
                Err(e) => self.error.set(e),
            }
        });
    }

    pub fn import(self) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        spawn_local(async move {
            let result = api::call::<Option<RecordingInfo>>("import_wav", json!({})).await;
            self.busy.set(false);
            match result {
                Ok(Some(info)) => {
                    self.refresh();
                    self.open(info.id);
                }
                Ok(None) => {}
                Err(e) => self.error.set(e),
            }
        });
    }

    pub fn transcribe(self) {
        if self.busy.get_untracked() {
            return;
        }
        let Some(id) = self.recording.get_untracked() else {
            return;
        };
        let navigation = self.navigation.get_untracked();
        self.busy.set(true);
        self.status.set("기기 안에서 받아쓰는 중입니다.".into());
        spawn_local(async move {
            let result =
                api::call::<Vec<ReviewSegment>>("transcribe_recording", json!({"recordingId":id}))
                    .await;
            self.busy.set(false);
            self.status.set(String::new());
            if self.navigation.get_untracked() != navigation {
                return;
            }
            match result {
                Ok(segments) => {
                    self.segments.set(segments);
                    self.refresh();
                }
                Err(e) => self.error.set(e),
            }
        });
    }

    pub fn select_model(self) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        spawn_local(async move {
            let result = api::call::<bool>("select_model", json!({})).await;
            self.busy.set(false);
            match result {
                Ok(true) => self
                    .status
                    .set("공식 Whisper small 모델의 무결성을 확인했습니다.".into()),
                Ok(false) => {}
                Err(e) => self.error.set(e),
            }
        });
    }

    pub fn start(self, mode: ReviewMode, segments: &[ReviewSegment]) {
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
        let flow = self.flow.get_untracked();
        if flow.frame() != Some(&playback.ticket.frame) || !flow.busy() {
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
        if let ReviewAction::CorrectAndConfirm { text } = &action
            && self
                .current()
                .is_none_or(|s| s.rejected_text_unchanged(text))
        {
            return;
        }
        let reset_rejected = matches!(action, ReviewAction::Invalidate)
            && self
                .current()
                .is_some_and(|s| s.rejected_text_unchanged(&s.text));
        let requires_listening = matches!(
            action,
            ReviewAction::Judge { .. }
                | ReviewAction::ConfirmCorrection
                | ReviewAction::CorrectAndConfirm { .. }
        );
        // Confirming a saved revision is impossible while the editor has unsaved changes.
        if matches!(action, ReviewAction::ConfirmCorrection)
            && self
                .current()
                .is_none_or(|s| s.text != self.draft.get_untracked())
        {
            return;
        }
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
                    self.flow
                        .update(|flow| active = flow.saved(&ticket, &saved, advance));
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
                        if reset_rejected {
                            self.start(ReviewMode::Binary, std::slice::from_ref(&saved));
                        }
                        self.refresh();
                        self.status.set("기기에 저장했습니다.".into());
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
