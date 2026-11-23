use crate::api;
use leptos::{prelude::*, task::spawn_local};
use seamone_core::{
    capture::{CaptureStatus, MAX_RECORDING_SECONDS},
    review::RecordingInfo,
};
use serde_json::json;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    None,
    Start,
    Save,
    Cancel,
}

#[derive(Clone, Copy)]
pub(crate) struct Microphone {
    status: RwSignal<CaptureStatus>,
    action: RwSignal<Action>,
    version: RwSignal<u32>,
    polling: RwSignal<bool>,
    busy: RwSignal<bool>,
    error: RwSignal<String>,
    notice: RwSignal<String>,
}

impl Microphone {
    pub(crate) fn new(
        busy: RwSignal<bool>,
        error: RwSignal<String>,
        notice: RwSignal<String>,
    ) -> Self {
        let microphone = Self {
            status: RwSignal::new(CaptureStatus::Unavailable),
            action: RwSignal::new(Action::None),
            version: RwSignal::new(0),
            polling: RwSignal::new(false),
            busy,
            error,
            notice,
        };
        microphone.poll();
        if let Ok(timer) = set_interval_with_handle(
            move || {
                if matches!(
                    microphone.status.get_untracked(),
                    CaptureStatus::Starting { .. }
                        | CaptureStatus::Recording { .. }
                        | CaptureStatus::Finishing { .. }
                ) {
                    microphone.poll();
                }
            },
            Duration::from_millis(250),
        ) {
            on_cleanup(move || timer.clear());
        }
        microphone
    }

    pub(crate) fn can_offer_start(self) -> bool {
        matches!(
            self.status.get(),
            CaptureStatus::Idle | CaptureStatus::Failed { .. }
        )
    }

    pub(crate) fn start_disabled(self) -> bool {
        self.busy.get() || self.action.get() != Action::None
    }

    fn poll(self) {
        if self.polling.get_untracked() || self.action.get_untracked() != Action::None {
            return;
        }
        self.polling.set(true);
        let version = self.version.get_untracked();
        spawn_local(async move {
            let result = api::call::<CaptureStatus>("get_capture_status", json!({})).await;
            self.polling.set(false);
            if version != self.version.get_untracked() {
                return;
            }
            match result {
                Ok(status) => {
                    if self.status.with_untracked(|current| *current != status) {
                        self.status.set(status);
                    }
                }
                Err(error) => self.error.set(error),
            }
        });
    }

    fn begin(self, action: Action) {
        self.version
            .update(|version| *version = version.wrapping_add(1));
        self.action.set(action);
        self.error.set(String::new());
        self.notice.set(String::new());
    }

    pub(crate) fn start(self) {
        if untrack(|| self.start_disabled() || !self.can_offer_start()) {
            return;
        }
        self.begin(Action::Start);
        spawn_local(async move {
            match api::call::<CaptureStatus>("start_recording", json!({})).await {
                Ok(status) => self.status.set(status),
                Err(error) => self.error.set(error),
            }
            self.action.set(Action::None);
            self.poll();
        });
    }

    fn finish(self, save: bool, on_saved: Callback<RecordingInfo>) {
        if self.action.get_untracked() != Action::None {
            return;
        }
        let id = match self.status.get_untracked() {
            CaptureStatus::Starting { session_id } if !save => session_id,
            CaptureStatus::Recording { session_id, .. }
            | CaptureStatus::LimitReached { session_id }
            | CaptureStatus::Interrupted { session_id, .. } => session_id,
            _ => return,
        };
        self.begin(if save { Action::Save } else { Action::Cancel });
        spawn_local(async move {
            match api::call::<Option<RecordingInfo>>(
                "finish_recording",
                json!({"sessionId": id, "save": save}),
            )
            .await
            {
                Ok(Some(info)) => {
                    self.status.set(CaptureStatus::Idle);
                    on_saved.run(info);
                }
                Ok(None) => {
                    self.status.set(CaptureStatus::Idle);
                    self.notice.set("녹음을 취소했습니다.".into());
                }
                Err(error) => self.error.set(error),
            }
            self.action.set(Action::None);
            self.poll();
        });
    }
}

#[component]
pub(crate) fn MicrophonePanel(
    microphone: Microphone,
    browsing: Signal<bool>,
    on_saved: Callback<RecordingInfo>,
) -> impl IntoView {
    let active = move || {
        matches!(
            microphone.status.get(),
            CaptureStatus::Starting { .. }
                | CaptureStatus::Recording { .. }
                | CaptureStatus::LimitReached { .. }
                | CaptureStatus::Interrupted { .. }
        )
    };
    let can_save = move || {
        matches!(
            microphone.status.get(),
            CaptureStatus::Recording { .. }
                | CaptureStatus::LimitReached { .. }
                | CaptureStatus::Interrupted { .. }
        )
    };
    view! {
        <Show when=move || match microphone.status.get() {
            CaptureStatus::Unavailable => false,
            CaptureStatus::Idle | CaptureStatus::Failed { .. } => {
                microphone.action.get() != Action::None || browsing.get()
            }
            _ => true,
        }>
            <section aria-labelledby="microphone-heading">
                <h2 id="microphone-heading">"마이크 녹음"</h2>
                <div class="toolbar">
                    <Show when=move || microphone.can_offer_start()>
                        <button disabled=move || microphone.start_disabled()
                            on:click=move |_| microphone.start()>"녹음 시작"</button>
                    </Show>
                    <Show when=active>
                        <button disabled=move || !can_save() || microphone.action.get() != Action::None
                            on:click=move |_| microphone.finish(true, on_saved)>"멈추고 저장"</button>
                        <button disabled=move || microphone.action.get() != Action::None
                            on:click=move |_| microphone.finish(false, on_saved)>"녹음 취소"</button>
                    </Show>
                </div>
                <Show when=move || !microphone.notice.with(String::is_empty)>
                    <p class="status" role="status">{move || microphone.notice.get()}</p>
                </Show>
                <p aria-live="polite">{move || match microphone.action.get() {
                    Action::Start => "마이크를 여는 중입니다.".into(),
                    Action::Save => "녹음을 저장하는 중입니다.".into(),
                    Action::Cancel => "녹음을 취소하는 중입니다.".into(),
                    Action::None => match microphone.status.get() {
                        CaptureStatus::Starting { .. } => "마이크 입력을 기다리는 중입니다.".into(),
                        CaptureStatus::Recording { elapsed_seconds, .. } => format!(
                            "녹음 중 {:02}:{:02} / {:02}:00",
                            elapsed_seconds / 60, elapsed_seconds % 60, MAX_RECORDING_SECONDS / 60,
                        ),
                        CaptureStatus::LimitReached { .. } => "30분 제한에 도달했습니다. 녹음을 저장하거나 취소한 뒤 창을 닫을 수 있습니다.".into(),
                        CaptureStatus::Interrupted { elapsed_seconds, reason, .. } => format!(
                            "{reason} 받은 음성 {:02}:{:02}. 녹음을 저장하거나 취소한 뒤 창을 닫을 수 있습니다.",
                            elapsed_seconds / 60, elapsed_seconds % 60,
                        ),
                        CaptureStatus::Finishing { .. } => "녹음을 마무리하는 중입니다.".into(),
                        _ => "기본 마이크로 최대 30분까지 녹음합니다.".into(),
                    }
                }}</p>
                <Show when=move || matches!(microphone.status.get(), CaptureStatus::Failed { .. })>
                    <p class="error" role="alert">{move || if let CaptureStatus::Failed { message } = microphone.status.get() { message } else { String::new() }}</p>
                    <p>"마이크 권한과 연결을 확인한 뒤 녹음 시작을 다시 누르세요. 음성 파일 가져오기로 시작할 수도 있습니다."</p>
                </Show>
            </section>
        </Show>
    }
}
