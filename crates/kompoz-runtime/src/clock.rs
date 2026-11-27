//! The clocks every crate reads (in place of `std::time::Instant` and `SystemTime`, which panic on the web:
//! wasm32-unknown-unknown's std has no clock): a monotonic one ([`Instant`]: deadlines, budgets, ages, how long
//! something took) and the wall clock ([`current_time_millis`], [`unix_time`]: Java's `System.currentTimeMillis`,
//! dates, stamps).
//!
//! Natively they are std's. On the web the platform plugs its own in when it starts (`set_source`: the page's
//! `performance.now()` and `Date.now()`); until it does, the monotonic clock only counts its readings, a millisecond
//! each (so that a deadline or a budget still ends: nothing waits on it), and the wall clock is the Unix epoch plus
//! that count.

use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::time::Duration;

/// A reading of the monotonic clock (as `std::time::Instant`, which it is natively): only its differences mean
/// something, and they never go back.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    /// Since the clock's origin (its first reading natively; the page's time origin on the web).
    since_origin: Duration,
}

impl Instant {
    /// Now.
    pub fn now() -> Instant {
        Instant { since_origin: source::monotonic() }
    }

    /// How long ago it was (zero for one later than now).
    pub fn elapsed(&self) -> Duration {
        Instant::now().saturating_duration_since(*self)
    }

    /// How long after `earlier` it is, zero if it isn't after (as std's, which saturates too).
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.saturating_duration_since(earlier)
    }

    /// How long after `earlier` it is, none if it isn't after.
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        self.since_origin.checked_sub(earlier.since_origin)
    }

    /// How long after `earlier` it is, zero if it isn't after.
    pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.since_origin.saturating_sub(earlier.since_origin)
    }

    /// `duration` later, none if that can't be told.
    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        self.since_origin.checked_add(duration).map(|since_origin| Instant { since_origin })
    }

    /// `duration` earlier, none if that is before the clock's origin.
    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        self.since_origin.checked_sub(duration).map(|since_origin| Instant { since_origin })
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;
    /// Panics if that can't be told (as std's).
    fn add(self, duration: Duration) -> Instant {
        self.checked_add(duration).expect("overflow when adding duration to instant")
    }
}

impl AddAssign<Duration> for Instant {
    fn add_assign(&mut self, duration: Duration) {
        *self = *self + duration;
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;
    /// Panics if that is before the clock's origin.
    fn sub(self, duration: Duration) -> Instant {
        self.checked_sub(duration).expect("overflow when subtracting duration from instant")
    }
}

impl SubAssign<Duration> for Instant {
    fn sub_assign(&mut self, duration: Duration) {
        *self = *self - duration;
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;
    /// How long after `earlier` it is, zero if it isn't after.
    fn sub(self, earlier: Instant) -> Duration {
        self.saturating_duration_since(earlier)
    }
}

impl fmt::Debug for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Instant({:?})", self.since_origin)
    }
}

/// The wall clock: how long since 1970-01-01T00:00Z it is (negative for a clock set before it).
pub fn unix_time() -> UnixTime {
    source::unix()
}

/// The milliseconds since 1970-01-01T00:00Z now (Java's `System.currentTimeMillis`).
pub fn current_time_millis() -> i64 {
    unix_time().as_millis()
}

/// A reading of the wall clock: its distance from the Unix epoch, before or after it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct UnixTime {
    nanos: i128,
}

impl UnixTime {
    /// `nanos` nanoseconds after the epoch (before it if negative).
    pub const fn from_nanos(nanos: i128) -> UnixTime {
        UnixTime { nanos }
    }

    /// Nanoseconds since the epoch (negative before it).
    pub const fn as_nanos(self) -> i128 {
        self.nanos
    }

    /// Whole milliseconds since the epoch, rounded down (as Java's `currentTimeMillis`).
    pub const fn as_millis(self) -> i64 {
        self.nanos.div_euclid(1_000_000) as i64
    }

    /// Whole seconds since the epoch, rounded down.
    pub const fn as_secs(self) -> i64 {
        self.nanos.div_euclid(1_000_000_000) as i64
    }
}

/// Where the web's clocks come from: functions of the page's, each in milliseconds as JavaScript gives them.
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
#[derive(Clone, Copy, Debug)]
pub struct ClockSource {
    /// The monotonic clock (`performance.now()`: since the page's time origin, never going back).
    pub monotonic_millis: fn() -> f64,
    /// The wall clock (`Date.now()`: since the Unix epoch).
    pub unix_millis: fn() -> f64,
}

/// The web's clocks from now on (the web platform's, set when it starts, before anything reads them).
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub fn set_source(clocks: ClockSource) {
    source::set(clocks);
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
mod source {
    use super::UnixTime;
    use std::sync::OnceLock;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub(super) fn monotonic() -> Duration {
        static ORIGIN: OnceLock<std::time::Instant> = OnceLock::new();
        ORIGIN.get_or_init(std::time::Instant::now).elapsed()
    }

    pub(super) fn unix() -> UnixTime {
        UnixTime::from_nanos(match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(after) => after.as_nanos() as i128,
            Err(before) => -(before.duration().as_nanos() as i128),
        })
    }
}

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod source {
    use super::{ClockSource, UnixTime};
    use std::sync::RwLock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    static SOURCE: RwLock<Option<ClockSource>> = RwLock::new(None);
    /// The readings without a source (each one a millisecond after the one before).
    static READINGS: AtomicU64 = AtomicU64::new(0);
    /// The latest monotonic reading: a source's clock never seems to go back (a page's clock can be coarsened).
    static LATEST_NANOS: AtomicU64 = AtomicU64::new(0);

    pub(super) fn set(clocks: ClockSource) {
        *SOURCE.write().unwrap_or_else(|e| e.into_inner()) = Some(clocks);
    }

    fn source() -> Option<ClockSource> {
        *SOURCE.read().unwrap_or_else(|e| e.into_inner())
    }

    fn counted() -> Duration {
        Duration::from_millis(READINGS.fetch_add(1, Ordering::Relaxed) + 1)
    }

    fn nanos_of_millis(ms: f64) -> u64 {
        if ms.is_finite() && ms > 0.0 { (ms * 1e6).min(u64::MAX as f64) as u64 } else { 0 }
    }

    pub(super) fn monotonic() -> Duration {
        match source() {
            Some(clocks) => {
                let now = nanos_of_millis((clocks.monotonic_millis)());
                Duration::from_nanos(LATEST_NANOS.fetch_max(now, Ordering::Relaxed).max(now))
            }
            None => counted(),
        }
    }

    pub(super) fn unix() -> UnixTime {
        match source() {
            Some(clocks) => {
                let ms = (clocks.unix_millis)();
                UnixTime::from_nanos(if ms.is_finite() { (ms * 1e6) as i128 } else { 0 })
            }
            None => UnixTime::from_nanos(counted().as_nanos() as i128),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_monotonic_clock_goes_on() {
        let a = Instant::now();
        let b = Instant::now();
        assert!(b >= a);
        assert_eq!(a - b, Duration::ZERO);
        assert_eq!(a.checked_duration_since(a + Duration::from_millis(1)), None);
        assert_eq!((a + Duration::from_millis(5)).duration_since(a), Duration::from_millis(5));
        assert_eq!((a + Duration::from_millis(5)) - Duration::from_millis(5), a);
        assert!(a.elapsed() <= b.elapsed() + (b - a) + Duration::from_secs(1));
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's clock, which Miri's isolation forbids")]
    fn the_wall_clock_is_after_2020() {
        // (1 January 2020 in milliseconds)
        assert!(current_time_millis() > 1_577_836_800_000);
        assert_eq!(UnixTime::from_nanos(-1).as_millis(), -1);
        assert_eq!(UnixTime::from_nanos(1_999_999).as_millis(), 1);
        assert_eq!(UnixTime::from_nanos(-1).as_secs(), -1);
    }
}
