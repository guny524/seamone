//! Optional desktop inference. Never downloads weights or enables a local adapter.
use crate::{TranscribeError, Transcriber, check_input};
use seamone_core::{
    TimedText, TranscriptSegment,
    review::{SAMPLE_RATE, SampleRange},
};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};
use unicode_segmentation::UnicodeSegmentation;
use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext,
    WhisperContextParameters, WhisperSegment,
};

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
        params.use_gpu(false).dtw_parameters(DtwParameters {
            mode: DtwMode::ModelPreset {
                model_preset: DtwModelPreset::Small,
            },
            ..DtwParameters::default()
        });
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
            let raw = segment
                .to_str()
                .map_err(|e| TranscribeError::Failed(e.to_string()))?;
            if !raw.trim().is_empty() {
                let mut line = TranscriptSegment::new(start, end, raw.trim());
                let bounds = SampleRange::from_millis(start, end, audio.len() as u64).ok();
                line.timing = bounds.and_then(|range| {
                    aligned_segment(&segment, raw, range, self.context.token_eot())
                });
                result.push(line);
            }
        }
        Ok(result)
    }
}

fn aligned_segment(
    segment: &WhisperSegment<'_>,
    raw: &str,
    range: SampleRange,
    eot: i32,
) -> Option<Vec<TimedText>> {
    let mut tokens = Vec::new();
    for index in 0..segment.n_tokens() {
        let token = segment.get_token(index)?;
        if token.token_id() < eot {
            tokens.push((token.to_bytes().ok()?, token.token_data().t_dtw));
        }
    }
    align_tokens(raw, range, &tokens)
}

fn align_tokens(raw: &str, line: SampleRange, tokens: &[(&[u8], i64)]) -> Option<Vec<TimedText>> {
    let text = raw.trim();
    let first = raw.len() - raw.trim_start().len();
    let end = first + text.len();
    let mut boundaries = text
        .grapheme_indices(true)
        .map(|(byte, _)| first + byte)
        .peekable();
    let mut offset = 0;
    let mut starts = Vec::new();
    let mut previous = line.start;
    for &(bytes, tick) in tokens {
        let next = offset + bytes.len();
        if raw.as_bytes().get(offset..next)? != bytes {
            return None;
        }
        if next > first && offset < end && next > offset {
            // whisper.cpp stores DTW anchors in 10 ms ticks on the original audio axis.
            let sample = u64::try_from(tick).ok()?.checked_mul(SAMPLE_RATE / 100)?;
            if sample < previous || sample >= line.end {
                return None;
            }
            previous = sample;
            // Start a span only at an actual token boundary that is also a grapheme boundary.
            let clipped_start = offset.max(first);
            while boundaries
                .peek()
                .is_some_and(|&boundary| boundary < clipped_start)
            {
                boundaries.next();
            }
            if boundaries.next_if_eq(&clipped_start).is_some()
                && starts.last().is_none_or(|&(_, time)| time < sample)
            {
                starts.push((clipped_start, sample));
            }
        }
        offset = next;
    }
    if offset != raw.len() || starts.first()?.0 != first {
        return None;
    }
    starts.push((end, line.end));
    Some(
        starts
            .windows(2)
            .map(|pair| {
                let [(start, from), (end, to)] = [pair[0], pair[1]];
                TimedText {
                    range: SampleRange {
                        start: from,
                        end: to,
                    },
                    text: raw[start..end].into(),
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_anchors_keep_utf8_fragments_graphemes_and_multi_character_tokens_together() {
        let tokens: &[(&[u8], i64)] = &[
            (&[b' ', 0xea], 10),
            (&[0xb0, 0x80], 11),
            (b"e", 33),
            ("\u{301}".as_bytes(), 34),
            (" 나다".as_bytes(), 59),
            (b"!", 59),
        ];
        let line = SampleRange {
            start: 1_600,
            end: 16_000,
        };
        let timing = align_tokens(" 가e\u{301} 나다!", line, tokens).unwrap();
        assert_eq!(
            timing
                .iter()
                .map(|s| (s.text.as_str(), s.range.start))
                .collect::<Vec<_>>(),
            [("가", 1_600), ("e\u{301}", 5_280), (" 나다!", 9_440)]
        );
        assert_eq!(timing.last().unwrap().range.end, line.end);
        assert!(align_tokens(" 가e\u{301} 다나!", line, tokens).is_none());
        let mut backwards = tokens.to_vec();
        backwards[4].1 = 12;
        assert!(align_tokens(" 가e\u{301} 나다!", line, &backwards).is_none());
    }
}
