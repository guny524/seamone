use leptos::prelude::*;
use leptos::task::spawn_local;
use seamone_core::{AppInfo, COMMAND_GET_APP_INFO};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// Tauri 가 창에 넣어 주는 `window.__TAURI__.core.invoke`, 앱 껍데기의 command 를 부른다.
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], catch)]
    async fn invoke(command: &str) -> Result<JsValue, JsValue>;
}

/// 앱 껍데기의 `COMMAND_GET_APP_INFO` command 를 불러 앱 이름과 판 번호를 받는다.
async fn fetch_app_info() -> Result<AppInfo, String> {
    let value = invoke(COMMAND_GET_APP_INFO)
        .await
        .map_err(|error| format!("{error:?}"))?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

#[component]
pub fn App() -> impl IntoView {
    let app_info = RwSignal::new(None::<Result<AppInfo, String>>);
    spawn_local(async move {
        app_info.set(Some(fetch_app_info().await));
    });

    view! {
        <main class="container">
            <h1>"seamone"</h1>
            {move || match app_info.get() {
                None => view! { <p class="status">"불러오는 중"</p> }.into_any(),
                Some(Ok(info)) => {
                    view! { <p class="status">{format!("{} {}", info.name, info.version)}</p> }
                        .into_any()
                }
                Some(Err(message)) => view! { <p class="error">{message}</p> }.into_any(),
            }}
        </main>
    }
}
