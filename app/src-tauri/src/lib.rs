use seamone_core::AppInfo;

/// 화면이 앱 이름과 판 번호를 물을 때 부르는 command, 값은 `seamone-core` 가 정한다.
#[tauri::command]
fn get_app_info() -> AppInfo {
    seamone_core::app_info()
}

/// 앱 껍데기가 화면에 내주는 command 를 등록한다, 실제 앱과 시험이 같은 등록을 쓴다.
fn register_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![get_app_info])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    register_commands(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("seamone 을 실행하지 못했다");
}

#[cfg(test)]
mod tests {
    use super::register_commands;
    use seamone_core::{AppInfo, COMMAND_GET_APP_INFO};
    use tauri::http::HeaderMap;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
    use tauri::webview::InvokeRequest;
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    #[test]
    fn registered_command_answers_over_ipc_with_the_core_app_info() {
        let app = register_commands(mock_builder())
            .build(mock_context(noop_assets()))
            .expect("앱을 만든다");
        let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .expect("창을 만든다");

        let response = get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: COMMAND_GET_APP_INFO.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                // 창이 열린 주소를 그대로 쓴다, 플랫폼마다 앱의 로컬 주소가 달라 고정한 주소는 원격으로 취급돼 거절된다
                url: webview.url().expect("창 주소를 읽는다"),
                body: InvokeBody::default(),
                headers: HeaderMap::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        );

        let info: AppInfo = response
            .expect("command 가 응답한다")
            .deserialize()
            .expect("응답이 AppInfo 로 읽힌다");
        assert_eq!(info, seamone_core::app_info());
    }
}
