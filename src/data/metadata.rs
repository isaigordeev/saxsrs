//! Metadata structures for SAXS processing.

use std::collections::HashMap;

/// Sample-level metadata tracking peak processing state.
#[derive(Clone, Debug, Default)]
pub struct SampleMetadata {
    /// Peaks that have been detected but not yet processed.
    /// Key: peak index, Value: peak intensity
    pub unprocessed_peaks: HashMap<usize, f64>,

    /// Peaks that have been fully processed (fitted and subtracted).
    /// Key: peak index, Value: fitted amplitude
    pub processed_peaks: HashMap<usize, f64>,

    /// The current peak being processed (if any).
    pub current_peak: Option<usize>,
}

impl SampleMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add peaks to the unprocessed set.
    pub fn add_unprocessed_peaks(&mut self, peaks: impl IntoIterator<Item = (usize, f64)>) {
        self.unprocessed_peaks.extend(peaks);
    }

    /// Select and remove the highest intensity peak from unprocessed.
    /// Returns the peak index if any peaks remain.
    pub fn select_highest_peak(&mut self) -> Option<usize> {
        let max_entry = self
            .unprocessed_peaks
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((&idx, _)) = max_entry {
            self.unprocessed_peaks.remove(&idx);
            self.current_peak = Some(idx);
            Some(idx)
        } else {
            None
        }
    }

    /// Mark the current peak as processed with given amplitude.
    pub fn mark_current_processed(&mut self, amplitude: f64) {
        if let Some(idx) = self.current_peak.take() {
            self.processed_peaks.insert(idx, amplitude);
        }
    }

    /// Check if there are any unprocessed peaks remaining.
    pub fn has_unprocessed_peaks(&self) -> bool {
        !self.unprocessed_peaks.is_empty()
    }

    /// Clear all peak data (for reprocessing).
    pub fn clear_peaks(&mut self) {
        self.unprocessed_peaks.clear();
        self.processed_peaks.clear();
        self.current_peak = None;
    }
}

/// Flow metadata passed between stages during pipeline execution.
#[derive(Clone, Debug, Default)]
pub struct FlowMetadata {
    /// Sample identifier this metadata belongs to.
    pub sample_id: String,

    /// Processed peaks (index -> amplitude).
    pub processed_peaks: HashMap<usize, f64>,

    /// Unprocessed peaks (index -> intensity).
    pub unprocessed_peaks: HashMap<usize, f64>,

    /// Current peak being processed.
    pub current_peak: Option<usize>,
}

impl FlowMetadata {
    pub fn new(sample_id: impl Into<String>) -> Self {
        Self {
            sample_id: sample_id.into(),
            ..Default::default()
        }
    }

    /// Get number of processed peaks.
    pub fn processed_count(&self) -> usize {
        self.processed_peaks.len()
    }

    /// Get number of unprocessed peaks.
    pub fn unprocessed_count(&self) -> usize {
        self.unprocessed_peaks.len()
    }

    /// Create from sample metadata.
    pub fn from_sample(sample_id: &str, metadata: &SampleMetadata) -> Self {
        Self {
            sample_id: sample_id.to_string(),
            processed_peaks: metadata.processed_peaks.clone(),
            unprocessed_peaks: metadata.unprocessed_peaks.clone(),
            current_peak: metadata.current_peak,
        }
    }

    /// Apply changes back to sample metadata.
    pub fn apply_to_sample(&self, metadata: &mut SampleMetadata) {
        metadata.processed_peaks = self.processed_peaks.clone();
        metadata.unprocessed_peaks = self.unprocessed_peaks.clone();
        metadata.current_peak = self.current_peak;
    }

    /// Select highest unprocessed peak and set as current.
    pub fn select_highest_peak(&mut self) -> Option<usize> {
        let max_entry = self
            .unprocessed_peaks
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((&idx, _)) = max_entry {
            self.unprocessed_peaks.remove(&idx);
            self.current_peak = Some(idx);
            Some(idx)
        } else {
            None
        }
    }

    /// Mark current peak as processed.
    pub fn mark_current_processed(&mut self, amplitude: f64) {
        if let Some(idx) = self.current_peak.take() {
            self.processed_peaks.insert(idx, amplitude);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_highest_peak() {
        let mut metadata = SampleMetadata::new();
        metadata.unprocessed_peaks.insert(5, 1.0);
        metadata.unprocessed_peaks.insert(10, 3.0);
        metadata.unprocessed_peaks.insert(15, 2.0);

        let selected = metadata.select_highest_peak();
        assert_eq!(selected, Some(10));
        assert_eq!(metadata.current_peak, Some(10));
        assert!(!metadata.unprocessed_peaks.contains_key(&10));
    }

    #[test]
    fn test_mark_processed() {
        let mut metadata = SampleMetadata::new();
        metadata.unprocessed_peaks.insert(5, 1.0);

        metadata.select_highest_peak();
        metadata.mark_current_processed(0.95);

        assert!(metadata.current_peak.is_none());
        assert_eq!(metadata.processed_peaks.get(&5), Some(&0.95));
    }

    #[test]
    fn test_flow_metadata_sync() {
        let mut sample_meta = SampleMetadata::new();
        sample_meta.unprocessed_peaks.insert(5, 1.0);

        let mut flow = FlowMetadata::from_sample("test", &sample_meta);
        flow.select_highest_peak();
        flow.mark_current_processed(0.9);

        flow.apply_to_sample(&mut sample_meta);

        assert!(sample_meta.unprocessed_peaks.is_empty());
        assert_eq!(sample_meta.processed_peaks.get(&5), Some(&0.9));
    }
}
