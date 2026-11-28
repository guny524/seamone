use crate::{api, quick_review::QuickReview};
use leptos::{prelude::*, task::spawn_local};
use seamone_core::{
    error_marks::ErrorMark,
    ipc::{
        ControlPlaybackArgs, EmptyArgs, RecordingArgs, RecordingPageArgs, SaveReview,
        SegmentPlaybackArgs, command,
    },
    playback::Action as PlaybackAction,
    review::{Action, LibraryPage, RecordingInfo, Request, Segment},
    review_flow::{ReviewFlow, ReviewMode, Ticket},
};

#[derive(Clone, Debug)]
pub struct Playback {
    pub ticket: Ticket,
    pub session: String,
    pub wav: String,
}

#[derive(Clone, Copy)]
pub struct Workspace {
    pub library: RwSignal<Option<LibraryPage>>,
    pub recording: RwSignal<Option<String>>,
    pub segments: RwSignal<Vec<Segment>>,
    pub marks: RwSignal<Vec<ErrorMark>>,
    pub quick: QuickReview,
    pub flow: RwSignal<ReviewFlow>,
    pub draft: RwSignal<String>,
    pub playback: RwSignal<Option<Playback>>,
    pub proof: RwSignal<Option<String>>,
    pub busy: RwSignal<bool>,
    pub error: RwSignal<String>,
    pub status: RwSignal<String>,
    pub show_info: RwSignal<bool>,
    info_dismissed: RwSignal<bool>,
    navigation: RwSignal<u64>,
}

async fn load_segments(recording: &str) -> Result<Vec<Segment>, String> {
    let mut offset = 0;
    let mut segments = Vec::new();
    loop {
        let page = api::call_typed::<command::GetSegments>(RecordingPageArgs {
            recording_id: recording.to_owned(),
            offset,
        })
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
            recording: RwSignal::new(None),
            segments: RwSignal::new(Vec::new()),
            marks: RwSignal::new(Vec::new()),
            quick: QuickReview::new(error, status),
            flow: RwSignal::new(ReviewFlow::default()),
            draft: RwSignal::new(String::new()),
            playback: RwSignal::new(None),
            proof: RwSignal::new(None),
            busy: RwSignal::new(false),
            error,
            status,
            show_info: RwSignal::new(false),
            info_dismissed: RwSignal::new(false),
            navigation: RwSignal::new(0),
        }
    }

    pub fn refresh(self) {
        spawn_local(async move {
            match api::call_typed::<command::GetLibrary>(EmptyArgs {}).await {
                Ok(page) => {
                    if !page.info_seen && !self.info_dismissed.get_untracked() {
                        self.show_info.set(true);
                    }
                    self.library.set(Some(page));
                }
                Err(e) => self.error.set(e),
            }
        });
    }

    pub fn dismiss_info(self) {
        self.info_dismissed.set(true);
        self.show_info.set(false);
        spawn_local(async move {
            if let Err(error) = api::call_typed::<command::DismissInfo>(EmptyArgs {}).await {
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

    pub fn open(self, id: String) {
        self.exit();
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
            let result = api::call_typed::<command::ImportAudio>(EmptyArgs {}).await;
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
        self.refresh();
        let editing = self
            .flow
            .with_untracked(|flow| flow.frame().is_some() || flow.mode() != ReviewMode::Browse);
        if !editing && self.quick.recording.with_untracked(Option::is_none) {
            self.open(info.id);
        }
        self.status
            .set(format!("{} · 녹음 보관함에 저장했습니다.", info.name));
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
                api::call_typed::<command::TranscribeRecording>(RecordingArgs { recording_id: id })
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
            let result = api::call_typed::<command::SelectModel>(EmptyArgs {}).await;
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

    pub fn start(self, mode: ReviewMode, segments: &[Segment]) {
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
        let id = self.recording.get();
        self.library.with(|page| {
            page.as_ref()?
                .recordings
                .iter()
                .find(|r| Some(&r.id) == id.as_ref() && r.engine != "아직 받아쓰지 않음")
                .cloned()
        })
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

    pub fn current(self) -> Option<Segment> {
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
        if untrack(|| self.quick.preparation_pending()) {
            return;
        }
        let mut ticket = None;
        self.flow.update(|flow| ticket = flow.play());
        let Some(ticket) = ticket else {
            return;
        };
        self.proof.set(None);
        self.playback.set(None);
        self.error.set(String::new());
        spawn_local(async move {
            let result = api::call_typed::<command::GetPlayback>(SegmentPlaybackArgs {
                segment_id: ticket.frame.id.clone(),
                revision: ticket.frame.revision,
            })
            .await;
            match result {
                Ok(reply) => {
                    if !self
                        .flow
                        .with_untracked(|flow| flow.frame() == Some(&ticket.frame))
                    {
                        Self::stop_session(reply.session);
                        return;
                    }
                    let played = api::call_typed::<command::ControlPlayback>(ControlPlaybackArgs {
                        session: reply.session.clone(),
                        action: PlaybackAction::Play,
                    })
                    .await;
                    let mut active = false;
                    if played.is_ok() {
                        self.flow.update(|flow| active = flow.loaded(&ticket));
                    }
                    if active {
                        self.playback.set(Some(Playback {
                            ticket,
                            session: reply.session,
                            wav: reply.wav_base64,
                        }));
                    } else {
                        Self::stop_session(reply.session);
                        if let Err(error) = played {
                            self.playback_error(&ticket, error);
                        }
                    }
                }
                Err(error) => self.playback_error(&ticket, error),
            }
        });
    }

    pub async fn control_playback(
        playback: &Playback,
        action: seamone_core::playback::Action,
    ) -> Result<seamone_core::playback::Status, String> {
        api::call_typed::<command::ControlPlayback>(ControlPlaybackArgs {
            session: playback.session.clone(),
            action,
        })
        .await
    }

    pub fn observe_playback(
        self,
        playback: &Playback,
        status: &seamone_core::playback::Status,
    ) -> bool {
        if !self.playback.with_untracked(|current| {
            current.as_ref().is_some_and(|current| {
                current.session == playback.session && current.ticket == playback.ticket
            })
        }) {
            return false;
        }
        if status.session.as_deref() != Some(playback.session.as_str())
            || status.state == seamone_core::playback::State::Failed
        {
            self.playback_error(
                &playback.ticket,
                status
                    .error
                    .clone()
                    .unwrap_or_else(|| "재생이 바뀌었습니다. 현재 조각을 다시 여세요.".into()),
            );
            return false;
        }
        if status.heard && !self.flow.with_untracked(ReviewFlow::listened) {
            let mut active = false;
            self.flow
                .update(|flow| active = flow.ended(&playback.ticket));
            if active {
                self.proof.set(Some(playback.session.clone()));
            }
        }
        true
    }

    pub fn stop_session(session: String) {
        api::stop_playback(session);
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

    pub fn save(self, action: Action) {
        let Some(segment) = untrack(|| self.current()) else {
            return;
        };
        match &action {
            Action::CorrectAndConfirm { text } if !segment.can_confirm_correction(text) => {
                return;
            }

            Action::ConfirmCorrection
                if segment.text != self.draft.get_untracked()
                    || !segment.can_confirm_correction(&segment.text) =>
            {
                return;
            }
            _ => {}
        }
        let reset_rejected =
            matches!(action, Action::Invalidate) && segment.rejected_text_unchanged(&segment.text);
        let dismiss_mark = matches!(action, Action::DismissErrorMark);
        let confirms_correction = matches!(
            action,
            Action::ConfirmCorrection | Action::CorrectAndConfirm { .. }
        );
        let submits_judgment = action.required_evidence().requires_playback();
        let mut ticket = None;
        self.flow
            .update(|flow| ticket = flow.submit(submits_judgment));
        let Some(ticket) = ticket else {
            return;
        };
        self.playback.set(None);
        let request = Request {
            event_id: api::event_id(),
            segment_id: ticket.frame.id.clone(),
            expected_revision: ticket.frame.revision,
            action,
        };
        let proof = self.proof.get_untracked();
        spawn_local(async move {
            let result =
                api::call_typed::<command::SaveReview>(SaveReview { request, proof }).await;
            match result {
                Ok(saved) => {
                    let mut active = false;
                    self.flow.update(|flow| {
                        let removed_from_queue = dismiss_mark
                            && flow.mode() == ReviewMode::Correction
                            && !saved.needs_review();
                        active =
                            flow.saved(&ticket, &saved, submits_judgment || removed_from_queue);
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
            let args = RecordingArgs { recording_id: id };
            let result: Result<(), String> = if copy {
                api::call_typed::<command::CopySavedText>(args).await
            } else {
                api::call_typed::<command::SaveTextFile>(args)
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
        let page = api::call_typed::<command::GetErrorMarks>(RecordingPageArgs {
            recording_id: recording.to_owned(),
            offset,
        })
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
