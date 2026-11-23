use crate::{
    capture::{Microphone, MicrophonePanel},
    onboarding::PrivacyInfo,
    quick_review::{QuickReviewPanel, range_label},
    timeline::CorrectionTimeline,
    workspace::{LibraryState, Playback, TranscriptionState, Workspace},
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
            .with(|page| page.as_ref().map(|page| page.inference_available))
    });
    let microphone = Microphone::new(
        workspace.busy,
        workspace.error,
        RwSignal::new(String::new()),
    );
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
                        <p class="status" role="status" aria-label="작업 안내">{move || workspace.status.get()}</p>
                    </Show>
                    <SavedRecordingNotice workspace can_open=show_capture_start/>
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
            <LibraryLoadNotice workspace/>
            <Show when=move || !locked()>
              <div class="library-actions">
                <button disabled=move || workspace.busy.get() on:click=move |_| workspace.import()>"음성 파일 가져오기"</button>
                <details>
                    <summary>"보관함 설정"</summary>
                    <button disabled=move || workspace.busy.get() || !workspace.inference_available() on:click=move |_| workspace.select_model()>"Whisper 모델 선택"</button>
                    <button disabled=move || workspace.busy.get() || workspace.library_state.with(|state| matches!(state, LibraryState::Loading)) on:click=move |_| workspace.refresh()>"보관함 새로 불러오기"</button>
                    <p>"WAV, MP3, AAC-LC M4A · 최대 30분, 512MiB"</p>
                    <p>{move || workspace.library.get().map(|page| format!("로컬 학습 대기 {}개 (원본 확인 재시도 {}개), 앱 내부 학습 엔진 준비 전",page.pending_training,page.retry_training))}</p>
                </details>
                <p class="input-help">"읽을 수 없는 음성은 WAV, MP3, AAC-LC M4A 파일을 다시 선택하세요. 선택을 취소하면 기존 녹음은 그대로 남습니다."</p>
              </div>
            </Show>
            <Show when=move || workspace.library.with(|page| page.as_ref().is_some_and(|p| p.recordings.is_empty()))>
                <p>"아직 저장한 녹음이 없습니다."</p>
            </Show>
            <ul class="recordings">
                <For each=move || workspace.library.get().map_or_else(Vec::new,|p| p.recordings) key=|r| (r.id.clone(),r.revision,r.engine.clone())
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
fn LibraryLoadNotice(workspace: Workspace) -> impl IntoView {
    view! {
        <Show when=move || workspace.library.with(Option::is_none) && workspace.library_state.with(|state| matches!(state, LibraryState::Loading))>
            <p role="status" aria-label="보관함 조회 안내">"보관함을 불러오는 중입니다."</p>
        </Show>
        <Show when=move || workspace.library_state.with(|state| matches!(state, LibraryState::Failed(_)))>
            <p class="error" role="alert" aria-label="보관함 조회 오류">{move || match workspace.library_state.get() {
                LibraryState::Failed(error) => error,
                _ => String::new(),
            }}</p>
            <p>"보관함을 다시 불러오세요. 저장한 녹음과 전사는 지우지 않습니다."</p>
            <Show when=move || workspace.library.with(Option::is_some)>
                <p>"기존 보관함 목록은 그대로 표시합니다."</p>
            </Show>
            <button on:click=move |_| workspace.refresh()>"보관함 다시 불러오기"</button>
        </Show>
    }
}

#[component]
fn SavedRecordingNotice(workspace: Workspace, can_open: Signal<bool>) -> impl IntoView {
    let needs_open = move || {
        workspace.saved_recording.with(|saved| {
            saved.as_ref().is_some_and(|saved| {
                workspace
                    .recording
                    .with(|selected| selected.as_ref() != Some(&saved.id))
            })
        })
    };
    view! {
        <Show when=move || workspace.saved_recording.with(Option::is_some)>
            <div class="saved-recording-notice">
                <p class="status" role="status" aria-label="녹음 저장 안내">{move || workspace.saved_recording.get()
                    .map(|recording| format!("{} · 녹음 보관함에 저장했습니다.{}", recording.name,
                        if needs_open() { " 저장한 녹음 열기로 확인하세요." } else { "" }))}</p>
                <div class="toolbar">
                    <Show when=needs_open>
                    <button disabled=move || workspace.busy.get() || !can_open.get() on:click=move |_| {
                        if let Some(recording) = workspace.saved_recording.get_untracked() {
                            workspace.open(recording.id);
                        }
                    }>"저장한 녹음 열기"</button>
                    </Show>
                    <button on:click=move |_| workspace.saved_recording.set(None)>"저장 안내 닫기"</button>
                </div>
            </div>
        </Show>
    }
}

#[component]
fn SavedTranscript(workspace: Workspace) -> impl IntoView {
    view! {
        <Show when=move || workspace.recording.with(Option::is_none)>
            <section class="empty-editor">
                <h2>{move || match workspace.library.get() {
                    None => workspace.library_state.with(|state| if matches!(state, LibraryState::Failed(_)) {
                        "보관함을 불러오지 못했습니다"
                    } else {
                        "보관함을 불러오는 중입니다."
                    }),
                    Some(page) if page.recordings.is_empty() => "첫 녹음을 추가하세요",
                    Some(_) => "녹음을 열어 시작하세요",
                }}</h2>
                <p>"음성 파일 가져오기에서 WAV, MP3, AAC-LC M4A를 고르세요. 마이크 녹음을 지원하는 기기에서는 녹음 시작 후 멈추고 저장을 누르세요."</p>
                <p>"녹음과 전사는 이 기기에 저장합니다."</p>
                <p>"저장한 녹음을 열면 그 녹음에서 바로 받아쓰기를 시작할 수 있습니다."</p>
            </section>
        </Show>
        <Show when=move || workspace.recording.get().is_some()>
            <section aria-labelledby="transcript-heading"
                aria-busy=move || if workspace.busy.get() { "true" } else { "false" }>
                <h2 id="transcript-heading">"저장한 글"</h2>
                <p class="selected-recording">{move || workspace.selected_recording().map(|r| format!("선택한 녹음: {}", r.name))}</p>
                <Show when=move || !workspace.has_transcription()>
                    <TranscriptionPreparation workspace/>
                </Show>
                <div class="toolbar">
                    <button class="primary" disabled=move || !workspace.can_transcribe() on:click=move |_| workspace.transcribe()>
                        {move || match workspace.transcription.get() {
                            TranscriptionState::Running => "기기 안에서 받아쓰는 중",
                            TranscriptionState::Failed => "같은 녹음 다시 받아쓰기",
                            _ => "기기 안에서 받아쓰기",
                        }}
                    </button>
                    <button disabled=move || workspace.busy.get() || workspace.segments.with(Vec::is_empty) on:click=move |_| workspace.export(true)>"저장한 글 복사"</button>
                    <button disabled=move || workspace.busy.get() || workspace.segments.with(Vec::is_empty) on:click=move |_| workspace.export(false)>"텍스트 파일로 저장"</button>
                    <button disabled=move || workspace.busy.get() on:click=move |_| { workspace.refresh(); if let Some(id) = workspace.recording.get_untracked() { workspace.open(id); } }>"수정본 새로 불러오기"</button>
                </div>
                <div class="toolbar">
                    <button class="primary" disabled=move || workspace.busy.get() || workspace.segments.with(Vec::is_empty) || workspace.quick_recording().is_none() on:click=move |_| workspace.start_quick()>"빠른 검수 시작"</button>
                    <button disabled=move || workspace.busy.get() || workspace.segments.with(Vec::is_empty) on:click=move |_| workspace.start(ReviewMode::Correction,&workspace.segments.get_untracked())>"오류와 수정본 교정 목록"</button>
                </div>
                <details>
                    <summary>"조각별 검증"</summary>
                    <button disabled=move || workspace.busy.get() || workspace.segments.with(Vec::is_empty) on:click=move |_| workspace.start(ReviewMode::Binary,&workspace.segments.get_untracked())>"자동 전사문 Yes/No 검증"</button>
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
                <Show when=move || !workspace.busy.get() && workspace.review_loaded.get() && workspace.has_transcription() && workspace.segments.with(Vec::is_empty)>
                    <p>"받아쓰기를 마쳤지만 인식된 글이 없습니다."</p>
                    <p>"음성이 담긴 다른 파일을 가져오거나 새로 녹음하세요. 이 녹음은 보관함에 남습니다."</p>
                </Show>
                <Show when=move || !workspace.segments.with(Vec::is_empty)>
                    <p>"조각별로 듣고 편집하거나 빠른 검수에서 오류를 표시하세요. 저장한 글은 검수 전에도 내보낼 수 있습니다."</p>
                </Show>
                <ol class="segments">
                    <For each=move || workspace.segments.get() key=|s| (s.id.clone(),s.revision)
                        children=move |s| {
                            let text = s.text.clone();
                            let state = if s.verified.is_some() { "사람이 확인한 정답" } else if s.needs_review() { "수정본 확인 필요" } else { "미검증" };
                            let verdict = match s.verdict { Some(Verdict::Yes) => "자동 전사문 판정: Yes", Some(Verdict::No) => "자동 전사문 판정: No", None => "자동 전사문 판정 없음" };
                            view! { <li><p class="transcript">{text}</p><p>{state}" / "{verdict}</p><button disabled=move || workspace.busy.get() on:click=move |_| workspace.start(ReviewMode::Browse,std::slice::from_ref(&s))>"이 조각 재생, 편집"</button></li> }
                        }/>
                </ol>
            </section>
        </Show>
    }
}

#[component]
fn TranscriptionPreparation(workspace: Workspace) -> impl IntoView {
    view! {
        <section class="transcription-preparation" aria-labelledby="preparation-heading">
            <h3 id="preparation-heading">"받아쓰기 준비"</h3>
            <Show when=move || workspace.library.with(Option::is_some) fallback=move || view! {
                <p>{move || workspace.library_state.with(|state| match state {
                    LibraryState::Loading => "보관함을 불러온 뒤 엔진 지원 여부를 확인합니다.",
                    LibraryState::Failed(_) => "보관함 조회에 실패해 엔진 지원 여부를 확인하지 못했습니다. 녹음 보관함의 보관함 다시 불러오기를 누르세요.",
                    LibraryState::Ready => "보관함 정보를 다시 불러와 엔진 지원 여부를 확인하세요.",
                })}</p>
            }>
            <Show when=move || workspace.inference_available() fallback=|| view! {
                <p>"새 받아쓰기는 Whisper 엔진이 포함된 데스크톱 설치본에서 사용할 수 있습니다. 이 설치본에서는 저장한 녹음과 기존 전사를 열 수 있습니다."</p>
            }>
                <p>"Whisper small 검증용 모델 ggml-small.bin을 직접 준비한 뒤 파일을 선택하세요. 이미 선택한 모델이 있으면 바로 받아쓰기를 시작하세요."</p>
                <button disabled=move || workspace.busy.get() on:click=move |_| workspace.select_model()>
                    {move || if workspace.transcription.get() == TranscriptionState::SelectingModel {
                        "모델 파일 확인 중"
                    } else {
                        "Whisper 모델 선택"
                    }}
                </button>
                <details>
                    <summary>"모델 파일 수동 준비 방법"</summary>
                    <p>"브라우저에서 아래 공식 모델 주소를 열어 파일을 준비하세요. 앱은 모델을 자동으로 내려받지 않습니다."</p>
                    <code class="model-source">"https://huggingface.co/ggerganov/whisper.cpp/resolve/80da2d8bfee42b0e836fc3a9890373e5defc00a6/ggml-small.bin"</code>
                    <p>"파일 선택 시 무결성을 확인합니다. 파일이 없거나 손상됐으면 정상 파일을 다시 선택하세요. 모델을 바꿔도 녹음과 기존 전사는 이 기기에 남습니다."</p>
                </details>
            </Show>
            </Show>
        </section>
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
