use seamone_core::{
    ipc::{ControlPlaybackArgs, command},
    playback::Action,
};
use wasm_bindgen::prelude::*;

mod tauri;

pub(crate) use tauri::call_typed;

#[wasm_bindgen(inline_js = "export function eventId() { return crypto.randomUUID(); }")]
extern "C" {
    #[wasm_bindgen(js_name = eventId)]
    pub fn event_id() -> String;
}

pub fn stop_playback(session: String) {
    leptos::task::spawn_local(async move {
        let _ = call_typed::<command::ControlPlayback>(ControlPlaybackArgs {
            session,
            action: Action::Stop,
        })
        .await;
    });
}
