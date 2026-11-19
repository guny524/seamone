//! Review navigation is independent of async playback and persistence callbacks.
use crate::review::ReviewSegment;
#[cfg(test)]
use crate::review::Verdict;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewMode {
    Browse,
    Binary,
    Correction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub id: String,
    pub revision: u64,
}

impl From<&ReviewSegment> for Frame {
    fn from(value: &ReviewSegment) -> Self {
        Self {
            id: value.id.clone(),
            revision: value.revision,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub frame: Frame,
    pub generation: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    Ready,
    Loading(Ticket),
    Playing(Ticket),
    Listened,
    Saving(Ticket),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewFlow {
    mode: ReviewMode,
    frames: Vec<Frame>,
    generation: u64,
    sequence: u64,
    phase: Phase,
}

impl Default for ReviewFlow {
    fn default() -> Self {
        Self {
            mode: ReviewMode::Browse,
            frames: Vec::new(),
            generation: 0,
            sequence: 0,
            phase: Phase::Ready,
        }
    }
}

impl ReviewFlow {
    pub fn start(&mut self, mode: ReviewMode, segments: &[ReviewSegment]) {
        self.exit();
        self.mode = mode;
        self.frames = segments
            .iter()
            .filter(|s| match mode {
                ReviewMode::Binary => {
                    s.verdict.is_none() && !s.needs_correction && s.text == s.machine_text
                }
                ReviewMode::Correction => s.needs_correction && s.verified.is_none(),
                ReviewMode::Browse => true,
            })
            .map(Frame::from)
            .collect();
    }

    pub fn skip(&mut self) -> bool {
        if matches!(self.phase, Phase::Saving(_)) || self.frames.is_empty() {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        self.frames.remove(0);
        self.phase = Phase::Ready;
        true
    }

    pub fn exit(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.mode = ReviewMode::Browse;
        self.frames.clear();
        self.phase = Phase::Ready;
    }

    #[must_use]
    pub fn mode(&self) -> ReviewMode {
        self.mode
    }
    #[must_use]
    pub fn frame(&self) -> Option<&Frame> {
        self.frames.first()
    }
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.frames.len()
    }
    #[must_use]
    pub fn busy(&self) -> bool {
        matches!(
            self.phase,
            Phase::Loading(_) | Phase::Playing(_) | Phase::Saving(_)
        )
    }
    #[must_use]
    pub fn listened(&self) -> bool {
        self.phase == Phase::Listened
    }

    fn ticket(&mut self) -> Option<Ticket> {
        self.sequence = self.sequence.wrapping_add(1);
        Some(Ticket {
            frame: self.frame()?.clone(),
            generation: self.generation,
            sequence: self.sequence,
        })
    }

    pub fn play(&mut self) -> Option<Ticket> {
        if self.busy() {
            return None;
        }
        let ticket = self.ticket()?;
        self.phase = Phase::Loading(ticket.clone());
        Some(ticket)
    }

    pub fn loaded(&mut self, ticket: &Ticket) -> bool {
        if self.phase != Phase::Loading(ticket.clone()) {
            return false;
        }
        self.phase = Phase::Playing(ticket.clone());
        true
    }

    pub fn ended(&mut self, ticket: &Ticket) -> bool {
        if self.phase != Phase::Playing(ticket.clone()) {
            return false;
        }
        self.phase = Phase::Listened;
        true
    }

    pub fn playback_failed(&mut self, ticket: &Ticket) -> bool {
        if self.phase != Phase::Loading(ticket.clone())
            && self.phase != Phase::Playing(ticket.clone())
        {
            return false;
        }
        self.phase = Phase::Ready;
        true
    }

    pub fn submit(&mut self, requires_listening: bool) -> Option<Ticket> {
        if self.busy() || (requires_listening && !self.listened()) {
            return None;
        }
        let ticket = self.ticket()?;
        self.phase = Phase::Saving(ticket.clone());
        Some(ticket)
    }

    /// Successful persistence is the only operation that advances the binary pass.
    pub fn saved(&mut self, ticket: &Ticket, saved: &ReviewSegment, advance: bool) -> bool {
        if self.phase != Phase::Saving(ticket.clone())
            || saved.id != ticket.frame.id
            || saved.revision < ticket.frame.revision
        {
            return false;
        }
        if advance {
            self.frames.remove(0);
        } else {
            self.frames[0] = Frame::from(saved);
        }
        self.phase = Phase::Ready;
        true
    }

    pub fn save_failed(&mut self, ticket: &Ticket) -> bool {
        if self.phase != Phase::Saving(ticket.clone()) {
            return false;
        }
        self.phase = Phase::Ready;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::{ReviewAction, ReviewRequest, SampleRange};
    fn segment(id: &str) -> ReviewSegment {
        ReviewSegment {
            id: id.into(),
            recording_id: "r".into(),
            recording_revision: 1,
            revision: 1,
            range: SampleRange {
                start: 0,
                end: 16_000,
            },
            machine_text: "네".into(),
            text: "네".into(),
            verdict: None,
            needs_correction: false,
            verified: None,
        }
    }
    #[test]
    fn only_same_frame_ended_unlocks_verdict_and_successful_write_advances() {
        let s = segment("s");
        let other = segment("other");
        let mut flow = ReviewFlow::default();
        flow.start(ReviewMode::Binary, &[s.clone(), other]);
        assert!(flow.submit(true).is_none());
        let play = flow.play().unwrap();
        assert!(!flow.ended(&play));
        assert!(flow.loaded(&play));
        assert!(flow.submit(true).is_none());
        assert!(flow.ended(&play));
        let write = flow.submit(true).unwrap();
        assert!(flow.submit(true).is_none());
        assert_eq!(flow.frame().unwrap().id, "s");
        assert!(flow.save_failed(&write));
        assert_eq!(flow.frame().unwrap().id, "s");
        let replay = flow.play().unwrap();
        flow.loaded(&replay);
        flow.ended(&replay);
        let write = flow.submit(true).unwrap();
        let saved = s
            .apply(
                &ReviewRequest {
                    event_id: "e".into(),
                    segment_id: "s".into(),
                    expected_revision: 1,
                    action: ReviewAction::Judge {
                        verdict: Verdict::Yes,
                    },
                },
                16_000,
            )
            .unwrap();
        assert!(flow.saved(&write, &saved, true));
        assert_eq!(flow.frame().unwrap().id, "other");
        assert!(!flow.ended(&replay));
        assert!(flow.submit(true).is_none());
    }
    #[test]
    fn exit_and_new_session_discard_late_callbacks() {
        let s = segment("s");
        let mut flow = ReviewFlow::default();
        flow.start(ReviewMode::Binary, std::slice::from_ref(&s));
        let old = flow.play().unwrap();
        flow.loaded(&old);
        flow.exit();
        flow.start(ReviewMode::Binary, &[s]);
        let new = flow.play().unwrap();
        flow.loaded(&new);
        assert!(!flow.ended(&old));
        assert!(flow.ended(&new));
        assert!(!flow.ended(&old));
    }
    #[test]
    fn correction_draft_requires_listening_to_its_new_revision() {
        let mut s = segment("s");
        s.verdict = Some(Verdict::No);
        s.needs_correction = true;
        let mut flow = ReviewFlow::default();
        flow.start(ReviewMode::Correction, std::slice::from_ref(&s));
        let play = flow.play().unwrap();
        flow.loaded(&play);
        flow.ended(&play);
        let write = flow.submit(false).unwrap();
        let saved = s
            .apply(
                &ReviewRequest {
                    event_id: "e".into(),
                    segment_id: "s".into(),
                    expected_revision: 1,
                    action: ReviewAction::SaveDraft { text: "예".into() },
                },
                16_000,
            )
            .unwrap();
        assert!(flow.saved(&write, &saved, false));
        assert!(flow.submit(true).is_none());
        assert!(!flow.ended(&play));
        let play = flow.play().unwrap();
        flow.loaded(&play);
        flow.ended(&play);
        assert!(flow.submit(true).is_some());
    }

    #[test]
    fn failed_or_invalid_playback_can_advance_without_a_verdict() {
        let mut bad = segment("bad");
        bad.range.end = bad.range.start;
        let good = segment("good");
        let mut flow = ReviewFlow::default();
        flow.start(ReviewMode::Binary, &[bad.clone(), good]);
        let failed = flow.play().unwrap();
        flow.playback_failed(&failed);
        assert!(flow.skip());
        assert_eq!(flow.frame().unwrap().id, "good");
        assert!(!flow.ended(&failed));
        assert!(flow.submit(true).is_none());
        assert!(bad.verified.is_none());
        assert!(bad.verdict.is_none());
    }
}
