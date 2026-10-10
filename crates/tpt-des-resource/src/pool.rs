//! Capacity-constrained resource pools.
//!
//! [`ResourcePool`] is the runtime-capacity workhorse; [`FinitePool`] is the
//! `const`-generic variant whose capacity is part of the type, per the
//! spec's "resource capacities are enforced via const generics" principle:
//!
//! ```
//! use tpt_des_resource::pool::FinitePool;
//!
//! let mut pool = FinitePool::<2>::new();
//! assert!(pool.acquire(1));
//! assert!(pool.acquire(1));
//! // The pool is now exhausted: a third unit cannot be granted.
//! assert!(!pool.acquire(1));
//! assert_eq!(pool.release(3), 2); // never over-release
//! ```

use crate::error::ResourceError;

/// A runtime-capacity resource pool (e.g. machines, operators, licenses).
///
/// Units are acquired and released atomically; `acquire` grants only when
/// the full requested amount is available (all-or-nothing, which is what
/// makes deadlock analysis sound).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourcePool {
    capacity: usize,
    available: usize,
}

impl ResourcePool {
    /// Create a pool with `capacity` units (must be ≥ 1).
    pub fn new(capacity: usize) -> Result<Self, ResourceError> {
        if capacity == 0 {
            return Err(ResourceError::InvalidParameter);
        }
        Ok(Self {
            capacity,
            available: capacity,
        })
    }

    /// Total units.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Currently free units.
    pub fn available(&self) -> usize {
        self.available
    }

    /// Long-run utilization bookkeeping aid: held fraction right now.
    pub fn held_fraction(&self) -> f64 {
        self.available as f64 / self.capacity as f64
    }

    /// Try to take `n` units; all-or-nothing.
    pub fn acquire(&mut self, n: usize) -> bool {
        if n > self.available {
            return false;
        }
        self.available -= n;
        true
    }

    /// Return `n` units, capped at the pool's capacity (returns the number
    /// of units actually accepted).
    pub fn release(&mut self, n: usize) -> usize {
        let accepted = n.min(self.capacity - self.available);
        self.available += accepted;
        accepted
    }
}

/// A pool whose capacity is fixed at compile time.
///
/// `CAPACITY` is enforced two ways: construction fails to compile for a
/// zero-capacity pool (const assertion), and `acquire` cannot grant more
/// than `CAPACITY` units because `amount` is capped by the constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinitePool<const CAPACITY: usize> {
    available: usize,
}

impl<const CAPACITY: usize> FinitePool<CAPACITY> {
    /// Create the pool with all `CAPACITY` units free.
    ///
    /// # Panics
    /// At compile time if `CAPACITY == 0` (via the const assertion).
    pub fn new() -> Self {
        const {
            assert!(CAPACITY > 0, "a FinitePool must have positive capacity");
        }
        Self {
            available: CAPACITY,
        }
    }

    /// Total units (the const parameter).
    pub const fn capacity(&self) -> usize {
        CAPACITY
    }

    /// Currently free units.
    pub fn available(&self) -> usize {
        self.available
    }

    /// Try to take `n` units; all-or-nothing.
    pub fn acquire(&mut self, n: usize) -> bool {
        if n > self.available {
            return false;
        }
        self.available -= n;
        true
    }

    /// Return `n` units, capped at capacity; returns units accepted.
    pub fn release(&mut self, n: usize) -> usize {
        let accepted = n.min(CAPACITY - self.available);
        self.available += accepted;
        accepted
    }
}

impl<const CAPACITY: usize> Default for FinitePool<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_pool_all_or_nothing() {
        let mut pool = ResourcePool::new(3).unwrap();
        assert!(pool.acquire(2));
        assert!(!pool.acquire(2));
        assert!(pool.acquire(1));
        assert_eq!(pool.available(), 0);
        assert_eq!(pool.release(2), 2);
        assert_eq!(pool.release(5), 1); // capped at capacity
        assert_eq!(pool.available(), 3);
        assert_eq!(ResourcePool::new(0), Err(ResourceError::InvalidParameter));
    }

    #[test]
    fn const_pool_enforces_capacity() {
        let mut pool = FinitePool::<4>::new();
        assert_eq!(pool.capacity(), 4);
        assert!(pool.acquire(4));
        assert!(!pool.acquire(1));
        assert_eq!(pool.release(9), 4);
    }
}
