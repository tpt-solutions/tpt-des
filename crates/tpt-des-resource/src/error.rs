//! Error types for the resource crate.

use core::fmt;

/// Errors returned by resource primitives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceError {
    /// A capacity or amount argument was zero, negative, or non-finite.
    InvalidParameter,
    /// The referenced resource is not registered with the controller.
    UnknownResource,
    /// An entity tried to release more units than it holds.
    ReleaseExceedsHold,
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceError::InvalidParameter => write!(f, "invalid capacity or amount"),
            ResourceError::UnknownResource => write!(f, "resource is not registered"),
            ResourceError::ReleaseExceedsHold => write!(f, "release exceeds the amount held"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ResourceError {}
