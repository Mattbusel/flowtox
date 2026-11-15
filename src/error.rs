//! The crate's error type.

use core::fmt;

/// Everything that can go wrong when building an estimator or fitting a model.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A constructor argument is out of range (for example a zero bucket volume).
    InvalidParameter {
        /// Name of the argument.
        name: &'static str,
        /// What the argument must satisfy.
        reason: &'static str,
    },
    /// Not enough observations for the requested computation.
    InsufficientData {
        /// Minimum number of observations needed.
        needed: usize,
        /// Number supplied.
        got: usize,
    },
    /// Two input slices that must have equal length do not.
    LengthMismatch {
        /// Length of the first slice.
        expected: usize,
        /// Length of the offending slice.
        got: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidParameter { name, reason } => {
                write!(f, "invalid parameter `{name}`: {reason}")
            }
            Error::InsufficientData { needed, got } => {
                write!(
                    f,
                    "insufficient data: need at least {needed} observations, got {got}"
                )
            }
            Error::LengthMismatch { expected, got } => {
                write!(
                    f,
                    "length mismatch: expected {expected} elements, got {got}"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

/// `Result` alias used throughout the crate.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn invalid(name: &'static str, reason: &'static str) -> Error {
    Error::InvalidParameter { name, reason }
}
