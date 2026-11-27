use crate::LibraryService;
#[cfg(all(
    feature = "desktop-whisper",
    not(any(target_os = "android", target_os = "ios"))
))]
use seamone_asr::Transcriber;
use seamone_core::review::Segment;
use std::path::Path;

#[must_use]
pub const fn available() -> bool {
    cfg!(all(
        feature = "desktop-whisper",
        not(any(target_os = "android", target_os = "ios"))
    ))
}

pub fn select(state: &LibraryService, path: &Path) -> Result<(), String> {
    if !available() {
        return Err("이 설치본에는 Whisper 엔진이 없습니다.".into());
    }
    if crate::audio::digest(path)? != SMALL_SHA256 {
        return Err("Whisper small 모델의 SHA-256이 맞지 않습니다. README에 지정한 공식 GGML 파일을 고르세요.".into());
    }
    state.store()?.set_setting(
        "whisper_model",
        path.to_str().ok_or("모델 경로를 읽지 못했습니다.")?,
    )
}

pub use seamone_asr::SMALL_SHA256;

pub fn transcribe(state: &LibraryService, id: &str) -> Result<Vec<Segment>, String> {
    let _foreground = state.background.foreground.enter();
    #[cfg(all(
        feature = "desktop-whisper",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    {
        let path = state.store()?.setting("whisper_model")?.ok_or("먼저 'Whisper 모델 선택'에서 공식 ggml-small.bin을 고르세요. 앱은 모델을 내려받지 않습니다.")?;
        let mut engine =
            seamone_asr::WhisperTranscriber::load(Path::new(&path)).map_err(|e| e.to_string())?;
        let (info, audio) = state.store()?.prepare_transcription(id)?;
        let segments = engine
            .transcribe(&audio, Some("ko"))
            .map_err(|e| e.to_string())?;
        state
            .store()?
            .commit_transcription(&info, segments, "whisper-rs 0.16", SMALL_SHA256)
    }
    #[cfg(not(all(
        feature = "desktop-whisper",
        not(any(target_os = "android", target_os = "ios"))
    )))]
    {
        let _ = (state, id);
        Err("이 설치본에는 Whisper 엔진이 없습니다. desktop-whisper feature로 만든 데스크톱 설치본을 사용하세요. 가져온 WAV와 수정본은 보관함에 남습니다.".into())
    }
}
