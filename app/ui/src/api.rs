use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], catch)]
    async fn invoke(command: &str, arguments: JsValue) -> Result<JsValue, JsValue>;
}

#[wasm_bindgen(inline_js = "export function eventId() { return crypto.randomUUID(); }")]
extern "C" {
    #[wasm_bindgen(js_name = eventId)]
    pub fn event_id() -> String;
}

pub async fn call<T: DeserializeOwned>(command: &str, args: impl Serialize) -> Result<T, String> {
    let args = args
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| e.to_string())?;
    let value = invoke(command, args)
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))?;
    serde_wasm_bindgen::from_value(value).map_err(|e| e.to_string())
}
