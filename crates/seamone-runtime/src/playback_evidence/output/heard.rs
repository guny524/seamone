//! A union can grow only from opaque callback-confirmed playback runs.

use super::renderer::PlayedRun;
use std::ops::Range;

pub(super) struct Heard {
    len: u64,
    runs: Vec<Range<u64>>,
    broken: bool,
}

impl Heard {
    pub(super) fn new(len: u64) -> Self {
        Self {
            len,
            runs: Vec::new(),
            broken: false,
        }
    }

    pub(super) fn add(&mut self, played: PlayedRun) {
        let run = played.range();
        if self.broken || run.start >= run.end || run.end > self.len {
            return;
        }
        self.runs.push(run);
        self.runs.sort_unstable_by_key(|run| run.start);
        let mut merged = 0;
        for next in 0..self.runs.len() {
            if merged > 0 && self.runs[merged - 1].end >= self.runs[next].start {
                self.runs[merged - 1].end = self.runs[merged - 1].end.max(self.runs[next].end);
            } else {
                self.runs[merged] = self.runs[next].clone();
                merged += 1;
            }
        }
        self.runs.truncate(merged);
    }

    pub(super) fn invalidate(&mut self) {
        self.broken = true;
        self.runs.clear();
    }

    pub(super) fn complete(&self) -> bool {
        !self.broken
            && self.len > 0
            && self
                .runs
                .first()
                .is_some_and(|run| run.start == 0 && run.end == self.len)
    }
}
