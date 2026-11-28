use crate::api;
use leptos::{prelude::*, task::spawn_local};
use seamone_core::{
    capture::{MAX_RECORDING_SECONDS, Status},
    ipc::{EmptyArgs, FinishRecordingArgs, command},
    review::RecordingInfo,
};
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
    status: RwSignal<Status>,
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
            status: RwSignal::new(Status::Unavailable),
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
                    Status::Starting { .. } | Status::Recording { .. } | Status::Finishing { .. }
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
        matches!(self.status.get(), Status::Idle | Status::Failed { .. })
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
            let result = api::call_typed::<command::GetCaptureStatus>(EmptyArgs {}).await;
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
            match api::call_typed::<command::StartRecording>(EmptyArgs {}).await {
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
            Status::Starting { session_id } if !save => session_id,
            Status::Recording { session_id, .. }
            | Status::LimitReached { session_id }
            | Status::Interrupted { session_id, .. } => session_id,
            _ => return,
        };
        self.begin(if save { Action::Save } else { Action::Cancel });
        spawn_local(async move {
            match api::call_typed::<command::FinishRecording>(FinishRecordingArgs {
                session_id: id,
                save,
            })
            .await
            {
                Ok(Some(info)) => {
                    self.status.set(Status::Idle);
                    on_saved.run(info);
                }
                Ok(None) => {
                    self.status.set(Status::Idle);
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
            Status::Starting { .. }
                | Status::Recording { .. }
                | Status::LimitReached { .. }
                | Status::Interrupted { .. }
        )
    };
    let can_save = move || {
        matches!(
            microphone.status.get(),
            Status::Recording { .. } | Status::LimitReached { .. } | Status::Interrupted { .. }
        )
    };
    view! {
        <Show when=move || match microphone.status.get() {
            Status::Unavailable => false,
            Status::Idle | Status::Failed { .. } => {
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
                <p aria-live="polite">{move || match microphone.action.get() {
                    Action::Start => "마이크를 여는 중입니다.".into(),
                    Action::Save => "녹음을 저장하는 중입니다.".into(),
                    Action::Cancel => "녹음을 취소하는 중입니다.".into(),
                    Action::None => match microphone.status.get() {
                        Status::Starting { .. } => "마이크 입력을 기다리는 중입니다.".into(),
                        Status::Recording { elapsed_seconds, .. } => format!(
                            "녹음 중 {:02}:{:02} / {:02}:00",
                            elapsed_seconds / 60, elapsed_seconds % 60, MAX_RECORDING_SECONDS / 60,
                        ),
                        Status::LimitReached { .. } => "30분 제한에 도달했습니다. 녹음을 저장하거나 취소한 뒤 창을 닫을 수 있습니다.".into(),
                        Status::Interrupted { elapsed_seconds, reason, .. } => format!(
                            "{reason} 받은 음성 {:02}:{:02}. 녹음을 저장하거나 취소한 뒤 창을 닫을 수 있습니다.",
                            elapsed_seconds / 60, elapsed_seconds % 60,
                        ),
                        Status::Finishing { .. } => "녹음을 마무리하는 중입니다.".into(),
                        _ => "기본 마이크로 최대 30분까지 녹음합니다.".into(),
                    }
                }}</p>
                <Show when=move || matches!(microphone.status.get(), Status::Failed { .. })>
                    <p class="error" role="alert">{move || if let Status::Failed { message } = microphone.status.get() { message } else { String::new() }}</p>
                </Show>
            </section>
        </Show>
    }
}
