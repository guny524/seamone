//! Fixtures for IPC tests, enabled only by the host's development dependency.

use crate::{LibraryService, audio::canonical_spec};
use seamone_core::review::Segment;
use std::path::Path;

/// # Errors
/// 보관함 생성과 DB 초기화가 실패하면 오류를 돌려준다
pub fn open(root: &Path) -> Result<LibraryService, String> {
    let service = LibraryService::open(root)?;
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    let service = LibraryService {
        recorder: crate::capture::Recorder::fake(),
        ..service
    };
    Ok(service)
}

/// # Errors
/// 시험용 WAV 생성, 가져오기, 전사 저장이 실패하면 오류를 돌려준다
pub fn transcribed(root: &Path) -> Result<(LibraryService, Segment), String> {
    let service = open(root)?;
    let source = root.join("test-input.wav");
    let mut wav = hound::WavWriter::create(&source, canonical_spec()).map_err(|e| e.to_string())?;
    for sample in 0_i16..16_000 {
        wav.write_sample(sample).map_err(|e| e.to_string())?;
    }
    wav.finalize().map_err(|e| e.to_string())?;
    let info = service.import_path(&source, || true)?;
    let mut engine = seamone_asr::FakeTranscriber::new().with_texts(vec!["네 안녕하세요".into()]);
    let segments = service
        .store()?
        .transcribe(&info.id, &mut engine, "test", "fixture-v1")?;
    Ok((
        service,
        segments
            .into_iter()
            .next()
            .ok_or("시험용 전사 조각이 없습니다.")?,
    ))
}
