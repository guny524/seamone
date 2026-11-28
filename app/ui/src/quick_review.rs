use crate::{api, timeline::ReviewTimeline};
use leptos::{prelude::*, task::spawn_local};
use seamone_core::{
    error_marks::{ErrorMark, ErrorMarkBatch, Half, MIN_PREVIEW_SAMPLES, Narrowing, candidate},
    ipc::{ControlPlaybackArgs, EmptyArgs, RecordingPlaybackArgs, SaveErrorMarksArgs, command},
    playback::{Action as PlaybackAction, State as PlaybackState, Status as PlaybackStatus},
    review::{RecordingInfo, SAMPLE_RATE, SampleRange, Segment},
};
use std::time::Duration;

const SCAN_CHUNK: u64 = 30 * SAMPLE_RATE;

#[derive(Clone, Debug)]
struct Clip {
    id: u64,
    range: SampleRange,
    scan: bool,
    session: Option<String>,
    wav: Option<String>,
}

impl Clip {
    fn observe(&self, status: &PlaybackStatus) -> Result<(u64, bool), String> {
        if self.session.is_none() || status.session != self.session {
            return Err("재생이 바뀌었습니다. 현재 구간을 다시 여세요.".into());
        }
        if status.state == PlaybackState::Failed {
            return Err(status
                .error
                .clone()
                .unwrap_or_else(|| "음성을 재생하지 못했습니다.".into()));
        }
        if status.state == PlaybackState::Idle
            || !(self.range.start..=self.range.end).contains(&status.position)
            || (status.state == PlaybackState::Finished && status.position != self.range.end)
        {
            return Err("재생 위치가 현재 구간과 맞지 않습니다.".into());
        }
        Ok((status.position, status.state == PlaybackState::Finished))
    }
}

#[derive(Clone, Copy)]
pub struct QuickReview {
    pub recording: RwSignal<Option<RecordingInfo>>,
    pub narrowing: RwSignal<Option<Narrowing>>,
    pub saving: RwSignal<bool>,
    cursor: RwSignal<u64>,
    attempt: RwSignal<Option<ErrorMarkBatch>>,
    pub analysis_wav: RwSignal<Option<Result<String, String>>>,
    pub preview_cursor: RwSignal<Option<u64>>,
    heard_from: RwSignal<u64>,
    sequence: StoredValue<u64>,
    analysis_request: StoredValue<u64>,
    clip: RwSignal<Option<Clip>>,
    last_scan: RwSignal<Option<Clip>>,
    preparing: RwSignal<bool>,
    flagging: StoredValue<bool>,
    error: RwSignal<String>,
    status: RwSignal<String>,
}

impl QuickReview {
    pub fn new(error: RwSignal<String>, status: RwSignal<String>) -> Self {
        Self {
            recording: RwSignal::new(None),
            cursor: RwSignal::new(0),
            narrowing: RwSignal::new(None),
            attempt: RwSignal::new(None),
            analysis_wav: RwSignal::new(None),
            preview_cursor: RwSignal::new(None),
            saving: RwSignal::new(false),
            heard_from: RwSignal::new(0),
            sequence: StoredValue::new(0),
            analysis_request: StoredValue::new(0),
            clip: RwSignal::new(None),
            last_scan: RwSignal::new(None),
            preparing: RwSignal::new(false),
            flagging: StoredValue::new(false),
            error,
            status,
        }
    }

    pub fn start(self, recording: RecordingInfo) {
        self.exit();
        self.recording.set(Some(recording));
        self.cursor.set(0);
        self.heard_from.set(0);
        self.status.set(String::new());
        self.resume();
    }

    pub fn exit(self) {
        self.stop_audio();
        self.narrowing.set(None);
        self.analysis_wav.set(None);
        self.attempt.set(None);
        self.recording.set(None);
        self.last_scan.set(None);
    }

    fn stop_audio(self) {
        let previous = self.clip.try_update(Option::take).flatten();
        if let Some(clip) = &previous
            && let Some(session) = &clip.session
        {
            api::stop_playback(session.clone());
        }
        if let Some(scan) = previous.filter(|clip| clip.scan && clip.wav.is_some()) {
            self.last_scan.set(Some(scan));
        }
        self.preview_cursor.set(None);
    }

    fn resume(self) {
        let Some(recording) = self.recording.get_untracked() else {
            return;
        };
        let start = self.cursor.get_untracked();
        if start < recording.samples {
            self.play(
                SampleRange {
                    start,
                    end: (start + SCAN_CHUNK).min(recording.samples),
                },
                true,
            );
        }
    }

    fn play(self, range: SampleRange, scan: bool) {
        if self.recording.with_untracked(Option::is_none) {
            return;
        }
        self.stop_audio();
        self.error.set(String::new());
        let id = self.next_sequence();
        if !scan {
            self.preview_cursor.set(Some(range.start));
        }
        self.clip.set(Some(Clip {
            id,
            range,
            scan,
            session: None,
            wav: None,
        }));
        self.prepare_current();
    }

    fn prepare_current(self) {
        if self.preparing.get_untracked() {
            return;
        }
        let Some(recording) = self.recording.get_untracked() else {
            return;
        };
        let Some(clip) = self
            .clip
            .get_untracked()
            .filter(|clip| clip.session.is_none())
        else {
            return;
        };
        self.preparing.set(true);
        spawn_local(async move {
            let result = api::call_typed::<command::PlayRecording>(RecordingPlaybackArgs {
                recording_id: recording.id.clone(),
                revision: recording.revision,
                range: clip.range,
            })
            .await;
            if !self.is_current(clip.id) {
                if let Ok(status) = result
                    && let Some(session) = status.session
                {
                    let _ = api::call_typed::<command::ControlPlayback>(ControlPlaybackArgs {
                        session,
                        action: PlaybackAction::Stop,
                    })
                    .await;
                }
                self.preparing.set(false);
                self.prepare_current();
                return;
            }
            self.preparing.set(false);
            match result {
                Ok(status) => {
                    let mut ready = clip.clone();
                    ready.session.clone_from(&status.session);
                    match ready.observe(&status) {
                        Ok(_) => {
                            self.clip.set(Some(ready));
                            if clip.scan {
                                self.load_scan_wav(clip.id, recording, clip.range);
                            }
                        }
                        Err(error) => {
                            if let Some(session) = ready.session {
                                api::stop_playback(session);
                            }
                            self.failed(clip.id, error);
                        }
                    }
                }
                Err(error) => self.failed(clip.id, error),
            }
        });
    }

    fn load_scan_wav(self, id: u64, recording: RecordingInfo, range: SampleRange) {
        spawn_local(async move {
            let result = api::call_typed::<command::GetRecordingPlayback>(RecordingPlaybackArgs {
                recording_id: recording.id,
                revision: recording.revision,
                range,
            })
            .await;
            if self.is_current(id) {
                match result {
                    Ok(wav) => self.clip.update(|clip| {
                        if let Some(clip) = clip {
                            clip.wav = Some(wav);
                        }
                    }),
                    Err(error) => self.error.set(error),
                }
            }
        });
    }

    fn next_sequence(self) -> u64 {
        self.sequence
            .update_value(|sequence| *sequence = sequence.wrapping_add(1));
        self.sequence.get_value()
    }

    fn is_current(self, id: u64) -> bool {
        self.clip
            .with_untracked(|clip| clip.as_ref().is_some_and(|clip| clip.id == id))
    }

    pub fn preparation_pending(self) -> bool {
        self.preparing.get()
    }

    pub fn flag(self, segments: RwSignal<Vec<Segment>>) {
        if self.narrowing.with_untracked(Option::is_some) || self.flagging.get_value() {
            return;
        }
        let Some(clip) = self.clip.get_untracked() else {
            self.resume();
            return;
        };
        let Some(session) = clip.session.clone() else {
            self.stop_audio();
            self.flag_at(segments, self.cursor.get_untracked());
            return;
        };
        self.flagging.set_value(true);
        spawn_local(async move {
            let result = api::call_typed::<command::ControlPlayback>(ControlPlaybackArgs {
                session,
                action: PlaybackAction::Pause,
            })
            .await;
            self.flagging.set_value(false);
            if !self.is_current(clip.id) {
                return;
            }
            match result {
                Ok(status) => {
                    let checked = clip;
                    match checked.observe(&status) {
                        Ok((position, _)) => self.flag_at(segments, position),
                        Err(error) => self.failed(checked.id, error),
                    }
                }
                Err(error) => self.failed(clip.id, error),
            }
        });
    }

    fn flag_at(self, segments: RwSignal<Vec<Segment>>, playhead: u64) {
        let latest_line_start = segments.with_untracked(|segments| {
            segments
                .iter()
                .rev()
                .find(|line| line.range.start < playhead && line.range.start < line.range.end)
                .map(|line| line.range.start)
        });
        if let Some(range) = candidate(self.heard_from.get_untracked(), playhead, latest_line_start)
        {
            self.stop_audio();
            self.cursor.set(playhead);
            self.error.set(String::new());
            self.status.set(String::new());
            self.narrowing.set(Some(Narrowing::new(range)));
            self.load_analysis_audio(range);
        } else if let Some(session) = self
            .clip
            .with_untracked(|clip| clip.as_ref()?.session.clone())
        {
            spawn_local(async move {
                if let Err(error) =
                    api::call_typed::<command::ControlPlayback>(ControlPlaybackArgs {
                        session,
                        action: PlaybackAction::Play,
                    })
                    .await
                {
                    self.error.set(error);
                }
            });
        }
    }

    fn load_analysis_audio(self, range: SampleRange) {
        let Some(recording) = self.recording.get_untracked() else {
            return;
        };
        self.analysis_wav.set(None);
        let guard = self.next_sequence();

        self.analysis_request.set_value(guard);
        spawn_local(async move {
            let wav = api::call_typed::<command::GetRecordingPlayback>(RecordingPlaybackArgs {
                recording_id: recording.id,
                revision: recording.revision,
                range,
            })
            .await;
            if self.analysis_request.get_value() == guard
                && self.narrowing.with_untracked(Option::is_some)
            {
                self.analysis_wav.set(Some(wav));
            }
        });
    }

    pub fn skip(self, segments: RwSignal<Vec<Segment>>) {
        if self.saving.get_untracked() {
            return;
        }
        if self.narrowing.with_untracked(Option::is_some) {
            return;
        }
        let playhead = self.cursor.get_untracked();
        let next = segments.with_untracked(|segments| {
            segments
                .iter()
                .filter(|line| line.range.start < line.range.end)
                .find_map(|line| {
                    (line.range.end > playhead).then_some(if line.range.start > playhead {
                        line.range.start
                    } else {
                        line.range.end
                    })
                })
        });
        if let Some(recording) = self.recording.get_untracked() {
            let next = next.unwrap_or(recording.samples);
            self.restart_from(next);
        }
    }

    fn restart_from(self, position: u64) {
        self.stop_audio();
        self.cursor.set(position);
        self.heard_from.set(position);
        self.error.set(String::new());
        self.resume();
    }

    fn dismiss(self) {
        if self.saving.get_untracked() {
            return;
        }
        self.attempt.set(None);
        self.narrowing.set(None);
        self.analysis_wav.set(None);
        self.restart_from(self.cursor.get_untracked());
    }

    pub fn choose(self, half: Half) {
        self.change_selection(|search| search.choose(half));
    }

    pub fn descend(self) {
        self.change_selection(Narrowing::descend);
    }

    pub fn up(self) {
        self.change_selection(Narrowing::up);
    }

    pub fn cancel_step(self) {
        if self.saving.get_untracked() {
            return;
        }
        self.attempt.set(None);
        self.change_selection(Narrowing::cancel_step);
        if self.narrowing.with_untracked(|search| {
            search
                .as_ref()
                .is_some_and(|search| search.selection().is_none() && search.retained().is_empty())
        }) {
            self.dismiss();
        }
    }

    fn change_selection(self, change: impl FnOnce(&mut Narrowing)) {
        if self.attempt.with_untracked(Option::is_some) {
            return;
        }
        self.narrowing.update(|search| {
            if let Some(search) = search {
                change(search);
            }
        });
        self.stop_audio();
        self.error.set(String::new());
    }

    fn preview(self, half: Option<Half>) {
        if self.saving.get_untracked() {
            return;
        }
        let range = self.narrowing.with_untracked(|search| {
            let search = search.as_ref()?;
            match half {
                Some(half) => search.half(half),
                None => search.selection(),
            }
        });
        if let Some(range) = range {
            self.play(range, false);
        }
    }

    pub fn confirm(self, on_saved: Callback<()>) {
        if self.saving.get_untracked() {
            return;
        }
        let Some(recording) = self.recording.get_untracked() else {
            return;
        };
        let Some(batch) = self.attempt.get_untracked().or_else(|| {
            let ranges = self
                .narrowing
                .with_untracked(|search| search.as_ref().map(Narrowing::final_ranges))?;
            (!ranges.is_empty()).then(|| ErrorMarkBatch {
                id: api::event_id(),
                recording_id: recording.id,
                recording_revision: recording.revision,
                ranges,
            })
        }) else {
            self.dismiss();
            return;
        };

        self.attempt.set(Some(batch.clone()));
        self.saving.set(true);
        self.stop_audio();
        self.error.set(String::new());
        spawn_local(async move {
            let result =
                api::call_typed::<command::SaveErrorMarks>(SaveErrorMarksArgs { batch }).await;
            self.saving.set(false);
            match result {
                Ok(()) => {
                    self.dismiss();
                    self.status.set("오류 구간을 기기에 저장했습니다.".into());
                    on_saved.run(());
                }
                Err(error) => self.error.set(error),
            }
        });
    }

    fn observe(self, id: u64, status: &PlaybackStatus) {
        if !self.is_current(id) || self.flagging.get_value() {
            return;
        }
        let result = self.clip.with_untracked(|clip| {
            let clip = clip.as_ref()?;
            Some(
                clip.observe(status)
                    .map(|(position, finished)| (finished, clip.scan, position)),
            )
        });
        match result {
            Some(Ok((finished, scan, position))) => {
                if scan {
                    self.cursor.set(position);
                } else {
                    self.preview_cursor.set(Some(position));
                }
                if finished {
                    self.stop_audio();
                    if scan {
                        self.resume();
                    }
                }
            }
            Some(Err(error)) => self.failed(id, error),
            None => {}
        }
    }

    fn failed(self, id: u64, message: String) {
        if self.is_current(id) {
            self.stop_audio();
            self.error.set(message);
        }
    }
}

pub fn range_label(range: SampleRange) -> String {
    format!(
        "{} ~ {}",
        position_label(range.start),
        position_label(range.end)
    )
}

pub fn position_label(sample: u64) -> String {
    let seconds = sample / SAMPLE_RATE;
    let millis = sample % SAMPLE_RATE * 1000 / SAMPLE_RATE;
    format!("{}:{:02}.{millis:03}", seconds / 60, seconds % 60)
}

#[component]
pub fn QuickReviewPanel(
    quick: QuickReview,
    segments: RwSignal<Vec<Segment>>,
    marks: RwSignal<Vec<ErrorMark>>,
    on_confirm: Callback<()>,
    on_exit: Callback<()>,
) -> impl IntoView {
    view! {
        <section class="quick-review" aria-labelledby="quick-review-heading">
            <div class="review-header">
                <div>
                    <p class="review-mode" id="quick-review-heading">"빠른 검수"</p>
                    <h2>{move || quick.recording.with(|r| r.as_ref().map(|r| r.name.clone()))}</h2>
                </div>
                <div class="review-header-actions">
                    <p class="review-notice" role="status">{move || quick.status.get()}</p>
                    <button disabled=move || quick.saving.get() on:click=move |_| on_exit.run(())>"검수 나가기"</button>
                </div>
            </div>
            <QuickOutput quick/>
            <Show when=move || quick.narrowing.with(Option::is_some)
                fallback=move || view! { <ScanPanel quick segments marks/> }>
                <NarrowingPanel quick segments marks on_confirm/>
            </Show>
        </section>
    }
}

#[component]
fn ScanPanel(
    quick: QuickReview,
    segments: RwSignal<Vec<Segment>>,
    marks: RwSignal<Vec<ErrorMark>>,
) -> impl IntoView {
    let scan_lines = Memo::new(move |_| {
        let at = quick.cursor.get();
        let from = at
            .saturating_sub(2 * SAMPLE_RATE)
            .max(quick.heard_from.get());
        segments.with(|segments| {
            let mut recent = segments
                .iter()
                .rev()
                .filter(|line| {
                    line.range.start <= at
                        && line.range.end > from
                        && line.range.start < line.range.end
                })
                .take(2)
                .cloned()
                .collect::<Vec<_>>();
            recent.reverse();
            recent
        })
    });
    let scan_range = Memo::new(move |_| {
        let at = quick.cursor.get();
        quick
            .last_scan
            .with(|clip| {
                clip.as_ref()
                    .filter(|clip| clip.range.start <= at && at < clip.range.end)
                    .map(|clip| clip.range)
            })
            .or_else(|| {
                quick.clip.with(|clip| {
                    clip.as_ref()
                        .filter(|clip| clip.scan)
                        .map(|clip| clip.range)
                })
            })
    });
    let scan_wav = Signal::derive(move || {
        let range = scan_range.get()?;
        quick
            .clip
            .with(|clip| scan_audio(clip.as_ref(), range))
            .or_else(|| {
                quick
                    .last_scan
                    .with(|clip| scan_audio(clip.as_ref(), range))
            })
            .map(Ok)
    });
    let scan_cursor = Signal::derive(move || Some(quick.cursor.get()));
    view! {
        <div class="review-state">
            <h3>"이어듣기"</h3>
            <span class="position">{move || quick.recording.with(|r| r.as_ref().map(|r|
                format!("{} / {}", position_label(quick.cursor.get()), position_label(r.samples))))}</span>
        </div>
        {move || scan_range.get().map(|range| view! {
            <ReviewTimeline quick segments marks range wav=scan_wav cursor=scan_cursor searching=false/>
        })}
        <TranscriptContext lines=scan_lines />
        <Show when=move || quick.recording.with(|r| r.as_ref().is_some_and(|r| quick.cursor.get() == r.samples))>
            <p role="status">"녹음 끝"</p>
        </Show>
        <div class="toolbar review-controls scan-controls">
            <button class="primary" on:click=move |_| quick.flag(segments)>"오류 표시"<span class="desktop-shortcut">" (Space)"</span></button>
            <button on:click=move |_| quick.skip(segments)>"다음 조각"<span class="desktop-shortcut">" (N)"</span></button>
            <Show when=move || quick.clip.with(Option::is_none) && quick.recording.with(|r| r.as_ref().is_some_and(|r| quick.cursor.get() < r.samples))>
                <button on:click=move |_| quick.resume()>"재생 다시 시도"</button>
            </Show>
        </div>
    }
}

#[component]
fn NarrowingPanel(
    quick: QuickReview,
    segments: RwSignal<Vec<Segment>>,
    marks: RwSignal<Vec<ErrorMark>>,
    on_confirm: Callback<()>,
) -> impl IntoView {
    let candidate_range = Memo::new(move |_| {
        quick
            .narrowing
            .with(|search| search.as_ref().map(|search| search.path()[0]))
    });
    let preview_cursor = Signal::derive(move || quick.preview_cursor.get());
    let preview_wav = Signal::derive(move || quick.analysis_wav.get());
    view! {
        <div class="review-state">
            <h3>"오류 위치 좁히기"</h3>
            <p class="position" aria-live="polite">{move || quick.narrowing.with(|search| search.as_ref().map(|search| {
            search.selection().map_or_else(
                || "보관한 오류만 저장할 수 있습니다.".into(),
                |range| {
                    let caption = if search.confirmed_leaf() { "선택한 범위" } else { "회색 후보" };
                    format!("{caption} {}", range_label(range))
                },
            )
            }))}</p>
        </div>
        {move || candidate_range.get().map(|range| view! {
            <ReviewTimeline quick segments marks range wav=preview_wav cursor=preview_cursor searching=true/>
        })}
        <Show when=move || quick.narrowing.with(|search| search.as_ref().and_then(|s| s.half(Half::Front)).is_some())
            fallback=move || view! {
                <Show when=move || quick.narrowing.with(|s| s.as_ref().and_then(Narrowing::selection).is_some())>
                    <p>{move || if quick.narrowing.with(|s| s.as_ref().is_some_and(Narrowing::confirmed_leaf)) {
                        "이 범위를 저장할 수 있습니다.".into()
                    } else {
                        format!("각 절반을 최소 {}ms로 나눌 수 없어 현재 범위가 마지막 후보입니다.",
                            MIN_PREVIEW_SAMPLES * 1000 / SAMPLE_RATE)
                    }}</p>
                    <button disabled=move || quick.saving.get() on:click=move |_| quick.preview(None)>"이 범위 듣기"</button>
                </Show>
            }>
            <div class="error-halves">
                <ErrorHalf quick half=Half::Front/>
                <ErrorHalf quick half=Half::Back/>
            </div>
        </Show>
        <Show when=move || quick.attempt.with(Option::is_some) && !quick.saving.get()
            fallback=|| view! {
                <div class="review-controls key-guide">
                    <span><kbd>"←"</kbd> <kbd>"→"</kbd>" 후보 선택"</span>
                    <span><kbd>"Enter"</kbd>" 좁히기"</span>
                    <span><kbd>"↑"</kbd>" 보관 후 올라가기"</span>
                    <span><kbd>"Backspace"</kbd>" 선택 취소"</span>
                    <span><kbd>"Esc"</kbd>" 저장 후 이어듣기"</span>
                </div>
            }>
            <p>"저장을 다시 시도하거나 현재 선택을 취소할 수 있습니다."</p>
        </Show>
        <TouchNarrowingActions quick on_confirm/>
    }
}

#[component]
fn TouchNarrowingActions(quick: QuickReview, on_confirm: Callback<()>) -> impl IntoView {
    let can_descend = move || {
        quick.attempt.with(Option::is_none)
            && quick.narrowing.with(|search| {
                search
                    .as_ref()
                    .is_some_and(|s| s.selection().is_some() && !s.confirmed_leaf())
            })
    };
    view! {
        <div class="touch-narrowing" role="group" aria-label="오류 탐색 조작">
            <button disabled=move || !can_descend() on:click=move |_| quick.descend()>
                {move || if quick.narrowing.with(|search| search.as_ref().is_some_and(|s| s.half(Half::Front).is_none())) {
                    "이 범위 선택"
                } else {
                    "더 좁히기"
                }}
            </button>
            <button disabled=move || quick.attempt.with(Option::is_some) ||
                quick.narrowing.with(|search| search.as_ref().is_none_or(|s| s.path().len() < 2))
                on:click=move |_| quick.up()>"보관 후 올라가기"</button>
            <button disabled=move || quick.saving.get() on:click=move |_| quick.cancel_step()>"선택 취소"</button>
            <button class="primary" disabled=move || quick.saving.get() on:click=move |_| on_confirm.run(())>
                {move || if quick.saving.get() {
                    "저장 중"
                } else if quick.attempt.with(Option::is_some) {
                    "저장 다시 시도"
                } else if quick.narrowing.with(|search| search.as_ref().is_some_and(|s| !s.final_ranges().is_empty())) {
                    "저장 후 이어듣기"
                } else {
                    "이어듣기"
                }}
            </button>
        </div>
    }
}

#[component]
fn TranscriptContext(lines: Memo<Vec<Segment>>) -> impl IntoView {
    view! {
        <div class="transcript-context">
            <For each=move || lines.get() key=|line| (line.id.clone(), line.revision)
                children=|line| view! {
                    <div>
                        <small>{range_label(line.range)}</small>
                        <p class="transcript">{line.text}</p>
                    </div>
                }/>
        </div>
    }
}

fn scan_audio(clip: Option<&Clip>, range: SampleRange) -> Option<String> {
    let clip = clip.filter(|clip| clip.scan && clip.range == range)?;
    clip.wav.clone()
}

#[component]
fn ErrorHalf(quick: QuickReview, half: Half) -> impl IntoView {
    let (select, shortcut, listen) = match half {
        Half::Front => ("앞 절반 선택", " (←)", "앞 절반 듣기"),
        Half::Back => ("뒤 절반 선택", " (→)", "뒤 절반 듣기"),
    };
    view! {
        <div class="error-half">
            <p>{move || quick.narrowing.with(|s| s.as_ref().and_then(|s| s.half(half)).map(range_label))}</p>
            <button aria-pressed=move || quick.narrowing.with(|s| s.as_ref().is_some_and(|s| s.selection().is_some()
                    && s.selection() == s.half(half))).to_string()
                disabled=move || quick.attempt.with(Option::is_some) on:click=move |_| quick.choose(half)>
                {select}<span class="desktop-shortcut">{shortcut}</span>
            </button>
            <button disabled=move || quick.saving.get() on:click=move |_| quick.preview(Some(half))>{listen}</button>
        </div>
    }
}

#[component]
fn QuickOutput(quick: QuickReview) -> impl IntoView {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let alive = Arc::new(AtomicBool::new(true));
    let pending = Arc::new(AtomicBool::new(false));
    let active = Arc::clone(&alive);
    let poll = move || {
        if !active.load(Ordering::Relaxed) || quick.flagging.get_value() {
            return;
        }
        let Some(id) = quick.clip.with_untracked(|clip| {
            clip.as_ref()
                .filter(|clip| clip.session.is_some())
                .map(|clip| clip.id)
        }) else {
            return;
        };
        if pending.swap(true, Ordering::Relaxed) {
            return;
        }
        let pending = Arc::clone(&pending);
        let active = Arc::clone(&active);
        spawn_local(async move {
            let status = api::call_typed::<command::GetPlaybackStatus>(EmptyArgs {}).await;
            pending.store(false, Ordering::Relaxed);
            if !active.load(Ordering::Relaxed) {
                return;
            }
            match status {
                Ok(status) => quick.observe(id, &status),
                Err(error) => quick.failed(id, error),
            }
        });
    };
    let timer = set_interval_with_handle(poll, Duration::from_millis(40)).ok();
    on_cleanup(move || {
        alive.store(false, Ordering::Relaxed);
        if let Some(timer) = timer {
            timer.clear();
        }
        quick.stop_audio();
    });
    view! { <span hidden aria-hidden="true" data-runtime-quick-output/> }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct DeferredIo;

    impl any_spawner::CustomExecutor for DeferredIo {
        fn spawn(&self, _future: any_spawner::PinnedFuture<()>) {}
        fn spawn_local(&self, _future: any_spawner::PinnedLocalFuture<()>) {}
        fn poll_local(&self) {}
    }

    fn owner() -> Owner {
        let _ = any_spawner::Executor::init_custom_executor(DeferredIo);
        Owner::new()
    }

    fn segment() -> Segment {
        serde_json::from_value(json!({
            "id":"s","recording_id":"r","recording_revision":1,"revision":1,
            "range":{"start":473_600,"end":505_600},"machine_text":"글","text":"글",
            "verdict":null,"needs_correction":false,"verified":null
        }))
        .unwrap()
    }

    #[test]
    fn space_during_next_chunk_preparation_marks_the_last_confirmed_position() {
        owner().with(|| {
            let quick =
                QuickReview::new(RwSignal::new(String::new()), RwSignal::new(String::new()));
            quick.recording.set(Some(RecordingInfo {
                id: "r".into(),
                name: "음성".into(),
                samples: 960_000,
                revision: 1,
                engine: "test".into(),
                model: "test".into(),
            }));
            quick.cursor.set(480_000);
            quick.preparing.set(true);
            quick.clip.set(Some(Clip {
                id: 2,
                range: SampleRange {
                    start: 480_000,
                    end: 960_000,
                },
                scan: true,
                session: None,
                wav: None,
            }));
            let segments = RwSignal::new(vec![segment()]);
            quick.flag(segments);
            assert!(quick.narrowing.with_untracked(Option::is_some));
            assert_eq!(
                quick.narrowing.get_untracked().unwrap().path()[0],
                SampleRange {
                    start: 457_600,
                    end: 480_000
                }
            );
            assert!(quick.clip.with_untracked(Option::is_none));
            assert!(!quick.is_current(2));
            assert_eq!(quick.cursor.get_untracked(), 480_000);
            assert!(quick.preparation_pending());
        });
    }

    #[test]
    fn leaving_quick_review_blocks_other_playback_until_preparation_cleanup_finishes() {
        owner().with(|| {
            let workspace = crate::workspace::Workspace::new();
            workspace.flow.update(|flow| {
                flow.start(seamone_core::review_flow::ReviewMode::Binary, &[segment()]);
            });
            workspace.quick.preparing.set(true);
            workspace.quick.exit();
            assert!(workspace.quick.preparation_pending());
            workspace.play();
            assert!(!workspace.flow.get_untracked().busy());
            assert!(workspace.playback.with_untracked(Option::is_none));
            workspace.quick.preparing.set(false);
            assert!(!workspace.quick.preparation_pending());
        });
    }

    fn clip() -> Clip {
        Clip {
            id: 7,
            range: SampleRange {
                start: 32_000,
                end: 48_000,
            },
            scan: true,
            session: Some("preview".into()),
            wav: None,
        }
    }

    fn status(position: u64, state: PlaybackState) -> PlaybackStatus {
        PlaybackStatus {
            session: Some("preview".into()),
            position,
            state,
            heard: false,
            error: None,
        }
    }

    #[test]
    fn positions_are_absolute_source_samples_and_only_runtime_finish_advances() {
        let clip = clip();
        assert_eq!(
            clip.observe(&status(36_000, PlaybackState::Playing))
                .unwrap(),
            (36_000, false)
        );
        assert_eq!(
            clip.observe(&status(48_000, PlaybackState::Paused))
                .unwrap(),
            (48_000, false)
        );
        assert_eq!(
            clip.observe(&status(48_000, PlaybackState::Finished))
                .unwrap(),
            (48_000, true)
        );
    }

    #[test]
    fn stale_sessions_errors_and_positions_outside_the_clip_are_rejected() {
        let mut clip = clip();
        let mut stale = status(36_000, PlaybackState::Playing);
        stale.session = Some("old".into());
        let mut failed = status(36_000, PlaybackState::Failed);
        failed.error = Some("device disconnected".into());
        for wrong in [
            stale,
            status(31_999, PlaybackState::Playing),
            status(48_001, PlaybackState::Playing),
            status(40_000, PlaybackState::Finished),
            status(32_000, PlaybackState::Idle),
            failed,
        ] {
            assert!(clip.observe(&wrong).is_err());
        }
        clip.session = None;
        let mut missing = status(32_000, PlaybackState::Ready);
        missing.session = None;
        assert!(clip.observe(&missing).is_err());
    }

    #[test]
    fn a_failed_preview_keeps_the_last_position_without_republishing_it() {
        owner().with(|| {
            use std::sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            };
            let quick =
                QuickReview::new(RwSignal::new(String::new()), RwSignal::new(String::new()));
            quick.cursor.set(36_000);
            quick.clip.set(Some(clip()));
            let reads = Arc::new(AtomicUsize::new(0));
            let observed = Arc::clone(&reads);
            let position = Memo::new(move |_| {
                observed.fetch_add(1, Ordering::Relaxed);
                quick.cursor.get()
            });
            assert_eq!(position.get(), 36_000);
            quick.failed(7, "device disconnected".into());
            assert_eq!(position.get(), 36_000);
            assert_eq!(reads.load(Ordering::Relaxed), 1);
            assert!(quick.clip.with_untracked(Option::is_none));
            assert_eq!(quick.error.get_untracked(), "device disconnected");
        });
    }

    #[test]
    fn waveform_data_does_not_change_the_session_or_its_position() {
        let mut clip = clip();
        let recording_range = clip.range;
        let paused = status(36_000, PlaybackState::Paused);
        let before = clip.observe(&paused).unwrap();
        clip.wav = Some("waveform".into());
        assert_eq!(
            scan_audio(Some(&clip), recording_range),
            Some("waveform".into())
        );
        assert_eq!(clip.observe(&paused).unwrap(), before);
        assert_eq!(clip.session.as_deref(), Some("preview"));
        assert!(
            scan_audio(
                Some(&clip),
                SampleRange {
                    start: 0,
                    end: 16_000
                }
            )
            .is_none()
        );
        clip.scan = false;
        assert!(scan_audio(Some(&clip), recording_range).is_none());
    }
}
