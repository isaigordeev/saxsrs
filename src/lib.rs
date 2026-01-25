//! SAXS Rust Runtime - High-performance SAXS data processing.
//!
//! This crate provides a runtime for batch processing Small-Angle X-ray
//! Scattering (SAXS) data with:
//!
//! - Priority-based scheduling (lower stage numbers processed first)
//! - Parallel batch processing using rayon
//! - Async execution with callback notifications
//! - FFI layer for Python (cffi) and other language bindings
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │     FFI Layer (saxsrs.h)            │
//! │  C-compatible functions & types     │
//! └─────────────────────────────────────┘
//!                   │
//!                   ▼
//! ┌─────────────────────────────────────┐
//! │         Pure Rust Runtime           │
//! │  ┌───────────┐  ┌───────────────┐  │
//! │  │ Scheduler │  │ Stage System  │  │
//! │  │ (Priority)│  │ (Trait-based) │  │
//! │  └───────────┘  └───────────────┘  │
//! │  ┌───────────┐  ┌───────────────┐  │
//! │  │  Regroup  │  │    Tokio      │  │
//! │  │   Pool    │  │   Workers     │  │
//! │  └───────────┘  └───────────────┘  │
//! └─────────────────────────────────────┘
//! ```
//!
//! # FFI Usage
//!
//! The library exposes C-compatible functions for use with Python cffi:
//!
//! ```c
//! // Create runtime
//! SaxsRuntimeHandle runtime;
//! saxs_runtime_create(NULL, &runtime);
//!
//! // Add samples
//! SaxsSampleHandle sample;
//! saxs_sample_create("sample1", q, intensity, err, len, &sample);
//! saxs_runtime_add_sample(runtime, sample);
//!
//! // Run with callbacks
//! saxs_runtime_run_async(runtime, on_complete, on_progress, on_sample, user_data);
//!
//! // Cleanup
//! saxs_runtime_free(runtime);
//! ```

pub mod data;
pub mod ffi;
pub mod runtime;
pub mod stage;

// Re-export commonly used items
pub use data::{FlowMetadata, Peak, Sample, SampleError, SampleMetadata};
pub use runtime::{InsertionPolicy, PriorityScheduler, RegroupPool, Runtime, RuntimeConfig};
pub use stage::{Stage, StageId, StageRegistry, StageRequest, StageResult};

// Re-export FFI types for cbindgen
pub use ffi::types::*;
pub use ffi::runtime::*;
pub use ffi::sample::*;
