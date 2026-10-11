//! Error type for the statistics crate.

use core::fmt;

/// Why a statistical estimator refused its input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatsError {
    /// The confidence level must lie strictly inside `(0, 1)`.
    InvalidConfidence,
    /// A parameter (quantile level, σ, cycle duration, …) is out of range.
    InvalidParameter,
    /// No observations were supplied.
    EmptySample,
    /// Too few observations for the requested estimate (an interval needs at
    /// least two independent units).
    InsufficientData,
    /// A count is inconsistent (for example more successes than trials).
    InvalidCount,
    /// A value is NaN or infinite.
    NonFinite,
    /// An observation arrived earlier than the last one recorded.
    OutOfOrder,
}

impl fmt::Display for StatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatsError::InvalidConfidence => write!(f, "confidence must lie strictly in (0, 1)"),
            StatsError::InvalidParameter => write!(f, "parameter out of range"),
            StatsError::EmptySample => write!(f, "sample is empty"),
            StatsError::InsufficientData => write!(f, "too few observations for an interval"),
            StatsError::InvalidCount => write!(f, "count is inconsistent with the trials"),
            StatsError::NonFinite => write!(f, "value is NaN or infinite"),
            StatsError::OutOfOrder => write!(f, "observation time precedes the last one"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StatsError {}

/// Shared validation: `confidence` must lie strictly inside `(0, 1)`.
pub(crate) fn check_confidence(confidence: f64) -> Result<(), StatsError> {
    if confidence > 0.0 && confidence < 1.0 {
        Ok(())
    } else {
        Err(StatsError::InvalidConfidence)
    }
}
