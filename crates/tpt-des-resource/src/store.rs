//! Discrete stores and continuous fluid buffers.
//!
//! * [`Store`] holds an integer number of discrete items (parts, messages).
//! * [`Buffer`] holds a continuous fluid level (raw material, budget) and
//!   supports rate-based pour/drain over durations, for hybrid
//!   discrete-continuous models.

use crate::error::ResourceError;

/// A discrete container holding `i64` items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Store {
    level: i64,
    capacity: Option<i64>,
}

impl Store {
    /// An empty unbounded store.
    pub fn unbounded() -> Self {
        Self {
            level: 0,
            capacity: None,
        }
    }

    /// An empty store with a maximum level (must be ≥ 1).
    pub fn with_capacity(capacity: i64) -> Result<Self, ResourceError> {
        if capacity < 1 {
            return Err(ResourceError::InvalidParameter);
        }
        Ok(Self {
            level: 0,
            capacity: Some(capacity),
        })
    }

    /// Current level.
    pub fn level(&self) -> i64 {
        self.level
    }

    /// Maximum level, if bounded.
    pub fn capacity(&self) -> Option<i64> {
        self.capacity
    }

    /// Put `n` items; returns the number accepted (capped at capacity).
    pub fn put(&mut self, n: i64) -> i64 {
        // Room left before the level would exceed capacity (or i64::MAX),
        // so the addition below can never overflow.
        let room = match self.capacity {
            Some(cap) => cap - self.level,
            None => i64::MAX - self.level,
        };
        let accepted = n.clamp(0, room);
        self.level += accepted;
        accepted
    }

    /// Take up to `n` items; returns the number actually taken.
    pub fn get(&mut self, n: i64) -> i64 {
        let taken = n.clamp(0, self.level);
        self.level -= taken;
        taken
    }

    /// Take exactly `n` items, or nothing.
    pub fn try_get(&mut self, n: i64) -> bool {
        if self.level >= n && n >= 0 {
            self.level -= n;
            true
        } else {
            false
        }
    }
}

/// A continuous fluid level in `[0, capacity]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Buffer {
    level: f64,
    capacity: f64,
}

impl Buffer {
    /// An empty buffer with the given capacity (must be finite and > 0).
    pub fn new(capacity: f64) -> Result<Self, ResourceError> {
        if capacity <= 0.0 || !capacity.is_finite() {
            return Err(ResourceError::InvalidParameter);
        }
        Ok(Self {
            level: 0.0,
            capacity,
        })
    }

    /// Current fluid level.
    pub fn level(&self) -> f64 {
        self.level
    }

    /// Maximum level.
    pub fn capacity(&self) -> f64 {
        self.capacity
    }

    /// Fill fraction in `[0, 1]`.
    pub fn fill_fraction(&self) -> f64 {
        self.level / self.capacity
    }

    /// Pour `amount` in; returns the accepted amount (overflow is refused,
    /// not lost silently — the caller sees the shortfall).
    pub fn pour(&mut self, amount: f64) -> f64 {
        // NaN is refused outright rather than poisoning the level.
        if amount.is_nan() {
            return 0.0;
        }
        let accepted = amount.clamp(0.0, self.capacity - self.level);
        self.level += accepted;
        accepted
    }

    /// Drain up to `amount`; returns what was actually taken.
    pub fn drain(&mut self, amount: f64) -> f64 {
        if amount.is_nan() {
            return 0.0;
        }
        let taken = amount.clamp(0.0, self.level);
        self.level -= taken;
        taken
    }

    /// Duration to pour `amount` at `rate` given the free space, or `None`
    /// if the rate is non-positive or the buffer can never absorb it.
    pub fn pour_duration(&self, amount: f64, rate: f64) -> Option<f64> {
        if rate <= 0.0 || rate.is_nan() || amount.is_nan() {
            return None;
        }
        let accepted = amount.min(self.capacity - self.level);
        if accepted <= 0.0 {
            Some(0.0)
        } else {
            Some(accepted / rate)
        }
    }

    /// Duration to drain `amount` at `rate` given the current level.
    pub fn drain_duration(&self, amount: f64, rate: f64) -> Option<f64> {
        if rate <= 0.0 || rate.is_nan() || amount.is_nan() {
            return None;
        }
        let taken = amount.min(self.level);
        if taken <= 0.0 {
            Some(0.0)
        } else {
            Some(taken / rate)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_respects_capacity() {
        let mut s = Store::with_capacity(10).unwrap();
        assert_eq!(s.put(7), 7);
        assert_eq!(s.put(7), 3); // capped
        assert_eq!(s.level(), 10);
        assert_eq!(s.get(4), 4);
        assert!(s.try_get(6));
        assert!(!s.try_get(1));
        assert_eq!(
            Store::with_capacity(0),
            Err(ResourceError::InvalidParameter)
        );
    }

    #[test]
    fn buffer_fluid_accounting() {
        let mut b = Buffer::new(10.0).unwrap();
        assert_eq!(b.pour(4.0), 4.0);
        assert_eq!(b.pour(8.0), 6.0); // overflow refused
        assert!((b.fill_fraction() - 1.0).abs() < 1e-12);
        assert_eq!(b.drain(3.0), 3.0);
        assert_eq!(b.drain(99.0), 7.0); // capped at level
        assert_eq!(b.level(), 0.0);
        assert_eq!(Buffer::new(0.0), Err(ResourceError::InvalidParameter));
    }

    #[test]
    fn buffer_rate_durations() {
        let mut b = Buffer::new(10.0).unwrap();
        assert_eq!(b.pour_duration(5.0, 2.0), Some(2.5));
        assert_eq!(b.pour_duration(5.0, 0.0), None);
        b.pour(4.0);
        assert_eq!(b.drain_duration(8.0, 2.0), Some(2.0)); // only 4 available
        assert_eq!(b.drain_duration(1.0, -1.0), None);
    }
}
