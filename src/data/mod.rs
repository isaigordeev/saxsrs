//! Data structures for SAXS processing.

pub mod metadata;
pub mod peak;
pub mod sample;

pub use metadata::{FlowMetadata, SampleMetadata};
pub use peak::{calc_prominence, diff, find_max, find_peaks, find_peaks_batch, CPeak, Peak};
pub use sample::{Sample, SampleError};
