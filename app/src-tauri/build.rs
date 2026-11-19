fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_app_info",
            "get_library",
            "get_segments",
            "import_wav",
            "transcribe_recording",
            "select_model",
            "get_playback",
            "playback_ended",
            "save_review",
            "dismiss_info",
            "export_text",
            "copy_saved_text",
            "save_text_file",
        ]),
    ))
    .expect("Tauri command 권한을 만들지 못했습니다");
}
