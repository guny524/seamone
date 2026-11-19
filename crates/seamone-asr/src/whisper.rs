//! Optional desktop inference. Never downloads weights or enables a local adapter.
use crate::{TranscribeError, Transcriber, check_input};
use seamone_core::TranscriptSegment;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::SMALL_SHA256;

pub struct WhisperTranscriber {
    context: WhisperContext,
}

impl WhisperTranscriber {
    pub fn load(path: &Path) -> Result<Self, TranscribeError> {
        static LOGGING: std::sync::Once = std::sync::Once::new();
        LOGGING.call_once(whisper_rs::install_logging_hooks);
        let unavailable = |e: std::io::Error| TranscribeError::ModelUnavailable(e.to_string());
        let mut file = File::open(path).map_err(unavailable)?;
        let mut hash = Sha256::new();
        let mut buffer = vec![0_u8; 65_536];
        loop {
            let count = file.read(&mut buffer).map_err(unavailable)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if format!("{:x}", hash.finalize()) != SMALL_SHA256 {
            return Err(TranscribeError::ModelUnavailable(
                "공식 Whisper small GGML 모델의 SHA-256이 맞지 않습니다.".into(),
            ));
        }
        let mut params = WhisperContextParameters::default();
        params.use_gpu(false);
        let context = WhisperContext::new_with_params(path, params)
            .map_err(|e| TranscribeError::ModelUnavailable(e.to_string()))?;
        Ok(Self { context })
    }
}

impl Transcriber for WhisperTranscriber {
    fn transcribe(
        &mut self,
        audio: &[f32],
        language: Option<&str>,
    ) -> Result<Vec<TranscriptSegment>, TranscribeError> {
        check_input(audio, language)?;
        if audio.iter().any(|s| s.abs() > 1.0) {
            return Err(TranscribeError::InvalidInput(
                "PCM 표본은 -1부터 1 사이여야 합니다.".into(),
            ));
        }
        if language.is_some_and(|l| whisper_rs::get_lang_id(l).is_none()) {
            return Err(TranscribeError::InvalidInput(
                "지원하지 않는 언어 코드입니다.".into(),
            ));
        }
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(language);
        params.set_translate(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);
        params.set_n_threads(4);
        let mut state = self
            .context
            .create_state()
            .map_err(|e| TranscribeError::Failed(e.to_string()))?;
        state
            .full(params, audio)
            .map_err(|e| TranscribeError::Failed(e.to_string()))?;
        let mut result = Vec::new();
        for i in 0..state.full_n_segments() {
            let segment = state
                .get_segment(i)
                .ok_or_else(|| TranscribeError::Failed("받아쓰기 조각이 없습니다.".into()))?;
            let start = u64::try_from(segment.start_timestamp())
                .map_err(|e| TranscribeError::Failed(e.to_string()))?
                .checked_mul(10)
                .ok_or_else(|| TranscribeError::Failed("시작 시각이 너무 큽니다.".into()))?;
            let end = u64::try_from(segment.end_timestamp())
                .map_err(|e| TranscribeError::Failed(e.to_string()))?
                .checked_mul(10)
                .ok_or_else(|| TranscribeError::Failed("끝 시각이 너무 큽니다.".into()))?;
            let text = segment
                .to_str()
                .map_err(|e| TranscribeError::Failed(e.to_string()))?
                .trim()
                .to_owned();
            if !text.is_empty() {
                result.push(TranscriptSegment::new(start, end, text));
            }
        }
        Ok(result)
    }
}
