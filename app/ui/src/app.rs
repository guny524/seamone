use crate::{
    api,
    workspace::{Playback, Workspace},
};
use leptos::{ev, prelude::*, task::spawn_local};
use seamone_core::{
    review::{ReviewAction, Verdict},
    review_flow::ReviewMode,
};
use serde_json::json;
use wasm_bindgen::JsCast;

#[component]
pub fn App() -> impl IntoView {
    let workspace = Workspace::new();
    workspace.refresh();
    let listener = window_event_listener(ev::keydown, move |event| {
        if event.repeat() || event.is_composing() {
            return;
        }
        let mode = workspace.flow.get_untracked().mode();
        let key = event.key();
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
            "2" if mode == ReviewMode::Binary => {
                event.prevent_default();
                workspace.save(ReviewAction::Judge {
                    verdict: Verdict::No,
                });
            }
            _ => {}
        }
    });
    on_cleanup(move || listener.remove());
    view! {
        <main class="container">
            <header><h1>"seamone"</h1><button on:click=move |_| workspace.show_info.set(true)>"개인정보와 학습 안내"</button></header>
            <Show when=move || workspace.show_info.get()><PrivacyInfo workspace/></Show>
            <p class="error" role="alert">{move || workspace.error.get()}</p>
            <p class="status" role="status">{move || workspace.status.get()}</p>
            <Show when=move || workspace.flow.get().frame().is_none() && workspace.flow.get().mode() == ReviewMode::Browse
                fallback=move || view! { <ReviewPanel workspace/> }><Library workspace/></Show>
        </main>
    }
}

#[component]
fn PrivacyInfo(workspace: Workspace) -> impl IntoView {
    view! {
        <section class="info" aria-labelledby="privacy-heading">
            <h2 id="privacy-heading">"기기에 남는 음성과 글"</h2>
            <p>"현재 앱은 가져온 WAV, 받아쓴 글, 수정본과 검증 결과를 이 기기에 저장합니다. 음성과 글을 외부로 보내지 않습니다. 모델도 자동으로 내려받지 않습니다."</p>
            <p>"사람이 음성을 듣고 그대로 맞는다고 확인한 글만 로컬 학습 대기열에 넣습니다. 현재 설치본의 앱 내부 학습 엔진은 준비되지 않았으므로 대기열을 보관하며 학습 진행률을 표시하지 않습니다."</p>
            <p>"향후 연합학습에서는 음성과 글 대신 기기에서 학습한 모델의 변화량을 보낼 계획입니다. 한 기기가 보내는 값의 크기를 제한하고 잡음을 더해, 공개된 모델이나 합친 값에서 특정 녹음을 추측할 가능성을 줄이려 합니다. 보안 집계는 서버가 여러 기기의 변화량을 합한 값만 읽게 하는 방식입니다. 현재 앱은 학습하거나 변화량을 전송하지 않습니다."</p>
            <p>"검증은 언제든 나갈 수 있습니다. 검증하지 않아도 저장한 글을 복사하거나 내보낼 수 있습니다."</p>
            <button on:click=move |_| {
                workspace.show_info.set(false);
                spawn_local(async move { if let Err(e) = api::call::<()>("dismiss_info",json!({})).await { workspace.error.set(e); } });
            }>"안내 닫기"</button>
        </section>
    }
}

#[component]
fn Library(workspace: Workspace) -> impl IntoView {
    view! {
        <section aria-labelledby="library-heading">
            <h2 id="library-heading">"녹음 보관함"</h2>
            <div class="toolbar">
                <button disabled=move || workspace.busy.get() on:click=move |_| workspace.import()>"WAV 가져오기"</button>
                <button disabled=move || workspace.busy.get() on:click=move |_| workspace.select_model()>"Whisper 모델 선택"</button>
                <button disabled=move || workspace.busy.get() on:click=move |_| workspace.refresh()>"보관함 새로 불러오기"</button>
            </div>
            <p>"16kHz mono PCM16 WAV, 30분 이하. 파일 선택 대화상자에서 가져옵니다."</p>
            <p>{move || workspace.library.get().map(|page| format!("로컬 학습 대기 {}개 (원본 확인 재시도 {}개), 앱 내부 학습 엔진 준비 전",page.pending_training,page.retry_training))}</p>
            <ul class="recordings">
                <For each=move || workspace.library.get().map_or_else(Vec::new,|p| p.recordings) key=|r| r.id.clone()
                    children=move |r| {
                        let id = r.id.clone();
                        view! { <li><button aria-pressed=move || workspace.recording.get().as_ref() == Some(&id)
                            disabled=move || workspace.busy.get() on:click=move |_| workspace.open(r.id.clone())>{r.name.clone()}</button>
                            <span>{format!("{}초, {}",r.samples/16_000,r.engine)}</span></li> }
                    }/>
            </ul>
        </section>
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
                    <button disabled=move || workspace.busy.get() on:click=move |_| workspace.start(ReviewMode::Binary,&workspace.segments.get_untracked())>"자동 전사문 Yes/No 검증"</button>
                    <button disabled=move || workspace.busy.get() on:click=move |_| workspace.start(ReviewMode::Correction,&workspace.segments.get_untracked())>"No와 수정본 확인 목록"</button>
                </div>
                <p>"검증을 건너뛰어도 복사와 내보내기를 쓸 수 있습니다. 편집은 초안으로 저장되며 별도 확인 전에는 학습 정답이 아닙니다."</p>
                <ol class="segments">
                    <For each=move || workspace.segments.get() key=|s| (s.id.clone(),s.revision)
                        children=move |s| {
                            let text = s.text.clone();
                            let state = if s.verified.is_some() { "사람이 확인한 정답" } else if s.needs_correction { "수정본 확인 필요" } else { "미검증" };
                            let verdict = match s.verdict { Some(Verdict::Yes) => "자동 전사문 판정: Yes", Some(Verdict::No) => "자동 전사문 판정: No", None => "자동 전사문 판정 없음" };
                            view! { <li><p class="transcript">{text}</p><p>{state}" / "{verdict}</p><button on:click=move |_| workspace.start(ReviewMode::Browse,std::slice::from_ref(&s))>"이 조각 재생, 편집"</button></li> }
                        }/>
                </ol>
            </section>
        </Show>
    }
}

#[component]
fn ReviewPanel(workspace: Workspace) -> impl IntoView {
    view! {
        <section class="review" aria-labelledby="review-heading">
            <div class="toolbar"><h2 id="review-heading">{move || match workspace.flow.get().mode() {
                ReviewMode::Binary => "자동 전사문 검증", ReviewMode::Correction => "수정본 그대로 확인", ReviewMode::Browse => "조각 편집",
            }}</h2><button on:click=move |_| workspace.exit()>"나가기 (Esc)"</button>
                <button on:click=move |_| workspace.skip()>"다음 조각 (N)"</button></div>
            <Show when=move || workspace.current().is_some() fallback=|| view! { <p>"이 목록에 남은 조각이 없습니다. 나가서 저장한 글을 사용할 수 있습니다."</p> }>
                <p role="status">{move || format!("남은 조각 {}개",workspace.flow.get().remaining())}</p>
                <p class="transcript" aria-live="polite">{move || workspace.current().map(|s| s.text)}</p>
                <p>{move || workspace.current().map(|s| format!("음성 구간 {}~{} ms",s.range.start/16,s.range.end/16))}</p>
                <button disabled=move || workspace.flow.get().busy() on:click=move |_| workspace.play()>{move || if workspace.flow.get().listened() { "다시 재생 (Space)" } else { "구간 재생 (Space)" }}</button>
                {move || workspace.playback.get().map(|playback| view! { <RangeAudio workspace playback/> })}
                <p role="status">{move || if workspace.flow.get().listened() { "이 수정본의 음성을 끝까지 들었습니다." } else { "현재 조각을 끝까지 들으면 확인 버튼을 쓸 수 있습니다." }}</p>
                <Show when=move || workspace.flow.get().mode() == ReviewMode::Binary fallback=move || view! { <CorrectionEditor workspace/> }>
                    <p>"글이 음성과 그대로 같습니까? 잘 안 들리거나 구간이 잘렸다면 검증하지 않고 나갈 수 있습니다."</p>
                    <div class="verdicts">
                        <button disabled=move || !workspace.flow.get().listened() || workspace.flow.get().busy() on:click=move |_| workspace.save(ReviewAction::Judge { verdict: Verdict::Yes })>"Yes, 그대로 맞음 (1)"</button>
                        <button disabled=move || !workspace.flow.get().listened() || workspace.flow.get().busy() on:click=move |_| workspace.save(ReviewAction::Judge { verdict: Verdict::No })>"No, 다름 (2)"</button>
                    </div>
                </Show>
            </Show>
        </section>
    }
}

#[component]
fn CorrectionEditor(workspace: Workspace) -> impl IntoView {
    view! {
        <label for="draft-text">"수정 초안"</label>
        <textarea id="draft-text" rows="5" maxlength="32768" prop:value=move || workspace.draft.get() disabled=move || workspace.flow.get().busy()
            on:input=move |event| workspace.draft.set(event_target_value(&event))/>
        <button disabled=move || workspace.flow.get().busy() on:click=move |_| workspace.save(ReviewAction::SaveDraft { text:workspace.draft.get_untracked() })>"초안 저장"</button>
        <p>"음성을 끝까지 듣고 나온 말을 그대로 고친 뒤 교정을 확정하세요. 요약이나 추측을 정답으로 확인하지 마세요. 중간에 멈추려면 초안만 저장할 수 있습니다. 초안을 저장해 수정본이 바뀌면 다시 들어야 합니다."</p>
        <Show when=move || workspace.flow.get().mode() == ReviewMode::Correction>
            <button disabled=move || !workspace.flow.get().listened() || workspace.flow.get().busy() || workspace.current().is_none_or(|s| !s.needs_correction || s.rejected_text_unchanged(&workspace.draft.get()))
                on:click=move |_| workspace.save(ReviewAction::CorrectAndConfirm { text:workspace.draft.get_untracked() })>"이대로 교정 확정 (Ctrl/Command+Enter)"</button>
        </Show>
        <p>{move || workspace.current().filter(|s| s.rejected_text_unchanged(&workspace.draft.get())).map(|_| "처음 No로 판정한 글을 그대로 확정할 수 없습니다. 그대로 맞는 글이라면 처음 판정을 취소하고 다시 검수하세요.")}</p>
        <button disabled=move || workspace.flow.get().busy() on:click=move |_| workspace.save(ReviewAction::Invalidate)>
            {move || if workspace.current().is_some_and(|s| s.rejected_text_unchanged(&s.text)) { "처음 판정 취소하고 다시 검수" } else { "검증 취소, 다시 확인 목록에 남기기" }}
        </button>
    }
}

#[component]
fn RangeAudio(workspace: Workspace, playback: Playback) -> impl IntoView {
    let audio = NodeRef::<leptos::html::Audio>::new();
    let ended = playback.clone();
    let failed = playback.ticket.clone();
    on_cleanup(move || {
        if let Some(node) = audio.get_untracked() {
            let _ = node.pause();
            node.set_src("");
        }
    });
    view! { <audio node_ref=audio src=playback.url autoplay=true aria-label="현재 조각의 음성"
    on:ended=move |_| workspace.ended(ended.clone())
    on:error=move |_| workspace.playback_error(&failed,"이 음성을 재생하지 못했습니다. 다시 재생하거나 검증하지 않고 나가세요.".into())/> }
}
