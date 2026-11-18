//! 받아쓰기 엔진 interface.
//!
//! 앱은 이 interface 만 안다, 기기마다 다른 엔진(CPU, Apple GPU, 폰 가속기)은 이 interface 뒤에서 갈아 끼운다.
//! 엔진은 cargo feature 로 켜고 끈다, 기본 feature 에는 엔진이 없고 시험용 가짜 엔진만 있다.
#![forbid(unsafe_code)]

mod fake;
pub mod speed;

pub use fake::FakeTranscriber;

use std::fmt;

use seamone_core::TranscriptSegment;

/// 엔진이 받는 음성의 표본 속도(Hz), 모든 엔진이 이 속도의 mono 음성만 받는다.
pub const SAMPLE_RATE_HZ: u32 = 16_000;

/// 음성을 글로 받아쓰는 엔진.
///
/// 입력 `audio` 는 16kHz mono 표본이고 값은 -1.0 에서 1.0 사이의 `f32` 다.
/// `language` 는 언어 힌트(`ko`, `en` 같은 언어 코드)다, 없으면(`None`) 엔진이 음성에서 언어를 스스로 가려
/// 받아쓰고, 있으면 그 언어로 받아쓴다.
///
/// 계약
/// - 입력이 잘못되면(빈 음성, 숫자가 아닌 표본, 빈 언어 힌트, 엔진이 받지 못하는 언어 힌트)
///   모델을 불러오기 전에 `TranscribeError::InvalidInput` 으로 거절한다
/// - 모델을 불러오지 못하면 `TranscribeError::ModelUnavailable`, 받아쓰는 도중 엔진이 실패하면
///   `TranscribeError::Failed` 다
/// - 성공하면 조각은 `start_ms` 가 작은 것부터 늘어서고 조각마다 `start_ms` 가 `end_ms` 이하다,
///   말이 없는 음성은 빈 목록이다
/// - 화자 칸(`speaker`)은 `None` 으로 둔다
pub trait Transcriber {
    /// 음성을 받아써 전사 조각의 목록을 돌려준다.
    fn transcribe(
        &mut self,
        audio: &[f32],
        language: Option<&str>,
    ) -> Result<Vec<TranscriptSegment>, TranscribeError>;
}

/// 받아쓰기가 실패한 까닭, 호출하는 쪽이 고칠 수 있는 입력 문제와 엔진 쪽 문제를 가른다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscribeError {
    /// 엔진이 모델을 불러오지 못했다(파일이 없거나 깨졌거나 이 기기가 지원하지 않는다)
    ModelUnavailable(String),
    /// 입력이 계약에 맞지 않는다, 호출하는 쪽이 입력을 고쳐야 한다
    InvalidInput(String),
    /// 받아쓰는 도중 엔진이 실패했다
    Failed(String),
}

impl fmt::Display for TranscribeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelUnavailable(reason) => write!(f, "모델을 불러오지 못했다: {reason}"),
            Self::InvalidInput(reason) => write!(f, "입력이 잘못됐다: {reason}"),
            Self::Failed(reason) => write!(f, "받아쓰다 실패했다: {reason}"),
        }
    }
}

impl std::error::Error for TranscribeError {}

/// 모든 엔진이 받아쓰기 전에 부르는 입력 검사, 빈 음성과 숫자가 아닌 표본과 빈 언어 힌트를 거절한다.
/// 엔진이 받지 못하는 언어인지는 엔진마다 다르므로 이 검사가 아니라 엔진이 가린다.
pub fn check_input(audio: &[f32], language: Option<&str>) -> Result<(), TranscribeError> {
    if audio.is_empty() {
        return Err(TranscribeError::InvalidInput("음성이 비어 있다".to_owned()));
    }
    if let Some(index) = audio.iter().position(|sample| !sample.is_finite()) {
        return Err(TranscribeError::InvalidInput(format!(
            "{index} 번째 표본이 숫자가 아니다"
        )));
    }
    if language.is_some_and(str::is_empty) {
        return Err(TranscribeError::InvalidInput(
            "언어 힌트가 비어 있다".to_owned(),
        ));
    }
    Ok(())
}

/// 표본 수에서 음성 길이(밀리초)를 구한다, 나머지는 올림한다.
#[must_use]
pub fn audio_ms(sample_count: usize) -> u64 {
    let samples = u64::try_from(sample_count).unwrap_or(u64::MAX);
    samples
        .saturating_mul(1_000)
        .div_ceil(u64::from(SAMPLE_RATE_HZ))
}

#[cfg(test)]
mod tests {
    use super::{
        FakeTranscriber, SAMPLE_RATE_HZ, TranscribeError, Transcriber, TranscriptSegment, audio_ms,
        check_input,
    };

    /// 받아쓴 결과가 `Transcriber` 의 계약(조각이 시작 시각 순서이고 조각마다 시작이 끝 이하)을 지키는지 본다.
    fn assert_follows_the_contract(segments: &[TranscriptSegment]) {
        for segment in segments {
            assert!(
                segment.start_ms <= segment.end_ms,
                "시작이 끝보다 늦다: {segment:?}"
            );
            assert_eq!(segment.speaker, None, "화자 칸은 비워 둔다: {segment:?}");
        }
        for pair in segments.windows(2) {
            assert!(
                pair[0].start_ms <= pair[1].start_ms,
                "시작 시각 순서가 아니다: {pair:?}"
            );
        }
    }

    /// 길이가 `ms` 밀리초인 소리 있는 음성
    fn audio_of(ms: usize) -> Vec<f32> {
        vec![0.1; ms * SAMPLE_RATE_HZ as usize / 1_000]
    }

    #[test]
    fn audio_ms_converts_sample_count_and_rounds_up() {
        assert_eq!(audio_ms(0), 0);
        assert_eq!(audio_ms(16_000), 1_000);
        assert_eq!(audio_ms(40_000), 2_500);
        assert_eq!(audio_ms(1), 1);
    }

    #[test]
    fn transcribe_returns_ordered_segments_that_cover_the_whole_audio() {
        let mut engine = FakeTranscriber::new();

        let segments = engine.transcribe(&audio_of(2_500), None).expect("받아쓴다");

        assert_follows_the_contract(&segments);
        let spans: Vec<(u64, u64)> = segments.iter().map(|s| (s.start_ms, s.end_ms)).collect();
        assert_eq!(spans, [(0, 1_000), (1_000, 2_000), (2_000, 2_500)]);
        assert_eq!(segments[0].text, "조각 1");
    }

    #[test]
    fn transcribe_uses_the_given_texts_and_repeats_them() {
        let mut engine = FakeTranscriber::new()
            .with_segment_ms(500)
            .with_texts(vec!["안녕".to_owned(), "하세요".to_owned()]);

        let segments = engine
            .transcribe(&audio_of(1_500), Some("ko"))
            .expect("받아쓴다");

        let texts: Vec<&str> = segments.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["안녕", "하세요", "안녕"]);
    }

    #[test]
    fn missing_language_hint_is_accepted() {
        let mut engine = FakeTranscriber::new();
        let audio = audio_of(1_000);

        let without_hint = engine.transcribe(&audio, None).expect("힌트 없이 받아쓴다");
        let with_hint = engine
            .transcribe(&audio, Some("ko"))
            .expect("힌트와 함께 받아쓴다");

        assert_eq!(without_hint, with_hint);
    }

    #[test]
    fn empty_audio_is_invalid_input() {
        let mut engine = FakeTranscriber::new();

        let error = engine.transcribe(&[], None).expect_err("거절한다");

        assert!(matches!(error, TranscribeError::InvalidInput(_)), "{error}");
    }

    #[test]
    fn non_finite_sample_is_invalid_input() {
        let mut engine = FakeTranscriber::new();
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut audio = audio_of(1_000);
            audio[7] = bad;

            let error = engine.transcribe(&audio, None).expect_err("거절한다");

            assert!(matches!(error, TranscribeError::InvalidInput(_)), "{error}");
        }
    }

    #[test]
    fn empty_language_hint_is_invalid_input() {
        let mut engine = FakeTranscriber::new();

        let error = engine
            .transcribe(&audio_of(1_000), Some(""))
            .expect_err("거절한다");

        assert!(matches!(error, TranscribeError::InvalidInput(_)), "{error}");
    }

    #[test]
    fn language_the_engine_cannot_take_is_invalid_input() {
        let mut engine = FakeTranscriber::new();

        let error = engine
            .transcribe(&audio_of(1_000), Some("xx"))
            .expect_err("거절한다");

        assert!(matches!(error, TranscribeError::InvalidInput(_)), "{error}");
    }

    #[test]
    fn unavailable_model_is_told_apart_from_invalid_input() {
        let mut engine = FakeTranscriber::new().with_unavailable_model("모델 파일이 없다");

        let unavailable = engine
            .transcribe(&audio_of(1_000), None)
            .expect_err("모델이 없다");
        let invalid = engine.transcribe(&[], None).expect_err("입력이 잘못됐다");

        assert_eq!(
            unavailable,
            TranscribeError::ModelUnavailable("모델 파일이 없다".to_owned())
        );
        assert!(
            matches!(invalid, TranscribeError::InvalidInput(_)),
            "입력 검사가 모델 불러오기보다 먼저다: {invalid}"
        );
    }

    #[test]
    fn check_input_accepts_valid_input() {
        assert_eq!(check_input(&[0.0, 0.5, -0.5], None), Ok(()));
        assert_eq!(check_input(&[0.0], Some("ko")), Ok(()));
    }

    #[test]
    fn error_message_names_the_cause() {
        let message = TranscribeError::ModelUnavailable("파일이 없다".to_owned()).to_string();

        assert!(message.contains("모델"), "{message}");
        assert!(message.contains("파일이 없다"), "{message}");
    }

    #[test]
    fn engines_can_be_used_through_the_trait_object() {
        let mut engine: Box<dyn Transcriber> = Box::new(FakeTranscriber::new());

        let segments = engine
            .transcribe(&audio_of(1_000), Some("en"))
            .expect("받아쓴다");

        assert_follows_the_contract(&segments);
        assert_eq!(segments.len(), 1);
    }
}
