// Rust guideline compliant 2026-06-22

//! Re-export facade over `moodbar-analysis` and `moodbar-decode`.
//!
//! Provides backward-compatible, zero-cost access to the core DSP analysis,
//! rendering, and audio decoding pipelines without duplicating types.

pub use moodbar_analysis::*;

#[cfg(feature = "decode")]
pub use moodbar_decode::*;
