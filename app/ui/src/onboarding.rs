use crate::capture::Microphone;
use leptos::prelude::*;

#[component]
pub(crate) fn PrivacyInfo(
    microphone: Microphone,
    can_add: Signal<bool>,
    import_disabled: RwSignal<bool>,
    inference_available: Signal<bool>,
    on_dismiss: Callback<()>,
    on_import: Callback<()>,
) -> impl IntoView {
    view! {
        <section class="info onboarding" aria-labelledby="privacy-heading">
            <div class="onboarding-header">
                <div>
                    <h2 id="privacy-heading">"기기에 남는 음성과 글"</h2>
                    <p>{move || if inference_available.get() {
                        "녹음하거나 음성 파일을 열어 기기 안에서 받아쓸 수 있습니다."
                    } else {
                        "음성 파일을 기기에 보관할 수 있습니다. 새 받아쓰기는 Whisper 엔진이 포함된 데스크톱 빌드에서 쓸 수 있습니다."
                    }}</p>
                </div>
                <button on:click=move |_| on_dismiss.run(())>"안내 닫기"</button>
            </div>
            <Show when=move || can_add.get()>
                <div class="toolbar onboarding-actions">
                    <Show when=move || microphone.can_offer_start()>
                        <button class="primary" disabled=move || microphone.start_disabled() on:click=move |_| {
                            on_dismiss.run(());
                            microphone.start();
                        }>"첫 녹음 시작"</button>
                    </Show>
                    <button class:primary=move || !microphone.can_offer_start()
                        disabled=move || import_disabled.get() on:click=move |_| {
                            on_dismiss.run(());
                            on_import.run(());
                        }>"첫 음성 파일 가져오기"</button>
                </div>
            </Show>
            <div class="onboarding-grid">
                <section aria-labelledby="privacy-now">
                    <h3 id="privacy-now">"지금 앱에서"</h3>
                    <p>"녹음과 전사문, 오류 표시, 수정본은 이 기기에 보관합니다. 검증은 건너뛰어도 글을 복사하거나 내보낼 수 있습니다."</p>
                    <p>"음성을 듣고 발화 그대로라고 확인한 전사만 학습 대기열에 담습니다."</p>
                </section>
                <section aria-labelledby="privacy-future">
                    <h3 id="privacy-future">"향후 연합학습"</h3>
                    <ol class="privacy-flow">
                        <li><strong>"기기 안에서 학습"</strong><span>"확인한 전사와 원본 음성으로 백그라운드에서 학습"</span></li>
                        <li><strong>"모델 변경분 보호"</strong><span>"변경분 크기를 제한하고 노이즈를 더해 특정 녹음을 추측하기 어렵게 함"</span></li>
                        <li><strong>"보안 집계"</strong><span>"서버가 여러 기기의 변경분 합계만 읽어 공통 모델 갱신"</span></li>
                    </ol>
                    <p class="onboarding-note">"기기 안 학습과 모델 변경분 전송은 준비 중입니다."</p>
                </section>
            </div>
        </section>
    }
}
