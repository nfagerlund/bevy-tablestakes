use bevy::reflect::prelude::*;
use bevy::time::Stopwatch;
use bevy::utils::Duration;

/// CountupTimer is basically a new TimerMode for Bevy's Timer struct. I
/// originally implemented it as such, but carrying a patch on Bevy is gonna be
/// a pain so I'm pulling it out into a duplicate implementation. I'll throw
/// this away if we ever resolve the set_duration/set_elapsed expectations such
/// that I feel safe PRing a new TimerMode.
#[derive(Clone, Debug, Default, Reflect, FromReflect)]
#[cfg_attr(feature = "serialize", derive(serde::Deserialize, serde::Serialize))]
#[reflect(Default)]
pub struct CountupTimer {
    stopwatch: Stopwatch,
    duration: Duration,
    finished: bool,
    times_finished_this_tick: u32,
}

#[allow(unused)]
impl CountupTimer {
    // Initialization

    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            ..Default::default()
        }
    }

    pub fn from_seconds(duration: f32) -> Self {
        Self::new(Duration::from_secs_f32(duration))
    }

    // A bunch of stuff we can just copy straight from Timer
    #[inline]
    pub fn finished(&self) -> bool {
        self.finished
    }
    #[inline]
    pub fn just_finished(&self) -> bool {
        self.times_finished_this_tick > 0
    }
    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.stopwatch.elapsed()
    }
    #[inline]
    pub fn elapsed_secs(&self) -> f32 {
        self.stopwatch.elapsed_secs()
    }
    #[inline]
    pub fn duration(&self) -> Duration {
        self.duration
    }
    #[inline]
    pub fn pause(&mut self) {
        self.stopwatch.pause();
    }
    #[inline]
    pub fn unpause(&mut self) {
        self.stopwatch.unpause();
    }
    #[inline]
    pub fn paused(&self) -> bool {
        self.stopwatch.paused()
    }
    pub fn reset(&mut self) {
        self.stopwatch.reset();
        self.finished = false;
        self.times_finished_this_tick = 0;
    }
    #[inline]
    pub fn times_finished_this_tick(&self) -> u32 {
        self.times_finished_this_tick
    }
    // Stuff that relies on changed impls but isn't itself changed:
    #[inline]
    pub fn percent(&self) -> f32 {
        self.elapsed().as_secs_f32() / self.duration().as_secs_f32()
    }
    #[inline]
    pub fn remaining_secs(&self) -> f32 {
        self.remaining().as_secs_f32()
    }

    // More interesting stuff

    /// Returns the time elapsed since the timer finished.
    pub fn countup_elapsed(&self) -> Duration {
        self.elapsed()
            .checked_sub(self.duration())
            .unwrap_or_default()
    }

    #[inline]
    pub fn countup_elapsed_secs(&self) -> f32 {
        self.countup_elapsed().as_secs_f32()
    }

    // Different from Timer version: requires .max to handle a timer that's done
    // and counting up.
    #[inline]
    pub fn percent_left(&self) -> f32 {
        (1.0 - self.percent()).max(0.0)
    }
    // Different from Timer version: requires .checked_sub so we can bottom out
    // at 0 for a timer that's done and counting up.
    #[inline]
    pub fn remaining(&self) -> Duration {
        self.duration()
            .checked_sub(self.elapsed())
            .unwrap_or_default()
    }

    /// Advance the timer. The heart of dankness. Hey, actually this is
    /// incredibly svelte if you get to ignore all those other TimerModes.
    pub fn tick(&mut self, delta: Duration) -> &Self {
        if self.paused() {
            self.times_finished_this_tick = 0;
            return self;
        }

        let previously_finished = self.finished();
        self.stopwatch.tick(delta);
        self.finished = self.elapsed() >= self.duration();

        if self.finished() && !previously_finished {
            self.times_finished_this_tick = 1;
        } else {
            self.times_finished_this_tick = 0;
        }

        self
    }
}

mod tests {
    #[allow(unused_imports)] // ???!?!?!?!?!!?!?!?!
    use super::*;

    #[test]
    fn countup_timer() {
        let mut t = CountupTimer::from_seconds(2.0);
        // Tick once, check all attributes
        t.tick(Duration::from_secs_f32(0.75));
        assert_eq!(t.elapsed_secs(), 0.75);
        assert_eq!(t.countup_elapsed_secs(), 0.0);
        assert_eq!(t.remaining_secs(), 1.25);
        assert_eq!(t.duration(), Duration::from_secs_f32(2.0));
        assert!(!t.finished());
        assert!(!t.just_finished());
        assert_eq!(t.times_finished_this_tick(), 0);
        assert_eq!(t.percent(), 0.375);
        assert_eq!(t.percent_left(), 0.625);
        // Tick past end; make sure elapsed keeps rolling and countup_elapsed is populated
        t.tick(Duration::from_secs_f32(1.5));
        assert_eq!(t.elapsed_secs(), 2.25);
        assert_eq!(t.countup_elapsed_secs(), 0.25);
        assert_eq!(t.remaining_secs(), 0.0);
        assert!(t.finished());
        assert!(t.just_finished());
        assert_eq!(t.times_finished_this_tick(), 1);
        assert_eq!(t.percent(), 1.125);
        assert_eq!(t.percent_left(), 0.0);
        // Continuing to tick should turn off just_finished but stay finished
        t.tick(Duration::from_secs_f32(0.5));
        assert_eq!(t.elapsed_secs(), 2.75);
        assert_eq!(t.countup_elapsed_secs(), 0.75);
        assert_eq!(t.remaining_secs(), 0.0);
        assert!(t.finished());
        assert!(!t.just_finished());
        assert_eq!(t.times_finished_this_tick(), 0);
        assert_eq!(t.percent(), 1.375);
        assert_eq!(t.percent_left(), 0.0);
        // No weird alternating behavior to just_finished() or anything
        t.tick(Duration::from_secs_f32(2.5));
        assert!(t.finished());
        assert!(!t.just_finished());
        assert_eq!(t.times_finished_this_tick(), 0);
    }

    #[test]
    fn paused_countup() {
        let mut t = CountupTimer::from_seconds(10.0);

        t.tick(Duration::from_secs_f32(10.0));
        assert!(t.just_finished());
        assert!(t.finished());
        // A paused count-up timer should change just_finished to false after a tick
        t.pause();
        t.tick(Duration::from_secs_f32(5.0));
        assert!(!t.just_finished());
        assert!(t.finished());
    }
}
