//! Stage system for SAXS processing pipeline.

pub mod find_peak;
pub mod process_peak;
pub mod registry;
pub mod traits;

pub use find_peak::FindPeakStage;
pub use process_peak::ProcessPeakStage;
pub use registry::StageRegistry;
pub use traits::{Stage, StageId, StageRequest, StageResult};
