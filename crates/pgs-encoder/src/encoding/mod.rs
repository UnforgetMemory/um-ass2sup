//! Encoding layer — PGS display set assembly and SUP serialization.
//!
//! Depends on [`crate::domain`] for types and pure functions. These
//! modules orchestrate domain objects into PGS binary segments.

pub mod display_set;
pub mod encoder;
pub mod sup;
