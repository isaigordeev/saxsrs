//! Stage trait definitions.

use crate::data::{FlowMetadata, Sample};

/// Identifier for a stage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum StageId {
    /// Remove background signal.
    Background,
    /// Cut/trim data range.
    Cut,
    /// Apply smoothing filter.
    Filter,
    /// Find peaks in the data.
    FindPeak,
    /// Process (fit and subtract) a single peak.
    ProcessPeak,
    /// Phase identification.
    Phase,
}

impl StageId {
    /// Get the string name of this stage.
    pub fn name(&self) -> &'static str {
        match self {
            StageId::Background => "background",
            StageId::Cut => "cut",
            StageId::Filter => "filter",
            StageId::FindPeak => "find_peak",
            StageId::ProcessPeak => "process_peak",
            StageId::Phase => "phase",
        }
    }
}

/// A request to execute a stage.
#[derive(Clone)]
pub struct StageRequest {
    /// The stage to execute.
    pub stage_id: StageId,
    /// Metadata to pass to the stage.
    pub metadata: FlowMetadata,
}

impl StageRequest {
    pub fn new(stage_id: StageId, metadata: FlowMetadata) -> Self {
        Self { stage_id, metadata }
    }
}

/// Result of executing a stage.
pub struct StageResult {
    /// The processed sample.
    pub sample: Sample,
    /// Updated metadata.
    pub metadata: FlowMetadata,
    /// Requests for subsequent stages.
    pub requests: Vec<StageRequest>,
}

impl StageResult {
    /// Create a result with no follow-up stages (terminal).
    pub fn terminal(sample: Sample, metadata: FlowMetadata) -> Self {
        Self {
            sample,
            metadata,
            requests: Vec::new(),
        }
    }

    /// Create a result with follow-up stages.
    pub fn with_requests(
        sample: Sample,
        metadata: FlowMetadata,
        requests: Vec<StageRequest>,
    ) -> Self {
        Self {
            sample,
            metadata,
            requests,
        }
    }
}

/// Trait for processing stages.
pub trait Stage: Send + Sync {
    /// Get the stage identifier.
    fn id(&self) -> StageId;

    /// Process a sample through this stage.
    fn process(&self, sample: Sample, metadata: FlowMetadata) -> StageResult;

    /// Get the stage name.
    fn name(&self) -> &'static str {
        self.id().name()
    }
}
