//! Human review and training eligibility, shared by native storage and the WASM UI.
use crate::transcript::{TimedText, valid_alignment};
use alloc::{string::String, vec::Vec};
use core::fmt;
use serde::{Deserialize, Serialize};

pub const SAMPLE_RATE: u64 = 16_000;
pub const MAX_PLAYBACK_SAMPLES: u64 = SAMPLE_RATE * 60;
pub const MAX_TEXT_BYTES: usize = 32_768;
pub const MAX_RECORDING_TEXT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleRange {
    pub start: u64,
    pub end: u64,
}

impl SampleRange {
    /// Convert the existing transcriber milliseconds at the native boundary. End is exclusive.
    /// # Errors
    /// Returns `InvalidRange` when converting either millisecond offset overflows.
    pub fn from_millis(start: u64, end: u64, samples: u64) -> Result<Self, Error> {
        let start = start.checked_mul(16).ok_or(Error::InvalidRange)?;
        let end = end.checked_mul(16).ok_or(Error::InvalidRange)?;
        Ok(Self {
            start: start.min(samples),
            end: end.min(samples),
        })
    }

    /// # Errors
    /// Returns `InvalidRange` for empty, reversed, out-of-source or overlong playback spans.
    pub fn validate(self, samples: u64) -> Result<(), Error> {
        if self.end > samples
            || self.start >= self.end
            || self.end - self.start > MAX_PLAYBACK_SAMPLES
        {
            return Err(Error::InvalidRange);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelSource {
    Accepted,
    Corrected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A stored review snapshot, constructed by the review transition.
/// Its JSON representation remains readable for persistence and IPC.
///
/// ```compile_fail,E0451
/// use seamone_core::review::{LabelSource, VerifiedText};
/// fn snapshot(existing: VerifiedText) {
///     let value = VerifiedText { revision: 2, text: "안녕".into(), source: LabelSource::Accepted };
///     let _ = (value.revision(), value.text(), value.source());
/// }
/// ```
///
/// ```
/// use seamone_core::review::{LabelSource, VerifiedText};
/// fn snapshot(existing: VerifiedText) {
///     let value = existing.clone();
///     let _ = (value.revision(), value.text(), value.source());
/// }
/// ```
pub struct VerifiedText {
    revision: u64,
    text: String,
    source: LabelSource,
}

impl VerifiedText {
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn source(&self) -> LabelSource {
        self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub recording_id: String,
    pub recording_revision: u64,
    pub revision: u64,
    pub range: SampleRange,
    pub machine_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_timing: Option<Vec<TimedText>>,
    pub text: String,
    pub verdict: Option<Verdict>,
    /// Draft edits need a separate pass without inventing a binary No verdict.
    pub needs_correction: bool,
    /// Text at the latest error mark, retained through subsequent draft saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_marked_text: Option<String>,
    pub verified: Option<VerifiedText>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Judge { verdict: Verdict },
    SaveDraft { text: String },
    CorrectAndConfirm { text: String },
    ConfirmCorrection,
    DismissErrorMark,
    Invalidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    None,
    Playback,
    PlaybackAndMatchingSource,
}

impl Evidence {
    #[must_use]
    pub const fn requires_playback(self) -> bool {
        match self {
            Self::None => false,
            Self::Playback | Self::PlaybackAndMatchingSource => true,
        }
    }
}

impl Action {
    #[must_use]
    pub const fn required_evidence(&self) -> Evidence {
        match self {
            Self::Judge {
                verdict: Verdict::Yes,
            }
            | Self::CorrectAndConfirm { .. }
            | Self::ConfirmCorrection => Evidence::PlaybackAndMatchingSource,
            Self::Judge {
                verdict: Verdict::No,
            } => Evidence::Playback,
            Self::SaveDraft { .. } | Self::DismissErrorMark | Self::Invalidate => Evidence::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub event_id: String,
    pub segment_id: String,
    pub expected_revision: u64,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
    StaleRevision,
    InvalidRange,
    InvalidText,
    InvalidTransition,
    MissingSource,
    NotVerified,
    UnchangedRejected,
    UnchangedErrorMarked,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::StaleRevision => "다른 수정본이 저장됐습니다. 목록을 새로 불러와 다시 확인하세요.",
            Self::InvalidRange => "재생 구간은 원본 안의 0초 초과, 60초 이하 구간이어야 합니다. 이 조각은 검증하지 않고 나갈 수 있습니다.",
            Self::InvalidText => "글을 비우거나 32KB를 넘겨 저장할 수 없습니다.",
            Self::InvalidTransition => "현재 수정본에서는 이 검증을 저장할 수 없습니다.",
            Self::MissingSource => "녹음 원본이 없거나 바뀌었습니다. 이 조각은 학습에 쓰지 않습니다.",
            Self::NotVerified => "이 수정본에는 사람이 확인한 정답이 없습니다.",
            Self::UnchangedRejected => "처음 No로 판정한 글과 같습니다. 처음 판정을 취소하고 다시 검수하세요.",
            Self::UnchangedErrorMarked => "오류를 표시할 때 저장된 글과 같습니다. 글을 고치거나 이 조각의 오류 판정을 해제하세요.",
        })
    }
}

impl core::error::Error for Error {}

impl Segment {
    #[must_use]
    pub fn display_timing(&self) -> Option<&[TimedText]> {
        self.machine_timing.as_deref().filter(|timing| {
            self.text == self.machine_text && valid_alignment(timing, &self.text, self.range)
        })
    }

    #[must_use]
    pub fn rejected_text_unchanged(&self, text: &str) -> bool {
        self.verdict == Some(Verdict::No) && text.trim() == self.machine_text.trim()
    }

    #[must_use]
    pub fn needs_review(&self) -> bool {
        self.needs_correction || self.error_marked_text.is_some()
    }

    #[must_use]
    pub fn awaits_correction(&self) -> bool {
        self.needs_review() && self.verified.is_none()
    }

    #[must_use]
    pub fn correction_error(&self, text: &str) -> Option<Error> {
        if self.rejected_text_unchanged(text) {
            return Some(Error::UnchangedRejected);
        }
        self.error_marked_text
            .as_ref()
            .filter(|marked| marked.trim() == text.trim())
            .map(|_| Error::UnchangedErrorMarked)
    }

    #[must_use]
    pub fn can_confirm_correction(&self, text: &str) -> bool {
        self.needs_review() && self.verified.is_none() && self.correction_error(text).is_none()
    }

    /// Listening is enforced by the review controller. Storage performs exact-revision CAS.
    /// # Errors
    /// Rejects stale revisions, invalid state transitions, invalid text or invalid audio spans.
    pub fn apply(&self, request: &Request, samples: u64) -> Result<Self, Error> {
        if request.segment_id != self.id || request.expected_revision != self.revision {
            return Err(Error::StaleRevision);
        }
        if matches!(&request.action, Action::SaveDraft { text } if text == &self.text) {
            return Ok(self.clone());
        }
        let mut next = self.clone();
        next.revision = self.revision.checked_add(1).ok_or(Error::StaleRevision)?;
        next.verified = None;
        match &request.action {
            Action::Judge { verdict } => {
                self.range.validate(samples)?;
                if self.verdict.is_some() || self.needs_review() || self.text != self.machine_text {
                    return Err(Error::InvalidTransition);
                }
                next.verdict = Some(*verdict);
                next.needs_correction = *verdict == Verdict::No;
                if *verdict == Verdict::Yes {
                    validate_text(&self.text)?;
                    next.verified = Some(VerifiedText {
                        revision: next.revision,
                        text: self.text.clone(),
                        source: LabelSource::Accepted,
                    });
                }
            }
            Action::SaveDraft { text } => {
                validate_text(text)?;
                next.text.clone_from(text);
                next.needs_correction = true;
            }
            Action::ConfirmCorrection | Action::CorrectAndConfirm { .. } => {
                let text = if let Action::CorrectAndConfirm { text } = &request.action {
                    text
                } else {
                    &self.text
                };
                self.range.validate(samples)?;
                validate_text(text)?;
                if let Some(error) = self.correction_error(text) {
                    return Err(error);
                }
                if !self.needs_review() || self.verified.is_some() {
                    return Err(Error::InvalidTransition);
                }
                next.needs_correction = true;
                next.text.clone_from(text);
                next.verified = Some(VerifiedText {
                    revision: next.revision,
                    text: text.clone(),
                    source: LabelSource::Corrected,
                });
            }
            Action::DismissErrorMark => {
                if self.verified.is_some() || next.error_marked_text.take().is_none() {
                    return Err(Error::InvalidTransition);
                }
            }
            Action::Invalidate => {
                next.verdict = None;
                if self.rejected_text_unchanged(&self.text) {
                    next.text.clone_from(&self.machine_text);
                    next.needs_correction = false;
                } else {
                    next.needs_correction = self.text != self.machine_text || self.needs_correction;
                }
            }
        }
        Ok(next)
    }
}

/// # Errors
/// Returns `InvalidText` for blank text or text exceeding the byte limit.
pub fn validate_text(text: &str) -> Result<(), Error> {
    if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
        return Err(Error::InvalidText);
    }
    Ok(())
}

/// Constructed only after checking a current verified revision and its immutable source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EligibleExample {
    segment: Segment,
}

impl EligibleExample {
    /// # Errors
    /// Rejects missing audio, invalid spans, stale revisions or text without a current valid label.
    pub fn check(segment: &Segment, samples: u64, source_exists: bool) -> Result<Self, Error> {
        if !source_exists {
            return Err(Error::MissingSource);
        }
        segment.range.validate(samples)?;
        let label = segment.verified.as_ref().ok_or(Error::NotVerified)?;
        if label.revision != segment.revision || label.text != segment.text {
            return Err(Error::StaleRevision);
        }
        validate_text(&label.text)?;
        let valid = match label.source {
            LabelSource::Accepted => {
                segment.verdict == Some(Verdict::Yes)
                    && !segment.needs_review()
                    && label.text == segment.machine_text
            }
            LabelSource::Corrected => {
                segment.needs_review() && segment.correction_error(&label.text).is_none()
            }
        };
        if !valid {
            return Err(Error::NotVerified);
        }
        Ok(Self {
            segment: segment.clone(),
        })
    }

    #[must_use]
    pub fn segment(&self) -> &Segment {
        &self.segment
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingInfo {
    pub id: String,
    pub name: String,
    pub samples: u64,
    pub revision: u64,
    pub engine: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryPage {
    pub recordings: Vec<RecordingInfo>,
    pub info_seen: bool,
    pub pending_training: u64,
    pub retry_training: u64,
    pub inference_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackReply {
    pub segment_id: String,
    pub revision: u64,
    pub wav_base64: String,
    pub session: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentPage {
    pub segments: Vec<Segment>,
    pub next_offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn segment() -> Segment {
        Segment {
            id: "s".into(),
            recording_id: "r".into(),
            recording_revision: 1,
            revision: 1,
            range: SampleRange {
                start: 16,
                end: 1600,
            },
            machine_text: "안녕".into(),
            machine_timing: None,
            text: "안녕".into(),
            verdict: None,
            needs_correction: false,
            error_marked_text: None,
            verified: None,
        }
    }
    fn request(s: &Segment, action: Action) -> Request {
        Request {
            event_id: "e".into(),
            segment_id: s.id.clone(),
            expected_revision: s.revision,
            action,
        }
    }
    #[test]
    fn unchanged_text_requires_yes_and_no_is_never_a_label() {
        let s = segment();
        assert_eq!(
            EligibleExample::check(&s, 1600, true),
            Err(Error::NotVerified)
        );
        let no = s
            .apply(
                &request(
                    &s,
                    Action::Judge {
                        verdict: Verdict::No,
                    },
                ),
                1600,
            )
            .unwrap();
        assert!(no.verified.is_none());
        let yes = s
            .apply(
                &request(
                    &s,
                    Action::Judge {
                        verdict: Verdict::Yes,
                    },
                ),
                1600,
            )
            .unwrap();
        assert!(EligibleExample::check(&yes, 1600, true).is_ok());
        assert_eq!(yes.verified.unwrap().source, LabelSource::Accepted);
    }

    #[test]
    fn verified_snapshot_preserves_its_database_and_ipc_json() {
        let current = segment();
        let accepted = current
            .apply(
                &request(
                    &current,
                    Action::Judge {
                        verdict: Verdict::Yes,
                    },
                ),
                1600,
            )
            .unwrap()
            .verified
            .unwrap();
        let wire = serde_json::json!({"revision": 2, "text": "안녕", "source": "accepted"});
        assert_eq!(serde_json::to_value(&accepted).unwrap(), wire);
        let loaded: VerifiedText = serde_json::from_value(wire).unwrap();
        assert_eq!(loaded, accepted);
        assert_eq!(loaded.revision(), 2);
        assert_eq!(loaded.text(), "안녕");
        assert_eq!(loaded.source(), LabelSource::Accepted);
        let draft = current
            .apply(
                &request(
                    &current,
                    Action::SaveDraft {
                        text: "안녕하세요".into(),
                    },
                ),
                1600,
            )
            .unwrap();
        let corrected = draft
            .apply(&request(&draft, Action::ConfirmCorrection), 1600)
            .unwrap()
            .verified
            .unwrap();
        let wire = serde_json::json!({"revision": 3, "text": "안녕하세요", "source": "corrected"});
        assert_eq!(serde_json::to_value(&corrected).unwrap(), wire);
        assert_eq!(
            serde_json::from_value::<VerifiedText>(wire).unwrap(),
            corrected
        );
    }
    #[test]
    fn arbitrary_edits_need_a_separate_verbatim_confirmation() {
        let s = segment();
        let draft = s
            .apply(
                &request(
                    &s,
                    Action::SaveDraft {
                        text: "안녕하세요".into(),
                    },
                ),
                1600,
            )
            .unwrap();
        assert!(draft.verified.is_none());
        let confirmed = draft
            .apply(&request(&draft, Action::ConfirmCorrection), 1600)
            .unwrap();
        assert_eq!(
            confirmed.verified.as_ref().unwrap().source,
            LabelSource::Corrected
        );
        assert!(EligibleExample::check(&confirmed, 1600, true).is_ok());
        let edited = confirmed
            .apply(
                &request(
                    &confirmed,
                    Action::SaveDraft {
                        text: "다시 수정".into(),
                    },
                ),
                1600,
            )
            .unwrap();
        assert!(EligibleExample::check(&edited, 1600, true).is_err());
        assert!(EligibleExample::check(&confirmed, 1600, false).is_err());
    }
    #[test]
    fn exact_revision_and_positive_bounded_audio_are_required() {
        let s = segment();
        let mut req = request(
            &s,
            Action::Judge {
                verdict: Verdict::Yes,
            },
        );
        req.expected_revision = 0;
        assert_eq!(s.apply(&req, 1600), Err(Error::StaleRevision));
        assert_eq!(
            SampleRange::from_millis(u64::MAX, u64::MAX, 100),
            Err(Error::InvalidRange)
        );
        assert_eq!(
            SampleRange::from_millis(1, 200, 100).unwrap(),
            SampleRange {
                start: 16,
                end: 100
            }
        );
        for range in [
            SampleRange { start: 0, end: 0 },
            SampleRange {
                start: 0,
                end: MAX_PLAYBACK_SAMPLES + 1,
            },
            SampleRange {
                start: 1,
                end: 1601,
            },
        ] {
            let mut bad = s.clone();
            bad.range = range;
            assert!(
                bad.apply(
                    &request(
                        &bad,
                        Action::Judge {
                            verdict: Verdict::Yes
                        }
                    ),
                    1600
                )
                .is_err()
            );
        }
    }

    #[test]
    fn a_rejected_unchanged_corrected_snapshot_is_ineligible() {
        let mut s = segment();
        s.verdict = Some(Verdict::No);
        s.needs_correction = true;
        s.verified = Some(VerifiedText {
            revision: s.revision,
            text: s.text.clone(),
            source: LabelSource::Corrected,
        });
        assert_eq!(
            EligibleExample::check(&s, 1600, true),
            Err(Error::NotVerified)
        );
    }

    #[test]
    fn an_unchanged_marked_snapshot_is_ineligible() {
        let mut s = segment();
        s.error_marked_text = Some("이전에 편집한 글".into());
        s.text = "  이전에 편집한 글  ".into();
        s.needs_correction = true;
        s.verified = Some(VerifiedText {
            revision: s.revision,
            text: s.text.clone(),
            source: LabelSource::Corrected,
        });
        assert_eq!(
            EligibleExample::check(&s, 1600, true),
            Err(Error::NotVerified)
        );
    }
}
