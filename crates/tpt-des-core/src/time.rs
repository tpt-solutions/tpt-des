//! Unit-safe simulation time.
//!
//! [`Time<Unit>`] wraps an `f64` instant and carries its time unit in a
//! phantom type parameter, so ticks, seconds, and hours cannot be mixed:
//!
//! ```
//! use tpt_des_core::{Hours, Seconds, Time};
//!
//! let shift = Time::<Hours>::from_raw(8.0);
//! let shift_secs: Time<Seconds> = shift.to_seconds();
//! assert_eq!(shift_secs.raw(), 28_800.0);
//!
//! let t0 = Time::<Seconds>::from_hours(1.0);
//! assert_eq!(t0.raw(), 3_600.0);
//! ```
//!
//! Ordering is total (`f64::total_cmp`) and construction rejects non-finite
//! values in debug builds; every arithmetic operation preserves finiteness
//! invariants, which keeps the event calendar's ordering well defined.

use core::marker::PhantomData;
use core::ops::{Add, Mul, Sub};

/// Marker type for simulation ticks — the abstract, dimensionless time unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ticks;

/// Marker type for real-world seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seconds;

/// Marker type for real-world hours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hours;

/// Number of seconds in one hour; the only fixed inter-unit factor.
pub const SECONDS_PER_HOUR: f64 = 3600.0;

/// An instant (or duration) of simulation time in unit `Unit`.
///
/// `Time` is deliberately cheap (`Copy`) and ordered totally, so it can key
/// the event calendar. Times may be negative (e.g. relative offsets), but must
/// always be finite.
pub struct Time<Unit> {
    raw: f64,
    _unit: PhantomData<fn() -> Unit>,
}

// Manual impls: the unit marker is a zero-sized tag and must not constrain
// `Clone`/`Copy`/`Default`/`Debug` availability on `Time<Unit>` itself.
impl<Unit> core::fmt::Debug for Time<Unit> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Time({})", self.raw)
    }
}

impl<Unit> Default for Time<Unit> {
    fn default() -> Self {
        Self::ZERO
    }
}

impl<Unit> Clone for Time<Unit> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Unit> Copy for Time<Unit> {}

impl<Unit> Time<Unit> {
    /// The origin (zero).
    pub const ZERO: Self = Self {
        raw: 0.0,
        _unit: PhantomData,
    };

    /// Wrap a raw `f64` value in this time unit.
    ///
    /// Panics in debug builds if `raw` is not finite; in release builds a
    /// non-finite time is carried through but will sort last (see [`Ord`]).
    /// Negative zero is normalized to `+0.0` so `total_cmp` ordering sees a
    /// single zero.
    pub fn from_raw(raw: f64) -> Self {
        debug_assert!(raw.is_finite(), "simulation time must be finite");
        Self {
            raw: if raw == 0.0 { 0.0 } else { raw },
            _unit: PhantomData,
        }
    }

    /// The raw value underlying this time.
    pub fn raw(self) -> f64 {
        self.raw
    }

    /// True if this is the origin.
    pub fn is_zero(self) -> bool {
        self.raw == 0.0
    }

    /// Saturating subtraction: `self - other`, clamped at zero.
    ///
    /// Useful for remaining-time computations where a late event must not
    /// produce negative delays.
    pub fn saturating_sub(self, other: Self) -> Self {
        Self {
            raw: (self.raw - other.raw).max(0.0),
            _unit: PhantomData,
        }
    }
}

impl<Unit> PartialEq for Time<Unit> {
    fn eq(&self, other: &Self) -> bool {
        self.raw.total_cmp(&other.raw) == core::cmp::Ordering::Equal
    }
}

impl<Unit> Eq for Time<Unit> {}

impl<Unit> PartialOrd for Time<Unit> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Unit> Ord for Time<Unit> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.raw.total_cmp(&other.raw)
    }
}

impl<Unit> Add for Time<Unit> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::from_raw(self.raw + rhs.raw)
    }
}

impl<Unit> Sub for Time<Unit> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::from_raw(self.raw - rhs.raw)
    }
}

impl<Unit> Mul<f64> for Time<Unit> {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::from_raw(self.raw * rhs)
    }
}

impl Time<Seconds> {
    /// Convert a duration in hours to seconds.
    pub fn from_hours(hours: f64) -> Self {
        Self::from_raw(hours * SECONDS_PER_HOUR)
    }

    /// Convert a duration in milliseconds to seconds.
    pub fn from_millis(millis: f64) -> Self {
        Self::from_raw(millis / 1_000.0)
    }

    /// Convert a duration in minutes to seconds.
    pub fn from_minutes(minutes: f64) -> Self {
        Self::from_raw(minutes * 60.0)
    }
}

impl Time<Hours> {
    /// Convert this duration to seconds.
    pub fn to_seconds(self) -> Time<Seconds> {
        Time::from_raw(self.raw * SECONDS_PER_HOUR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_conversion() {
        assert_eq!(Time::<Seconds>::from_hours(2.0).raw(), 7200.0);
        assert_eq!(Time::<Hours>::from_raw(1.5).to_seconds().raw(), 5400.0);
        assert_eq!(Time::<Seconds>::from_millis(250.0).raw(), 0.25);
        assert_eq!(Time::<Seconds>::from_minutes(3.0).raw(), 180.0);
    }

    #[test]
    fn total_ordering() {
        let a = Time::<Ticks>::from_raw(1.0);
        let b = Time::<Ticks>::from_raw(2.0);
        assert!(a < b);
        assert_eq!(a.cmp(&a), core::cmp::Ordering::Equal);
        // -0.0 and +0.0 are equal under total_cmp.
        assert_eq!(Time::<Ticks>::from_raw(-0.0), Time::<Ticks>::ZERO);
    }

    #[test]
    fn arithmetic() {
        let t = Time::<Seconds>::from_raw(5.0);
        assert_eq!((t + t).raw(), 10.0);
        assert_eq!((t - Time::<Seconds>::from_raw(2.0)).raw(), 3.0);
        assert_eq!((t * 2.0).raw(), 10.0);
        assert!(t.saturating_sub(Time::<Seconds>::from_raw(9.0)).is_zero());
        assert!(!t.is_zero());
    }
}
