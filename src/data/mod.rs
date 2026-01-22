//! Data structures for SAXS processing.

mod metadata;
mod peak;
mod sample;

pub use metadata::{FlowMetadata, SampleMetadata};
pub use peak::{calc_prominence, find_peaks_impl, Peak};
pub use sample::Sample;
