//! 받아쓴 글의 조각, 받아쓰기 엔진이 돌려주고 화면이 보여 주고 고친다.

use crate::review::SampleRange;
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

/// Verbatim text sharing one model-aligned span on the recording's 16 kHz sample axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedText {
    pub range: SampleRange,
    pub text: String,
}

#[must_use]
pub fn valid_alignment(timing: &[TimedText], text: &str, line: SampleRange) -> bool {
    let mut remaining = text;
    let mut previous_end = line.start;
    for span in timing {
        if span.text.is_empty()
            || span.range.start < previous_end
            || span.range.start >= span.range.end
            || span.range.end > line.end
        {
            return false;
        }
        let Some(rest) = remaining.strip_prefix(&span.text) else {
            return false;
        };
        remaining = rest;
        previous_end = span.range.end;
    }
    !timing.is_empty() && remaining.is_empty()
}

/// 받아쓴 글 한 조각, 음성의 한 구간(`start_ms` 부터 `end_ms` 까지)에 글 하나가 대응한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// 조각이 시작하는 시각, 음성 처음부터 지난 밀리초
    pub start_ms: u64,
    /// 조각이 끝나는 시각, 음성 처음부터 지난 밀리초, `start_ms` 이상이다
    pub end_ms: u64,
    /// 받아쓴 글
    pub text: String,
    /// 말한 사람의 이름표, 화자 구분은 뒤 이슈가 하므로 지금은 모든 엔진이 `None` 으로 둔다
    pub speaker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<Vec<TimedText>>,
}

impl TranscriptSegment {
    /// 화자 칸을 비운 조각을 만든다.
    #[must_use]
    pub fn new(start_ms: u64, end_ms: u64, text: impl Into<String>) -> Self {
        Self {
            start_ms,
            end_ms,
            text: text.into(),
            speaker: None,
            timing: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TranscriptSegment;

    #[test]
    fn new_segment_has_an_empty_speaker() {
        let segment = TranscriptSegment::new(1_500, 3_200, "안녕하세요");

        assert_eq!(segment.start_ms, 1_500);
        assert_eq!(segment.end_ms, 3_200);
        assert_eq!(segment.text, "안녕하세요");
        assert_eq!(segment.speaker, None);
    }

    #[test]
    fn segment_json_has_exactly_the_four_fields() {
        let value = serde_json::to_value(TranscriptSegment::new(0, 900, "네")).expect("직렬화된다");
        let fields = value.as_object().expect("JSON 객체다");

        let mut names: Vec<&str> = fields.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["end_ms", "speaker", "start_ms", "text"]);
        assert!(fields["speaker"].is_null());
    }

    #[test]
    fn segment_survives_a_json_round_trip_with_a_speaker() {
        let mut segment = TranscriptSegment::new(10, 20, "회의를 시작합니다");
        segment.speaker = Some("화자 1".to_owned());

        let json = serde_json::to_string(&segment).expect("직렬화된다");
        let back: TranscriptSegment = serde_json::from_str(&json).expect("역직렬화된다");

        assert_eq!(back, segment);
    }
}
