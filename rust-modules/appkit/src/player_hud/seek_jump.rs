//! Requested-target accumulation and optional quiet-period commits for relative seeks.
use super::scrub_clamp_target;

/// Fresh presses this close together belong to one relative-seek gesture. Keyframe rounding
/// must not erase its requested travel; after a pause, the published playhead is authoritative.
pub const BURST_GAP_MS: u32 = 1_500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeekBurst {
    last: Option<(i64, u32)>,
    deferred: bool,
}

impl SeekBurst {
    pub const IDLE: Self = Self { last: None, deferred: false };

    pub fn reset(&mut self) {
        *self = Self::IDLE;
    }

    /// Logical gesture state, including the expiry clock, for the screen's recorder.
    pub fn snapshot(&self) -> Option<(i64, u32)> {
        self.last
    }

    /// Defer the most recent jump until its quiet period expires. No second target or clock
    /// is kept: the same requested position is used by subsequent taps and by the commit.
    pub fn defer(&mut self) {
        self.deferred = self.last.is_some();
    }

    pub fn pending(&self) -> Option<i64> {
        if self.deferred { self.last.map(|(target, _)| target) } else { None }
    }

    /// Take a queued target once, measured from the latest tap; wrapping clocks are supported.
    pub fn take_due(&mut self, now: u32, quiet_ms: u32) -> Option<i64> {
        let (target, at) = self.last?;
        if !self.deferred || now.wrapping_sub(at) < quiet_ms { return None; }
        self.deferred = false;
        Some(target)
    }

    pub fn jump(&mut self, now: u32, position: i64, step: i64, duration: i64) -> i64 {
        let base = self
            .last
            .filter(|&(_, at)| now.wrapping_sub(at) <= BURST_GAP_MS)
            .map_or(position, |(target, _)| target);
        let target = scrub_clamp_target(base.saturating_add(step), duration);
        self.last = Some((target, now));
        self.deferred = false;
        target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deferred_jump_rearms_and_is_taken_once_including_tick_wrap() {
        let mut burst = SeekBurst::IDLE;
        burst.jump(u32::MAX - 300, 40, 30, 1_000_000_000_000);
        burst.defer();
        assert_eq!(burst.take_due(100, 500), None);
        assert_eq!(burst.take_due(199, 500), Some(70));
        assert_eq!(burst.take_due(200, 500), None);
        burst.jump(300, 40, -10, 1_000_000_000_000);
        burst.defer();
        burst.jump(600, 40, 30, 1_000_000_000_000);
        burst.defer();
        assert_eq!(burst.take_due(800, 500), None, "the latest tap owns the deadline");
        assert_eq!(burst.take_due(1_100, 500), Some(90));
        burst.jump(1_200, 40, 30, 1_000_000_000_000);
        burst.defer();
        burst.reset();
        assert_eq!(burst.take_due(2_000, 500), None, "cancellation retires the timer");
    }

    #[test]
    fn seek_jump_burst_reverses_clamps_and_handles_tick_wrap() {
        let mut burst = SeekBurst::IDLE;
        let second = 1_000_000_000;
        assert_eq!(
            burst.jump(u32::MAX - 100, 40 * second, 60 * second, 100 * second),
            97 * second
        );
        assert_eq!(
            burst.jump(100, 40 * second, -60 * second, 100 * second),
            37 * second
        );
        assert_eq!(burst.jump(200, 40 * second, -60 * second, 100 * second), 0);
        burst.reset();
        assert_eq!(
            burst.jump(300, 40 * second, 10 * second, 100 * second),
            50 * second
        );
        assert_eq!(
            burst.jump(1_801, 43 * second, 10 * second, 100 * second),
            53 * second
        );
    }

}
