use crate::{
    capture::{Microphone, MicrophonePanel},
    onboarding::PrivacyInfo,
    quick_review::{QuickReviewPanel, range_label},
    timeline::CorrectionTimeline,
    workspace::{Playback, Workspace},
};
use leptos::{ev, prelude::*};
use seamone_core::{
    error_marks::{Half, overlaps},
    review::{ReviewAction, SAMPLE_RATE, Verdict},
    review_flow::{ReviewFlow, ReviewMode, Ticket},
};
use std::time::Duration;
use wasm_bindgen::JsCast;
use web_sys::HtmlAudioElement;

#[component]
pub fn App() -> impl IntoView {
    let workspace = Workspace::new();
    workspace.refresh();
    let browsing = Signal::derive(move || {
        workspace
            .flow
            .with(|flow| flow.frame().is_none() && flow.mode() == ReviewMode::Browse)
    });
    let show_capture_start =
        Signal::derive(move || browsing.get() && workspace.quick.recording.with(Option::is_none));
    let show_intro_actions =
        Signal::derive(move || show_capture_start.get() && workspace.needs_intro());
    let inference_available = Signal::derive(move || {
        workspace
            .library
            .with(|page| page.as_ref().is_some_and(|page| page.inference_available))
    });
    let microphone = Microphone::new(workspace.busy, workspace.error, workspace.status);
    let correcting = move || {
        workspace
            .flow
            .with(|flow| flow.mode() == ReviewMode::Correction)
    };
    register_review_shortcuts(&workspace);
    let leave_quick = Callback::new(move |()| workspace.leave_quick());
    view! {
        <div class="container">
            <header class="app-header">
                <h1>"Seamone"</h1>
                <button on:click=move |_| workspace.show_info.set(true)>"개인정보와 학습 안내"</button>
            </header>
            <Show when=move || workspace.show_info.get()>
                <PrivacyInfo microphone can_add=show_intro_actions import_disabled=workspace.busy inference_available
                    on_dismiss=Callback::new(move |()| workspace.dismiss_info())
                    on_import=Callback::new(move |()| workspace.import())/>
            </Show>
            <div class="app-layout" class:is-reviewing=move || workspace.quick.recording.with(Option::is_some)
                class:is-correcting=correcting>
                <aside class="library-pane" aria-labelledby="library-heading">
                    <MicrophonePanel microphone
                        browsing=show_capture_start on_saved=Callback::new(move |info| workspace.open_ingested(info))/>
                    <Library workspace browsing/>
                </aside>
                <main class="editor-pane">
                    <p class="error" role="alert">{move || workspace.error.get()}</p>
                    <Show when=move || workspace.quick.recording.with(Option::is_none)>
                        <p class="status" role="status">{move || workspace.status.get()}</p>
                    </Show>
                    <Show when=move || workspace.quick.recording.with(Option::is_some) fallback=move || view! {
                        <Show when=move || browsing.get()
                            fallback=move || view! { <ReviewPanel workspace/> }><SavedTranscript workspace/></Show>
                    }><QuickReviewPanel quick=workspace.quick segments=workspace.segments marks=workspace.marks
                        on_confirm=Callback::new(move |()| workspace.confirm_quick()) on_exit=leave_quick/></Show>
                </main>
                <Show when=move || workspace.quick.recording.with(Option::is_some)>
                    <CorrectionQueue workspace/>
                </Show>
                <Show when=correcting><CorrectionProgress workspace/></Show>
            </div>
        </div>
    }
}

fn register_review_shortcuts(workspace: &Workspace) {
    let workspace = *workspace;
    let swallow_space_release = StoredValue::new(false);
    let listener = window_event_listener(ev::keydown, move |event| {
        if event.is_composing() {
            return;
        }
        if workspace.quick.recording.with_untracked(Option::is_some) {
            quick_keydown(&workspace, &event, swallow_space_release);
            return;
        }
        if event.repeat() {
            return;
        }
        let key = event.key();
        let mode = workspace.flow.get_untracked().mode();
        if key == "Escape"
            && (mode != ReviewMode::Browse || workspace.flow.get_untracked().frame().is_some())
        {
            event.prevent_default();
            workspace.exit();
            return;
        }
        let editing = event
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|e| {
                matches!(e.tag_name().as_str(), "TEXTAREA" | "INPUT")
                    || e.get_attribute("contenteditable").is_some()
            });
        if mode == ReviewMode::Correction
            && key == "Enter"
            && (event.ctrl_key() || event.meta_key())
        {
            event.prevent_default();
            workspace.save(ReviewAction::CorrectAndConfirm {
                text: workspace.draft.get_untracked(),
            });
            return;
        }
        if editing || mode == ReviewMode::Browse {
            return;
        }
        match key.as_str() {
            "n" | "N" => {
                event.prevent_default();
                workspace.skip();
            }
            " " => {
                event.prevent_default();
                workspace.play();
            }
            "1" if mode == ReviewMode::Binary => {
                event.prevent_default();
                workspace.save(ReviewAction::Judge {
                    verdict: Verdict::Yes,
                });
            }
            _ => {}
        }
    });
    let keyup = window_event_listener(ev::keyup, move |event| {
        if event.key() == " " && swallow_space_release.get_value() {
            swallow_space_release.set_value(false);
            event.prevent_default();
        }
    });
    on_cleanup(move || {
        listener.remove();
        keyup.remove();
    });
}

fn quick_keydown(
    workspace: &Workspace,
    event: &web_sys::KeyboardEvent,
    swallow_space_release: StoredValue<bool>,
) {
    let key = event.key();
    let quick = workspace.quick;
    let narrowing = quick.narrowing.with_untracked(Option::is_some);
    if key == "Enter" && !narrowing {
        return;
    }
    if !matches!(
        key.as_str(),
        " " | "Enter" | "ArrowLeft" | "ArrowRight" | "ArrowUp" | "Backspace" | "Escape" | "n" | "N"
    ) {
        return;
    }
    // Reserved shortcuts take precedence over the focused button.
    event.prevent_default();
    if key == " " {
        swallow_space_release.set_value(true);
    }
    if event.repeat()
        || event.ctrl_key()
        || event.meta_key()
        || event.alt_key()
        || event.is_composing()
    {
        return;
    }
    match key.as_str() {
        " " if !narrowing => quick.flag(workspace.segments),
        "ArrowLeft" if narrowing => quick.choose(Half::Front),
        "ArrowRight" if narrowing => quick.choose(Half::Back),
        "Enter" if narrowing => quick.descend(),
        "ArrowUp" if narrowing => quick.up(),
        "Backspace" if narrowing => quick.cancel_step(),
        "n" | "N" if !narrowing => quick.skip(workspace.segments),
        "Escape" if narrowing => workspace.confirm_quick(),
        "Escape" => workspace.leave_quick(),
        _ => {}
    }
}

#[component]
fn Library(workspace: Workspace, browsing: Signal<bool>) -> impl IntoView {
    let locked = move || !browsing.get() || workspace.quick.recording.with(Option::is_some);
    view! {
        <section class="library" aria-labelledby="library-heading">
            <h2 id="library-heading">"녹음 보관함"</h2>
            <Show when=move || !locked()>
              <div class="library-actions">
                <button disabled=move || workspace.busy.get() on:click=move |_| workspace.import()>"음성 파일 가져오기"</button>
                <details>
                    <summary>"보관함 설정"</summary>
                    <button disabled=move || workspace.busy.get() on:click=move |_| workspace.select_model()>"Whisper 모델 선택"</button>
                    <button disabled=move || workspace.busy.get() on:click=move |_| workspace.refresh()>"보관함 새로 불러오기"</button>
                    <p>"WAV, MP3, AAC-LC M4A · 최대 30분, 512MiB"</p>
                    <p>{move || workspace.library.get().map(|page| format!("로컬 학습 대기 {}개 (원본 확인 재시도 {}개), 앱 내부 학습 엔진 준비 전",page.pending_training,page.retry_training))}</p>
                </details>
              </div>
            </Show>
            <ul class="recordings">
                <For each=move || workspace.library.get().map_or_else(Vec::new,|p| p.recordings) key=|r| r.id.clone()
                    children=move |r| {
                        let id = r.id.clone();
                        view! { <li><button class="recording-button" aria-label=r.name.clone()
                            aria-pressed=move || if workspace.recording.get().as_ref() == Some(&id) { "true" } else { "false" }
                            disabled=move || workspace.busy.get() || locked() on:click=move |_| workspace.open(r.id.clone())>
                            <span>{r.name.clone()}</span>
                            <small>{format!("{}초 · {}",r.samples/16_000,r.engine)}</small>
                        </button></li> }
                    }/>
            </ul>
        </section>
    }
}

#[component]
fn SavedTranscript(workspace: Workspace) -> impl IntoView {
    view! {
        <Show when=move || workspace.recording.with(Option::is_none)>
            <section class="empty-editor">
                <h2>"녹음을 열어 시작하세요"</h2>
                <p>"보관함에서 녹음을 선택하거나 새로 녹음하고 음성 파일을 가져올 수 있습니다."</p>
            </section>
        </Show>
        <Show when=move || workspace.recording.get().is_some()>
            <section aria-labelledby="transcript-heading">
                <h2 id="transcript-heading">"저장한 글"</h2>
                <div class="toolbar">
                    <button disabled=move || workspace.busy.get() on:click=move |_| workspace.transcribe()>"기기 안에서 받아쓰기"</button>
                    <button on:click=move |_| workspace.export(true)>"저장한 글 복사"</button>
                    <button on:click=move |_| workspace.export(false)>"텍스트 파일로 저장"</button>
                    <button disabled=move || workspace.busy.get() on:click=move |_| { if let Some(id) = workspace.recording.get_untracked() { workspace.open(id); } }>"수정본 새로 불러오기"</button>
                </div>
                <div class="toolbar">
                    <button class="primary" disabled=move || workspace.busy.get() || workspace.quick_recording().is_none() on:click=move |_| workspace.start_quick()>"빠른 검수 시작"</button>
                    <button disabled=move || workspace.busy.get() on:click=move |_| workspace.start(ReviewMode::Correction,&workspace.segments.get_untracked())>"오류와 수정본 교정 목록"</button>
                </div>
                <details>
                    <summary>"조각별 검증"</summary>
                    <button disabled=move || workspace.busy.get() on:click=move |_| workspace.start(ReviewMode::Binary,&workspace.segments.get_untracked())>"자동 전사문 Yes/No 검증"</button>
                </details>
                <Show when=move || workspace.marks.with(|marks| !marks.is_empty())>
                    <h3>"표시한 오류 구간"</h3>
                    <ul>
                        <For each=move || workspace.marks.get() key=|mark| mark.id.clone()
                            children=move |mark| view! {
                                <li>{range_label(mark.range)}{move || workspace.segments.with(|segments| {
                                    (!segments.iter().any(|s| s.recording_revision == mark.recording_revision &&
                                        overlaps(s.range, mark.range))).then_some(" · 겹치는 전사 조각 없음")
                                })}</li>
                            }/>
                    </ul>
                </Show>
                <p>"검증을 건너뛰어도 복사와 내보내기를 쓸 수 있습니다. 편집은 초안으로 저장되며 별도 확인 전에는 학습 정답이 아닙니다."</p>
                <ol class="segments">
                    <For each=move || workspace.segments.get() key=|s| (s.id.clone(),s.revision)
                        children=move |s| {
                            let text = s.text.clone();
                            let state = if s.verified.is_some() { "사람이 확인한 정답" } else if s.needs_review() { "수정본 확인 필요" } else { "미검증" };
                            let verdict = match s.verdict { Some(Verdict::Yes) => "자동 전사문 판정: Yes", Some(Verdict::No) => "자동 전사문 판정: No", None => "자동 전사문 판정 없음" };
                            view! { <li><p class="transcript">{text}</p><p>{state}" / "{verdict}</p><button on:click=move |_| workspace.start(ReviewMode::Browse,std::slice::from_ref(&s))>"이 조각 재생, 편집"</button></li> }
                        }/>
                </ol>
            </section>
        </Show>
    }
}

#[component]
fn CorrectionQueue(workspace: Workspace) -> impl IntoView {
    let pending = Memo::new(move |_| {
        workspace.marks.with(|marks| {
            workspace.segments.with(|segments| {
                marks
                    .iter()
                    .filter(|mark| {
                        segments.iter().any(|segment| {
                            segment.awaits_correction()
                                && segment.recording_revision == mark.recording_revision
                                && overlaps(segment.range, mark.range)
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
        })
    });
    view! {
        <aside class="correction-pane" aria-labelledby="correction-queue-heading">
            <h2 id="correction-queue-heading">"교정 대기"</h2>
            <Show when=move || pending.with(Vec::is_empty) fallback=move || view! {
                <p class="queue-count">{move || format!("오류 {}개",pending.with(Vec::len))}</p>
                <ol class="correction-marks">
                    <For each=move || pending.get() key=|mark| mark.id.clone() children=|mark| view! {
                        <li><span>{range_label(mark.range)}</span></li>
                    }/>
                </ol>
                <button disabled=move || workspace.busy.get() || workspace.quick.saving.get()
                    || workspace.quick.narrowing.with(Option::is_some) on:click=move |_| workspace.open_quick_correction()>
                    "교정 목록 열기"
                </button>
            }>
                <p>"교정을 기다리는 오류 구간이 없습니다."</p>
            </Show>
        </aside>
    }
}

#[component]
fn CorrectionProgress(workspace: Workspace) -> impl IntoView {
    view! {
        <aside class="correction-pane" aria-labelledby="correction-progress-heading">
            <h2 id="correction-progress-heading">"교정할 조각"</h2>
            <p>{move || format!("남은 조각 {}개",workspace.flow.with(ReviewFlow::remaining))}</p>
            <ol class="correction-marks">
                <For each=move || workspace.flow.with(|flow| flow.frames().to_vec())
                    key=|frame| (frame.id.clone(), frame.revision) children=move |frame| {
                        let active_frame = frame.clone();
                        let label = move || workspace.segments.with(|segments| segments.iter().find(|s|
                            s.id == frame.id && s.revision == frame.revision).map(|s| range_label(s.range)));
                        view! { <li aria-current=move || workspace.flow.with(|flow|
                            flow.frame() == Some(&active_frame)).to_string()>{label}</li> }
                    }/>
            </ol>
        </aside>
    }
}

#[component]
fn ReviewPanel(workspace: Workspace) -> impl IntoView {
    let current = Memo::new(move |_| workspace.current());
    let correcting = move || {
        workspace
            .flow
            .with(|flow| flow.mode() == ReviewMode::Correction)
    };
    let position = RwSignal::new(None::<(Ticket, u64)>);
    let on_position = Callback::new(move |(ticket, seconds): (Ticket, f64)| {
        let Some(segment) = current.get_untracked() else {
            return;
        };
        let Ok(elapsed) = Duration::try_from_secs_f64(seconds) else {
            return;
        };
        let offset = (elapsed.as_nanos() * u128::from(SAMPLE_RATE) / 1_000_000_000).min(
            u128::from(segment.range.end.saturating_sub(segment.range.start)),
        );
        position.set(Some((
            ticket,
            segment.range.start + u64::try_from(offset).unwrap_or(0),
        )));
    });
    let cursor = Signal::derive(move || {
        workspace.playback.with(|playback| {
            let playback = playback.as_ref()?;
            position.with(|position| {
                position
                    .as_ref()
                    .filter(|(ticket, _)| *ticket == playback.ticket)
                    .map(|(_, at)| *at)
            })
        })
    });
    let wav = Signal::derive(move || {
        workspace.playback.with(|playback| {
            playback
                .as_ref()?
                .url
                .strip_prefix("data:audio/wav;base64,")
                .map(|wav| Ok(wav.to_owned()))
        })
    });
    view! {
        <section class="review" class:correction-workspace=correcting aria-labelledby="review-heading">
            <div class="review-header"><h2 id="review-heading">{move || match workspace.flow.get().mode() {
                ReviewMode::Binary => "자동 전사문 검증", ReviewMode::Correction => "수정본 그대로 확인", ReviewMode::Browse => "조각 편집",
            }}</h2><div class="review-header-actions">
                <button on:click=move |_| workspace.skip()>"다음 조각"<span class="desktop-shortcut">" (N)"</span></button>
                <button on:click=move |_| workspace.exit()>"나가기"<span class="desktop-shortcut">" (Esc)"</span></button>
            </div></div>
            <Show when=move || workspace.current().is_some() fallback=|| view! { <p>"이 목록에 남은 조각이 없습니다. 나가서 저장한 글을 사용할 수 있습니다."</p> }>
                <Show when=correcting fallback=move || view! {
                    <p role="status">{move || format!("남은 조각 {}개",workspace.flow.get().remaining())}</p>
                    <p class="transcript" aria-live="polite">{move || current.get().map(|s| s.text)}</p>
                    <p>{move || current.get().map(|s| format!("음성 구간 {}~{} ms",s.range.start/16,s.range.end/16))}</p>
                }>
                    <div class="review-state">
                        <h3>"전체 조각 음성"</h3>
                        <span class="position">{move || current.get().map(|s| range_label(s.range))}</span>
                    </div>
                    {move || current.get().filter(|s| s.range.end > s.range.start).map(|s| view! {
                        <CorrectionTimeline range=s.range revision=s.recording_revision marks=workspace.marks wav cursor/>
                    })}
                </Show>
                <div class="segment-transport">
                    <button class="primary" disabled=move || workspace.flow.get().busy() on:click=move |_| workspace.play()>
                        {move || if workspace.flow.get().listened() { "다시 재생" } else { "구간 재생" }}
                        <span class="desktop-shortcut">" (Space)"</span>
                    </button>
                    {move || workspace.playback.get().map(|playback| view! { <RangeAudio workspace playback on_position/> })}
                    <p role="status" class:listened=move || workspace.flow.with(ReviewFlow::listened)>
                        {move || if workspace.flow.get().listened() { "이 수정본의 음성을 끝까지 들었습니다." } else { "현재 조각을 끝까지 들으면 확인 버튼을 쓸 수 있습니다." }}
                    </p>
                </div>
                <Show when=move || workspace.flow.get().mode() == ReviewMode::Binary fallback=move || view! { <CorrectionEditor workspace/> }>
                    <p>"글이 음성과 그대로 같습니까? 잘 안 들리거나 구간이 잘렸다면 검증하지 않고 나갈 수 있습니다."</p>
                    <div class="verdicts">
                        <button disabled=move || !workspace.flow.get().listened() || workspace.flow.get().busy() on:click=move |_| workspace.save(ReviewAction::Judge { verdict: Verdict::Yes })>"Yes, 그대로 맞음 (1)"</button>
                        <button disabled=move || !workspace.flow.get().listened() || workspace.flow.get().busy() on:click=move |_| workspace.save(ReviewAction::Judge { verdict: Verdict::No })>"No, 다름"</button>
                    </div>
                </Show>
            </Show>
        </section>
    }
}

#[component]
fn CorrectionEditor(workspace: Workspace) -> impl IntoView {
    view! {
        <div class="correction-texts">
            <Show when=move || workspace.flow.with(|flow| flow.mode() == ReviewMode::Correction)>
                <section class="original-transcript" aria-labelledby="original-heading">
                    <h3 id="original-heading">"자동 전사 원문"</h3>
                    <p class="transcript">{move || workspace.current().map(|segment| segment.machine_text)}</p>
                </section>
            </Show>
            <div class="draft-area">
                <label for="draft-text">"수정 초안"</label>
                <textarea id="draft-text" rows="5" maxlength="32768" prop:value=move || workspace.draft.get() disabled=move || workspace.flow.get().busy()
                    on:input=move |event| workspace.draft.set(event_target_value(&event))/>
            </div>
        </div>
        <p class="editor-help">"들은 말을 그대로 적으세요. 초안을 저장해 전사가 바뀌면 확정하기 전에 다시 들어야 합니다."</p>
        <div class="toolbar correction-save">
            <button disabled=move || workspace.flow.get().busy() on:click=move |_| workspace.save(ReviewAction::SaveDraft { text:workspace.draft.get_untracked() })>"초안 저장"</button>
            <Show when=move || workspace.flow.get().mode() == ReviewMode::Correction>
                <button class="primary" disabled=move || !workspace.flow.get().listened() || workspace.flow.get().busy() || workspace.current().is_none_or(|s| !s.can_confirm_correction(&workspace.draft.get()))
                    on:click=move |_| workspace.save(ReviewAction::CorrectAndConfirm { text:workspace.draft.get_untracked() })>
                    "이대로 교정 확정"<span class="desktop-shortcut">" (Ctrl/Command+Enter)"</span>
                </button>
            </Show>
        </div>
        <Show when=move || workspace.flow.get().mode() == ReviewMode::Correction>
            {move || {
                let segment = workspace.current()?;
                let ranges = workspace.marks.with(|marks| marks.iter()
                    .filter(|mark| mark.recording_revision == segment.recording_revision && overlaps(mark.range, segment.range))
                    .map(|mark| range_label(mark.range))
                    .collect::<Vec<_>>());
                (!ranges.is_empty()).then(|| view! { <p>"표시한 오류 구간 "{ranges.join(", ")}</p> })
            }}
        </Show>
        <p>{move || workspace.current().and_then(|s| s.correction_error(&workspace.draft.get())).map(|error| error.to_string())}</p>
        <Show when=move || workspace.current().is_some_and(|s| s.verified.is_none() && s.error_marked_text.is_some())>
            <button disabled=move || workspace.flow.get().busy() on:click=move |_| workspace.save(ReviewAction::DismissErrorMark)>"이 조각의 오류 판정 해제"</button>
        </Show>
        <Show when=move || workspace.current().is_some_and(|s| s.error_marked_text.is_none() || s.verdict.is_some() || s.verified.is_some())>
            <button disabled=move || workspace.flow.get().busy() on:click=move |_| workspace.save(ReviewAction::Invalidate)>
                {move || if workspace.current().is_some_and(|s| s.rejected_text_unchanged(&s.text)) { "처음 판정 취소하고 다시 검수" } else { "검증 취소, 다시 확인 목록에 남기기" }}
            </button>
        </Show>
    }
}

#[component]
fn RangeAudio(
    workspace: Workspace,
    playback: Playback,
    on_position: Callback<(Ticket, f64)>,
) -> impl IntoView {
    let audio = NodeRef::<leptos::html::Audio>::new();
    let ended = playback.clone();
    let failed = playback.ticket.clone();
    let timing = playback.ticket.clone();
    on_cleanup(move || {
        if let Some(node) = audio.get_untracked() {
            let _ = node.pause();
            node.set_src("");
        }
    });
    view! { <audio node_ref=audio src=playback.url autoplay=true aria-label="현재 조각의 음성"
    on:timeupdate=move |event| {
        if workspace.flow.with_untracked(|flow| flow.playing(&timing)) {
            on_position.run((timing.clone(), event_target::<HtmlAudioElement>(&event).current_time()));
        }
    }
    on:ended=move |event| {
        if event_target::<web_sys::HtmlAudioElement>(&event).ended() {
            workspace.ended(ended.clone());
        }
    }
    on:error=move |_| workspace.playback_error(&failed,"이 음성을 재생하지 못했습니다. 다시 재생하거나 검증하지 않고 나가세요.".into())/> }
}
