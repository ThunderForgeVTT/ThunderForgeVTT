//! Which throw plays when several arrive at once (research R7).
//!
//! One throw plays. Up to four wait, in arrival order. A fifth waiting throw
//! pushes out the oldest waiting one, which is skipped: its roll is in the
//! chat, and the board does not fall behind the table. The playing throw is
//! never skipped. When it lands, the next one starts while the landed one
//! holds and fades on its own.

use std::collections::VecDeque;

/// How many throws may wait behind the playing one.
pub const MAX_WAITING: usize = 4;

#[derive(Debug, Clone)]
pub struct ThrowQueue<T> {
    pub playing: Option<T>,
    pub waiting: VecDeque<T>,
    pub fading: Vec<T>,
}

impl<T> Default for ThrowQueue<T> {
    fn default() -> Self {
        Self {
            playing: None,
            waiting: VecDeque::new(),
            fading: Vec::new(),
        }
    }
}

impl<T> ThrowQueue<T> {
    /// Adds a throw. It plays at once if nothing is playing. Answers the
    /// throw skipped to make room, if any.
    pub fn push(&mut self, throw: T) -> Option<T> {
        if self.playing.is_none() {
            self.playing = Some(throw);
            return None;
        }
        let skipped = if self.waiting.len() >= MAX_WAITING {
            self.waiting.pop_front()
        } else {
            None
        };
        self.waiting.push_back(throw);
        skipped
    }

    /// The playing throw has landed: it moves to the fading throws, and the
    /// next waiting throw starts.
    pub fn landed(&mut self) {
        if let Some(done) = self.playing.take() {
            self.fading.push(done);
        }
        self.playing = self.waiting.pop_front();
    }

    /// Removes and answers the fading throws for which `done` holds.
    pub fn fade_done(&mut self, done: impl Fn(&T) -> bool) -> Vec<T> {
        let (gone, kept): (Vec<T>, Vec<T>) = self.fading.drain(..).partition(|t| done(t));
        self.fading = kept;
        gone
    }

    pub fn is_empty(&self) -> bool {
        self.playing.is_none() && self.waiting.is_empty() && self.fading.is_empty()
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
