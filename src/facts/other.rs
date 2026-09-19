//! Stub for other platforms.

use super::Facts;

/// Non-macOS platforms have no fast probes yet: leave every key absent.
pub(crate) fn probe(_facts: &mut Facts) {}
