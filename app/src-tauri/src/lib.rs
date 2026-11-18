use seamone_core::AppInfo;

/// 화면이 앱 이름과 판 번호를 물을 때 부르는 command, 값은 `seamone-core` 가 정한다.
#[tauri::command]
fn get_app_info() -> AppInfo {
    seamone_core::app_info()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_app_info])
        .run(tauri::generate_context!())
        .expect("seamone 을 실행하지 못했다");
}

#[cfg(test)]
mod tests {
    use super::get_app_info;

    #[test]
    fn get_app_info_command_returns_the_core_app_info() {
        assert_eq!(get_app_info(), seamone_core::app_info());
    }
}
