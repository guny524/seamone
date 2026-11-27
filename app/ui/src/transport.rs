use seamone_core::playback::{Action, State};
use std::collections::VecDeque;

#[derive(Clone)]
pub struct Queue {
    commands: VecDeque<Action>,
    running: bool,
    desired: State,
    awaiting: Option<State>,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            commands: VecDeque::new(),
            running: false,
            desired: State::Playing,
            awaiting: None,
        }
    }

    pub fn observe(&mut self, state: State) {
        if self.running {
            return;
        }
        if self.awaiting.is_none_or(|expected| expected == state)
            || matches!(state, State::Finished | State::Failed)
        {
            self.desired = state;
            self.awaiting = None;
        }
    }

    pub fn toggle(&self) -> Action {
        if self.desired == State::Playing {
            Action::Pause
        } else {
            Action::Play
        }
    }

    pub fn enqueue(&mut self, action: Action) -> bool {
        let expected = match action {
            Action::Play => Some(State::Playing),
            Action::Pause | Action::Stop => Some(State::Paused),
            Action::Seek { .. } => None,
        };
        if let Some(expected) = expected {
            self.desired = expected;
            self.awaiting = Some(expected);
        }
        self.commands.push_back(action);
        let start = !self.running;
        self.running = true;
        start
    }

    pub fn next(&mut self) -> Option<Action> {
        let action = self.commands.pop_front();
        if action.is_none() {
            self.running = false;
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_wait_for_the_previous_reply_and_toggles_use_the_last_intent() {
        let mut queue = Queue::new();
        let pause = queue.toggle();
        assert_eq!(pause, Action::Pause);
        assert!(queue.enqueue(pause));
        assert_eq!(queue.next(), Some(Action::Pause));
        queue.observe(State::Playing);
        let play = queue.toggle();
        assert_eq!(play, Action::Play);
        assert!(!queue.enqueue(play));
        assert!(!queue.enqueue(Action::Seek { at: 12_000 }));
        assert!(!queue.enqueue(Action::Stop));
        assert_eq!(queue.next(), Some(Action::Play));
        assert_eq!(queue.next(), Some(Action::Seek { at: 12_000 }));
        assert_eq!(queue.next(), Some(Action::Stop));
        assert_eq!(queue.next(), None);
        queue.observe(State::Playing);
        assert_eq!(queue.toggle(), Action::Play);
        queue.observe(State::Paused);
        assert_eq!(queue.toggle(), Action::Play);
        assert!(queue.enqueue(Action::Play));
    }
}
