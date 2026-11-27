use crate::review::{SAMPLE_RATE, SampleRange};
use alloc::{string::String, vec, vec::Vec};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorMark {
    pub id: String,
    pub recording_id: String,
    pub recording_revision: u64,
    pub range: SampleRange,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorMarkPage {
    pub marks: Vec<ErrorMark>,
    pub next_offset: Option<u32>,
}

#[must_use]
pub fn overlaps(a: SampleRange, b: SampleRange) -> bool {
    a.start < a.end && b.start < b.end && a.start < b.end && b.start < a.end
}

/// Use recent audio from this uninterrupted pass, with context before the current ASR line.
#[must_use]
pub fn candidate(
    heard_from: u64,
    playhead: u64,
    latest_line_start: Option<u64>,
) -> Option<SampleRange> {
    (heard_from < playhead).then(|| SampleRange {
        start: latest_line_start
            .filter(|start| *start < playhead)
            .map_or(heard_from, |start| start.saturating_sub(SAMPLE_RATE))
            .max(heard_from)
            .max(playhead.saturating_sub(6 * SAMPLE_RATE)),
        end: playhead,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    Front,
    Back,
}

/// Both sides must be long enough to listen to (a quarter second each).
pub const MIN_PREVIEW_SAMPLES: u64 = SAMPLE_RATE / 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorMarkBatch {
    pub id: String,
    pub recording_id: String,
    pub recording_revision: u64,
    pub ranges: Vec<SampleRange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedRange {
    pub depth: usize,
    pub path: Vec<SampleRange>,
    pub range: SampleRange,
}

#[derive(Clone, Debug)]
pub struct Narrowing {
    path: Vec<SampleRange>,
    selected: Option<Half>,
    picked_side: bool,
    retained: Vec<RetainedRange>,
}

impl Narrowing {
    #[must_use]
    pub fn new(range: SampleRange) -> Self {
        Self {
            path: vec![range],
            selected: Some(Half::Front),
            picked_side: false,
            retained: Vec::new(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &[SampleRange] {
        &self.path
    }

    #[must_use]
    pub fn retained(&self) -> &[RetainedRange] {
        &self.retained
    }

    #[must_use]
    pub fn half(&self, half: Half) -> Option<SampleRange> {
        let range = *self.path.last()?;
        let length = range.end.saturating_sub(range.start);
        if length < 2 * MIN_PREVIEW_SAMPLES {
            return None;
        }
        let middle = range.start + length / 2;
        Some(match half {
            Half::Front => SampleRange {
                start: range.start,
                end: middle,
            },
            Half::Back => SampleRange {
                start: middle,
                end: range.end,
            },
        })
    }

    #[must_use]
    pub fn selection(&self) -> Option<SampleRange> {
        let half = self.selected?;
        self.half(half).or_else(|| self.path.last().copied())
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.path.len() - usize::from(self.half(Half::Front).is_none())
    }

    #[must_use]
    pub fn confirmed_leaf(&self) -> bool {
        self.picked_side && self.selected.is_some() && self.half(Half::Front).is_none()
    }

    pub fn choose(&mut self, half: Half) {
        if self.half(half).is_some() {
            self.selected = Some(half);
            self.picked_side = true;
        }
    }

    pub fn descend(&mut self) {
        if let Some(range) = self.selected.and_then(|half| self.half(half)) {
            self.path.push(range);
            self.reset_side();
        } else if self.selected.is_some() {
            self.picked_side = true;
        }
    }

    pub fn up(&mut self) {
        if self.path.len() == 1 {
            return;
        }
        let Some(&branch) = self.path.last() else {
            return;
        };
        if self.picked_side
            || !self
                .retained
                .iter()
                .any(|mark| overlaps(mark.range, branch))
        {
            self.retain_selection();
        }
        self.path.pop();
        self.reset_side();
    }

    pub fn cancel_step(&mut self) {
        if self.path.len() == 1 {
            self.selected = None;
            self.picked_side = false;
        } else {
            self.path.pop();
            self.reset_side();
        }
    }

    #[must_use]
    pub fn final_ranges(&self) -> Vec<SampleRange> {
        let mut ranges = self
            .retained
            .iter()
            .map(|mark| mark.range)
            .collect::<Vec<_>>();
        if (self.path.len() > 1 || self.picked_side)
            && let Some(range) = self.unretained_selection()
        {
            ranges.push(range);
        }
        ranges
    }

    fn unretained_selection(&self) -> Option<SampleRange> {
        self.selection()
            .filter(|&range| !self.retained.iter().any(|mark| overlaps(mark.range, range)))
    }

    fn retain_selection(&mut self) {
        if let Some(range) = self.unretained_selection() {
            self.retained.push(RetainedRange {
                depth: self.depth(),
                path: self.path.clone(),
                range,
            });
        }
    }

    fn reset_side(&mut self) {
        self.selected = Some(Half::Front);
        self.picked_side = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_uses_only_recent_audio_actually_played_including_prior_line_context() {
        let second = SAMPLE_RATE;
        assert_eq!(candidate(3 * second, 3 * second, None), None);
        assert_eq!(
            candidate(second, 9 * second, Some(8 * second)),
            Some(SampleRange {
                start: 7 * second,
                end: 9 * second
            })
        );
        assert_eq!(
            candidate(second, 9 * second, Some(2 * second))
                .unwrap()
                .start,
            3 * second
        );
        assert_eq!(
            candidate(8 * second, 9 * second, Some(8 * second))
                .unwrap()
                .start,
            8 * second
        );
    }

    fn units(start: u64, end: u64) -> SampleRange {
        SampleRange {
            start: start * MIN_PREVIEW_SAMPLES,
            end: end * MIN_PREVIEW_SAMPLES,
        }
    }

    #[test]
    fn flags_four_then_zero_and_one_without_widening_when_ascending() {
        let mut search = Narrowing::new(units(0, 8));
        assert_eq!(search.selection(), Some(units(0, 4)));
        assert_eq!(search.final_ranges(), []);
        search.choose(Half::Back);
        assert_eq!(search.path(), &[units(0, 8)]);
        search.descend();
        assert_eq!(search.path(), &[units(0, 8), units(4, 8)]);
        assert_eq!(search.selection(), Some(units(4, 6)));
        search.descend();
        assert_eq!(search.selection(), Some(units(4, 5)));
        search.up();
        search.up();
        assert_eq!(search.selection(), Some(units(0, 4)));
        assert_eq!(
            search.retained(),
            &[RetainedRange {
                depth: 3,
                path: vec![units(0, 8), units(4, 8), units(4, 6)],
                range: units(4, 5)
            }]
        );
        assert_eq!(search.final_ranges(), [units(4, 5)]);
        search.descend();
        assert_eq!(search.final_ranges(), [units(4, 5), units(0, 2)]);
        assert_eq!(search.retained().len(), 1);
    }

    #[test]
    fn cancelling_a_split_preserves_retained_siblings_at_the_same_depth() {
        let mut search = Narrowing::new(units(0, 8));
        search.choose(Half::Back);
        search.descend();
        search.descend();
        search.up();
        search.choose(Half::Back);
        search.descend();
        search.choose(Half::Back);
        search.up();
        search.cancel_step();
        search.cancel_step();
        assert_eq!(search.selection(), None);
        assert_eq!(
            search.retained(),
            &[
                RetainedRange {
                    depth: 3,
                    path: vec![units(0, 8), units(4, 8), units(4, 6)],
                    range: units(4, 5)
                },
                RetainedRange {
                    depth: 3,
                    path: vec![units(0, 8), units(4, 8), units(6, 8)],
                    range: units(7, 8)
                },
            ]
        );
    }

    #[test]
    fn split_stops_at_the_minimum_listenable_length() {
        let range = SampleRange {
            start: 5,
            end: 5 + MIN_PREVIEW_SAMPLES - 1,
        };
        let mut search = Narrowing::new(range);
        assert_eq!(search.half(Half::Front), None);
        assert_eq!(search.final_ranges(), []);
        search.descend();
        assert_eq!(search.path(), &[range]);
        assert_eq!(search.selection(), Some(range));
        assert!(search.confirmed_leaf());
        assert_eq!(search.final_ranges(), [range]);
    }
}
