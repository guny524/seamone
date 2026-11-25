use crate::{api, timeline::ReviewTimeline};
use leptos::{prelude::*, task::spawn_local};
use seamone_core::{
    error_marks::{ErrorMark, ErrorMarkBatch, Half, MIN_PREVIEW_SAMPLES, Narrowing, candidate},
    ipc::Command,
    review::{RecordingInfo, ReviewSegment, SAMPLE_RATE, SampleRange},
};
use serde_json::json;
use std::time::Duration;
use web_sys::HtmlAudioElement;

const SCAN_CHUNK: u64 = 30 * SAMPLE_RATE;

#[derive(Clone, Debug)]
struct Clip {
    id: u64,
    range: SampleRange,
    scan: bool,
    url: Option<String>,
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
    audio: StoredValue<Option<HtmlAudioElement>, LocalStorage>,
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
            audio: StoredValue::new_local(None),
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
        if let Some(audio) = self.audio.try_update_value(Option::take).flatten() {
            let _ = audio.pause();
        }
        let last_scan = self.clip.try_update(Option::take).flatten();
        if let Some(scan) = last_scan.filter(|clip| clip.scan && clip.url.is_some()) {
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
        let Some(recording) = self.recording.get_untracked() else {
            return;
        };
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
            url: None,
        }));
        spawn_local(async move {
            let result = api::call::<String>(
                Command::GetRecordingPlayback,
                json!({"recordingId":recording.id,"revision":recording.revision,"range":range}),
            )
            .await;
            if !self.is_current(id) {
                return;
            }
            match result {
                Ok(wav) => self.clip.set(Some(Clip {
                    id,
                    range,
                    scan,
                    url: Some(format!("data:audio/wav;base64,{wav}")),
                })),
                Err(error) => self.failed(id, error),
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

    fn playhead(self) -> u64 {
        self.clip.with_untracked(|clip| {
            clip.as_ref().filter(|clip| clip.scan).map_or_else(
                || self.cursor.get_untracked(),
                |clip| self.clip_position(clip.range),
            )
        })
    }

    fn clip_position(self, range: SampleRange) -> u64 {
        let elapsed = self.audio.with_value(|audio| {
            audio
                .as_ref()
                .and_then(|node| Duration::try_from_secs_f64(node.current_time()).ok())
                .map_or(0, |time| {
                    time.as_nanos() * u128::from(SAMPLE_RATE) / 1_000_000_000
                })
        });
        let span = range.end - range.start;
        range.start + u64::try_from(elapsed.min(u128::from(span))).unwrap_or(span)
    }

    pub fn flag(self, segments: RwSignal<Vec<ReviewSegment>>) {
        if self.narrowing.with_untracked(Option::is_some) {
            return;
        }
        let playhead = self.playhead();
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
        } else if self.clip.with_untracked(Option::is_none) {
            self.resume();
        }
    }

    fn load_analysis_audio(self, range: SampleRange) {
        let Some(recording) = self.recording.get_untracked() else {
            return;
        };
        self.analysis_wav.set(None);
        let guard = self.next_sequence();
        // Preview changes use their own clip IDs. The candidate stays fixed for every depth.
        self.analysis_request.set_value(guard);
        spawn_local(async move {
            let wav = api::call::<String>(
                Command::GetRecordingPlayback,
                json!({"recordingId":recording.id,"revision":recording.revision,"range":range}),
            )
            .await;
            if self.analysis_request.get_value() == guard
                && self.narrowing.with_untracked(Option::is_some)
            {
                self.analysis_wav.set(Some(wav));
            }
        });
    }

    pub fn skip(self, segments: RwSignal<Vec<ReviewSegment>>) {
        if self.saving.get_untracked() {
            return;
        }
        if self.narrowing.with_untracked(Option::is_some) {
            return;
        }
        let playhead = self.playhead();
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
        // Preserve the same request if the commit succeeds but its reply is lost.
        self.attempt.set(Some(batch.clone()));
        self.saving.set(true);
        self.stop_audio();
        self.error.set(String::new());
        spawn_local(async move {
            let result = api::call::<()>(Command::SaveErrorMarks, json!({"batch":batch})).await;
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

    fn ended(self, id: u64, node: &HtmlAudioElement) {
        if !node.ended() {
            return;
        }
        let Some((scan, end)) = self.clip.with_untracked(|clip| {
            let clip = clip.as_ref().filter(|clip| clip.id == id)?;
            Some((clip.scan, clip.range.end))
        }) else {
            return;
        };
        self.stop_audio();
        if scan {
            self.cursor.set(end);
            self.resume();
        }
    }

    fn failed(self, id: u64, message: String) {
        if self.is_current(id) {
            self.cursor.set(self.playhead());
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
    segments: RwSignal<Vec<ReviewSegment>>,
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
            {move || quick.clip.get().filter(|clip| clip.url.is_some()).map(|clip| view! { <QuickAudio quick clip/> })}
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
    segments: RwSignal<Vec<ReviewSegment>>,
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
    segments: RwSignal<Vec<ReviewSegment>>,
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
fn TranscriptContext(lines: Memo<Vec<ReviewSegment>>) -> impl IntoView {
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
    clip.url
        .as_deref()?
        .strip_prefix("data:audio/wav;base64,")
        .map(str::to_owned)
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
fn QuickAudio(quick: QuickReview, clip: Clip) -> impl IntoView {
    let audio = NodeRef::<leptos::html::Audio>::new();
    let id = clip.id;
    audio.on_load(move |node| {
        if quick.is_current(id) {
            quick.audio.set_value(Some(node));
        } else {
            let _ = node.pause();
        }
    });
    view! {
        <audio node_ref=audio src=clip.url autoplay=true aria-label="빠른 검수 음성"
            on:timeupdate=move |_| {
                if quick.is_current(id) {
                    let position = quick.clip_position(clip.range);
                    if clip.scan {
                        quick.cursor.set(position);
                    } else {
                        quick.preview_cursor.set(Some(position));
                    }
                }
            }
            on:ended=move |event| quick.ended(id, &event_target::<HtmlAudioElement>(&event))
            on:error=move |_| quick.failed(id,"음성을 재생하지 못했습니다.".into())/>
    }
}
