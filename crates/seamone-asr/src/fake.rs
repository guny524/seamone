//! 시험과 측정 CLI 의 연습 실행에 쓰는 가짜 엔진.

use seamone_core::TranscriptSegment;

use crate::{TranscribeError, Transcriber, audio_ms, check_input};

/// 가짜 엔진이 받는 언어 힌트
const SUPPORTED_LANGUAGES: [&str; 2] = ["ko", "en"];

/// 모델 없이 음성을 일정한 길이로 잘라 정해 둔 글을 돌려주는 가짜 엔진, `Transcriber` 의 계약을 지키는 가장 단순한 구현이다.
#[derive(Debug, Clone)]
pub struct FakeTranscriber {
    segment_ms: u64,
    texts: Vec<String>,
    model_error: Option<String>,
}

impl Default for FakeTranscriber {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeTranscriber {
    /// 1초마다 한 조각을 만들고 글은 "조각 1", "조각 2" 로 이어진다.
    #[must_use]
    pub fn new() -> Self {
        Self {
            segment_ms: 1_000,
            texts: Vec::new(),
            model_error: None,
        }
    }

    /// 조각 하나의 길이(밀리초)를 정한다, 0 이면 1 로 본다.
    #[must_use]
    pub fn with_segment_ms(mut self, segment_ms: u64) -> Self {
        self.segment_ms = segment_ms.max(1);
        self
    }

    /// 조각마다 돌려줄 글을 정한다, 조각이 글보다 많으면 처음부터 되풀이한다.
    #[must_use]
    pub fn with_texts(mut self, texts: Vec<String>) -> Self {
        self.texts = texts;
        self
    }

    /// 모델을 불러오지 못한 엔진을 흉내 낸다, 입력이 맞아도 `TranscribeError::ModelUnavailable` 로 끝난다.
    #[must_use]
    pub fn with_unavailable_model(mut self, reason: impl Into<String>) -> Self {
        self.model_error = Some(reason.into());
        self
    }

    fn text_of(&self, index: usize) -> String {
        if self.texts.is_empty() {
            format!("조각 {}", index + 1)
        } else {
            self.texts[index % self.texts.len()].clone()
        }
    }
}

impl Transcriber for FakeTranscriber {
    fn transcribe(
        &mut self,
        audio: &[f32],
        language: Option<&str>,
    ) -> Result<Vec<TranscriptSegment>, TranscribeError> {
        check_input(audio, language)?;
        if let Some(code) = language
            && !SUPPORTED_LANGUAGES.contains(&code)
        {
            return Err(TranscribeError::InvalidInput(format!(
                "가짜 엔진이 받지 못하는 언어 힌트다: {code}"
            )));
        }
        if let Some(reason) = &self.model_error {
            return Err(TranscribeError::ModelUnavailable(reason.clone()));
        }

        let total_ms = audio_ms(audio.len());
        let mut segments = Vec::new();
        let mut start_ms = 0;
        while start_ms < total_ms {
            let end_ms = start_ms.saturating_add(self.segment_ms).min(total_ms);
            segments.push(TranscriptSegment::new(
                start_ms,
                end_ms,
                self.text_of(segments.len()),
            ));
            start_ms = end_ms;
        }
        Ok(segments)
    }
}
