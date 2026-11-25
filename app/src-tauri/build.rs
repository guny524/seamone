fn main() {
    println!("cargo::rustc-check-cfg=cfg(native_capture)");
    let desktop_capture = matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("linux" | "macos" | "windows")
    );
    if desktop_capture {
        println!("cargo::rustc-cfg=native_capture");
    }
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(seamone_core::ipc::Command::NAMES),
        ),
    )
    .expect("Tauri command 권한을 만들지 못했습니다");
}
