//! Domain layer — pure data types and stateless pure functions.
//!
//! This layer has zero I/O side effects and no dependencies on encoding
//! or decoding concerns. Every module here can be tested in isolation.

pub mod composition;
pub mod epoch;
pub mod palette;
pub mod rle;
pub mod segment;
pub mod timing;
