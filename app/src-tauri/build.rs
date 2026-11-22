fn main() {
    println!("cargo::rustc-check-cfg=cfg(native_capture)");
    let desktop_capture = matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("linux" | "macos" | "windows")
    );
    let commands = &[
        "get_app_info",
        "get_library",
        "get_segments",
        "import_audio",
        "get_capture_status",
        "start_recording",
        "finish_recording",
        "transcribe_recording",
        "select_model",
        "get_playback",
        "get_recording_playback",
        "get_error_marks",
        "save_error_mark",
        "save_error_marks",
        "playback_ended",
        "save_review",
        "dismiss_info",
        "export_text",
        "copy_saved_text",
        "save_text_file",
    ];
    if desktop_capture {
        println!("cargo::rustc-cfg=native_capture");
    }
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("Tauri command 권한을 만들지 못했습니다");
}
