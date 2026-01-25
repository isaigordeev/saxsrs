//! FFI (Foreign Function Interface) layer for C bindings.
//!
//! This module provides C-compatible functions that can be called from
//! Python via cffi, or from any other language that supports C FFI.

pub mod runtime;
pub mod sample;
pub mod types;

pub use runtime::*;
pub use sample::*;
pub use types::*;
