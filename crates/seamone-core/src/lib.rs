//! seamone 의 도메인 crate.
//!
//! 화면(Leptos, WASM)과 앱 껍데기(Tauri)가 주고받는 타입은 여기에 한 번만 정의하고 양쪽이 같이 쓴다.
//! WASM 으로도 컴파일되므로 파일, 소켓, 스레드처럼 브라우저에 없는 것에 기대지 않는다.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod capture;
pub mod error_marks;
pub mod ipc;
pub mod review;
pub mod review_flow;
mod transcript;

pub use transcript::{TimedText, TranscriptSegment, valid_alignment};

use alloc::{borrow::ToOwned, string::String};
use serde::{Deserialize, Serialize};

/// 앱 이름
pub const APP_NAME: &str = "seamone";

/// 화면이 앱 껍데기에 앱 이름과 판 번호를 물을 때 부르는 Tauri command 의 이름, 껍데기의 함수 이름과 같다.
pub const COMMAND_GET_APP_INFO: &str = ipc::Command::GetAppInfo.name();

/// 앱 이름과 판 번호, 앱 껍데기가 Tauri command 로 돌려주고 화면이 그대로 받는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

/// 앱 이름과 판 번호를 돌려준다, 판 번호는 workspace 전체가 같이 쓴다.
#[must_use]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: APP_NAME.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AppInfo, app_info};

    #[test]
    fn app_info_names_the_app_and_gives_a_semantic_version() {
        let info = app_info();

        assert_eq!(info.name, "seamone");
        let parts: Vec<&str> = info.version.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "판 번호는 주.부.수 세 칸이다: {}",
            info.version
        );
        assert!(
            parts.iter().all(|part| part.parse::<u32>().is_ok()),
            "판 번호의 세 칸은 모두 숫자다: {}",
            info.version
        );
    }

    #[test]
    fn app_info_json_has_exactly_the_name_and_version_fields() {
        let value = serde_json::to_value(app_info()).expect("직렬화된다");
        let fields = value.as_object().expect("JSON 객체다");

        let mut names: Vec<&str> = fields.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["name", "version"]);
        assert!(fields["name"].is_string());
        assert!(fields["version"].is_string());
    }

    #[test]
    fn app_info_survives_a_json_round_trip() {
        let json = serde_json::to_string(&app_info()).expect("직렬화된다");

        let back: AppInfo = serde_json::from_str(&json).expect("역직렬화된다");

        assert_eq!(back, app_info());
    }
}
