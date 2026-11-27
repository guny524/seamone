use crate::{
    quick_review::{QuickReview, position_label, range_label},
    spectrum::{BINS, wav_spectrum},
};
use leptos::prelude::*;
use seamone_core::{
    TimedText,
    error_marks::{ErrorMark, overlaps},
    review::{SampleRange, Segment},
    sample_count_as_f64,
};
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, ImageData};

#[component]
pub fn ReviewTimeline(
    quick: QuickReview,
    segments: RwSignal<Vec<Segment>>,
    marks: RwSignal<Vec<ErrorMark>>,
    range: SampleRange,
    wav: Signal<Option<Result<String, String>>>,
    cursor: Signal<Option<u64>>,
    searching: bool,
) -> impl IntoView {
    let lines = Memo::new(move |_| {
        segments.with(|lines| {
            let mut visible = lines
                .iter()
                .filter(|line| overlaps(line.range, range))
                .cloned()
                .collect::<Vec<_>>();
            visible.sort_by_key(|line| line.range.start);
            visible
        })
    });
    view! {
        <div class=if searching { "sample-timeline candidate-timeline" } else { "sample-timeline scan-timeline" }
            data-start=range.start data-end=range.end>
            <TimeRuler range/>
            <RecordingSpectrum wav range empty="원본 음성을 읽는 중입니다."/>
            <AlignedTranscript lines range cursor highlight=!searching/>
            {move || searching.then(|| quick.narrowing.with(|search| search.as_ref().map(|search| {
                let retained = search.retained();
                let active_depth = search.depth();
                let deepest = retained.iter().map(|mark| mark.depth).max().unwrap_or(0).max(active_depth);
                (0..=deepest).map(|depth| {
                    let mut locked = search.path().get(depth).copied().into_iter().chain(
                        retained.iter().filter_map(|mark| mark.path.get(depth).copied())
                    ).collect::<Vec<_>>();
                    locked.sort_unstable_by_key(|span| (span.start, span.end));
                    locked.dedup();
                    let candidate = (depth == active_depth && (!search.confirmed_leaf() || depth == 0))
                        .then(|| search.selection()).flatten();
                    let confirmed = search.confirmed_leaf();
                    view! {
                        <div class="timeline-depth" data-depth=depth>
                            {locked.into_iter().map(|span| {
                                if depth == 0 { bar(range, span, "root", "처음 후보 범위") }
                                else { bar(range, span, "locked", "Enter로 고른 범위") }
                            }).collect_view()}
                            {candidate.map(|span| {
                                if confirmed { bar(range, span, "locked", "Enter로 고른 범위") }
                                else { bar(range, span, "candidate", "회색 후보") }
                            })}
                            {retained.iter().filter(|mark| mark.depth == depth).map(|mark| bar(range, mark.range, "retained", "보관한 오류")).collect_view()}
                        </div>
                    }
                }).collect_view()
            }))).flatten()}
            <Show when=move || !searching && marks.with(|marks| marks.iter().any(|mark| overlaps(mark.range, range)))>
                <div class="timeline-depth" aria-label="저장한 오류 구간">
                    {move || marks.with(|marks| marks.iter().filter(|mark| overlaps(mark.range, range))
                        .map(|mark| bar(range, mark.range, "retained", "저장한 오류")).collect_view())}
                </div>
            </Show>
            <Playhead range cursor/>
        </div>
        <Show when=move || searching && lines.with(|lines| !lines.is_empty())>
            <div class="timeline-context" aria-label="전사 줄 전체">
                <For each=move || lines.get() key=|line| (line.id.clone(), line.revision)
                    children=|line| view! {
                        <p><small>{range_label(line.range)}</small>" "{line.text}</p>
                    }/>
            </div>
        </Show>
    }
}

#[component]
pub(crate) fn CorrectionTimeline(
    range: SampleRange,
    revision: u64,
    marks: RwSignal<Vec<ErrorMark>>,
    wav: Signal<Option<Result<String, String>>>,
    cursor: Signal<Option<u64>>,
) -> impl IntoView {
    let ranges = Memo::new(move |_| {
        marks.with(|marks| {
            marks
                .iter()
                .filter(|mark| mark.recording_revision == revision && overlaps(mark.range, range))
                .map(|mark| mark.range)
                .collect::<Vec<_>>()
        })
    });
    view! {
        <div class="sample-timeline correction-timeline" data-start=range.start data-end=range.end>
            <TimeRuler range/>
            <RecordingSpectrum wav range empty="구간을 재생하면 스펙토그램이 표시됩니다."/>
            <Show when=move || !ranges.with(Vec::is_empty)>
                <div class="timeline-depth" aria-label="저장한 오류 구간">
                    {move || ranges.get().into_iter()
                        .map(|span| bar(range, span, "retained", "저장한 오류")).collect_view()}
                </div>
            </Show>
            <Playhead range cursor/>
        </div>
    }
}

#[component]
fn TimeRuler(range: SampleRange) -> impl IntoView {
    view! {
        <div class="timeline-times">
            <span>{position_label(range.start)}</span>
            {(1..4).map(|tick| view! {
                <span class="timeline-tick" style:left=format!("{}%",tick*25)>
                    {position_label(range.start + (range.end - range.start) * tick / 4)}
                </span>
            }).collect_view()}
            <span>{position_label(range.end)}</span>
        </div>
    }
}

#[component]
fn Playhead(range: SampleRange, cursor: Signal<Option<u64>>) -> impl IntoView {
    move || {
        cursor.get().map(|at| {
        view! { <div class="timeline-playhead" data-sample=at style=format!("left:{}%", percentage(range, at)) aria-hidden="true"/> }
    })
    }
}

#[component]
fn AlignedTranscript(
    lines: Memo<Vec<Segment>>,
    range: SampleRange,
    cursor: Signal<Option<u64>>,
    highlight: bool,
) -> impl IntoView {
    let rows = Memo::new(move |_| {
        let mut rows: Vec<Vec<Segment>> = Vec::new();
        for line in lines.get() {
            if let Some(row) = rows.iter_mut().find(|row| {
                row.last()
                    .is_some_and(|last| last.range.end <= line.range.start)
            }) {
                row.push(line);
            } else {
                rows.push(vec![line]);
            }
        }
        rows
    });
    view! {
        <div class="timeline-transcript" aria-label="원본 시각에 맞춘 전사">
            <For each=move || rows.get()
                key={|row| row.iter().map(|line| (line.id.clone(), line.revision)).collect::<Vec<_>>()}
                children=move |row| view! {
                    <div class="timeline-line">
                        {row.into_iter().flat_map(|line| {
                            let fallback = line.display_timing().is_none();
                            let spans = line.display_timing().map_or_else(
                                || vec![TimedText { range: line.range, text: line.text.clone() }],
                                <[TimedText]>::to_vec,
                            );
                            spans.into_iter().filter(move |span| overlaps(span.range, range)).map(move |span| {
                                let label = format!("{} · {}{}", span.text, range_label(span.range),
                                    if fallback { " · 전사 줄 시간" } else { "" });
                                view! {
                                    <span class=if fallback { "timeline-text line-time" } else { "timeline-text token-time" }
                                        class:active-token=move || highlight && cursor.get().is_some_and(|at|
                                            at >= span.range.start && at < span.range.end)
                                        data-start=span.range.start data-end=span.range.end
                                        style=span_style(range, span.range) title=label.clone() aria-label=label>{span.text}</span>
                                }
                            })
                        }).collect_view()}
                    </div>
                }/>
            <Show when=move || lines.with(Vec::is_empty)>
                <p>"이 범위에 전사문이 없습니다."</p>
            </Show>
        </div>
    }
}

#[component]
fn RecordingSpectrum(
    wav: Signal<Option<Result<String, String>>>,
    range: SampleRange,
    empty: &'static str,
) -> impl IntoView {
    let canvas = NodeRef::<leptos::html::Canvas>::new();
    let spectrum_error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        let Some(node) = canvas.get() else {
            return;
        };
        let Some(wav) = wav.get() else {
            node.set_width(1);
            spectrum_error.set(None);
            return;
        };
        let result = wav.and_then(|wav| {
            let spectrum = wav_spectrum(&wav, range.end - range.start)?;
            let width = u32::try_from(spectrum.columns).expect("limited spectrum width");
            let height = u32::try_from(BINS).expect("limited frequency bins");
            node.set_width(width);
            node.set_height(height);
            let render = || -> Option<()> {
                let context = node
                    .get_context("2d")
                    .ok()??
                    .dyn_into::<CanvasRenderingContext2d>()
                    .ok()?;
                let image = ImageData::new_with_u8_clamped_array_and_sh(
                    Clamped(&spectrum.rgba),
                    width,
                    height,
                )
                .ok()?;
                context.put_image_data(&image, 0.0, 0.0).ok()
            };
            render().ok_or_else(|| "스펙트로그램을 그리지 못했습니다.".into())
        });
        spectrum_error.set(result.err());
    });
    view! {
            <div class="timeline-spectrum">
                <canvas node_ref=canvas role="img" aria-label="원본 음성 스펙트로그램, 아래 0 Hz, 위 8 kHz"/>
                <span aria-hidden="true">"8 kHz"</span>
                <span aria-hidden="true">"0 Hz"</span>
                <p role="status">{move || spectrum_error.get().or_else(||
                    wav.with(|wav| wav.is_none().then(|| empty.into()))
                )}</p>
            </div>
    }
}

fn bar(viewport: SampleRange, span: SampleRange, kind: &str, name: &str) -> impl IntoView {
    let label = format!("{name} {}", range_label(span));
    view! {
        <span class=format!("timeline-bar {kind}") style=span_style(viewport, span)
            data-start=span.start data-end=span.end title=label.clone() aria-label=label/>
    }
}

fn span_style(viewport: SampleRange, span: SampleRange) -> String {
    let left = percentage(viewport, span.start);
    let right = percentage(viewport, span.end);
    format!("left:{left}%;width:{}%", right - left)
}

fn percentage(range: SampleRange, sample: u64) -> f64 {
    let offset = sample.clamp(range.start, range.end) - range.start;
    let duration = range.end - range.start;
    100.0 * sample_count_as_f64(offset) / sample_count_as_f64(duration)
}

#[cfg(test)]
mod tests {
    use super::percentage;
    use seamone_core::review::SampleRange;

    #[test]
    fn percentage_preserves_sample_positions_and_clamps_to_the_viewport() {
        let range = SampleRange {
            start: 16_000,
            end: 28_800_000,
        };
        for (sample, expected) in [
            (0, 0.0),
            (16_000, 0.0),
            (14_408_000, 50.0),
            (28_800_000, 100.0),
            (u64::MAX, 100.0),
        ] {
            assert_eq!(percentage(range, sample), expected);
        }
    }
}
