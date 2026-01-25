//! FindPeak stage implementation.

use super::traits::{Stage, StageId, StageRequest, StageResult};
use crate::data::{find_peaks, FlowMetadata, Sample};

/// Configuration for peak finding.
#[derive(Debug, Clone)]
pub struct FindPeakConfig {
    /// Minimum peak height.
    pub min_height: f64,
    /// Minimum peak prominence.
    pub min_prominence: f64,
    /// Minimum distance between peaks (in indices).
    pub min_distance: usize,
}

impl Default for FindPeakConfig {
    fn default() -> Self {
        Self {
            min_height: 0.5,
            min_prominence: 0.3,
            min_distance: 10,
        }
    }
}

/// Stage for finding peaks in SAXS intensity data.
pub struct FindPeakStage {
    config: FindPeakConfig,
}

impl FindPeakStage {
    /// Create with custom configuration.
    pub fn new(config: FindPeakConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::default()
    }
}

impl Default for FindPeakStage {
    fn default() -> Self {
        Self {
            config: FindPeakConfig::default(),
        }
    }
}

impl Stage for FindPeakStage {
    fn id(&self) -> StageId {
        StageId::FindPeak
    }

    fn process(&self, mut sample: Sample, mut metadata: FlowMetadata) -> StageResult {
        // Find peaks in intensity data
        let peaks = find_peaks(
            sample.intensity_ref(),
            self.config.min_height,
            self.config.min_prominence,
        );

        // Filter by minimum distance if configured
        let filtered_peaks: Vec<_> = if self.config.min_distance > 1 {
            filter_by_distance(peaks, self.config.min_distance)
        } else {
            peaks
        };

        // Add new peaks to unprocessed set
        for peak in &filtered_peaks {
            // Only add if not already processed
            if !metadata.processed_peaks.contains_key(&peak.index) {
                metadata.unprocessed_peaks.insert(peak.index, peak.value);
            }
        }

        // Update sample metadata
        metadata.apply_to_sample(sample.metadata_mut());

        // Determine next stage
        let requests = if metadata.unprocessed_peaks.is_empty() {
            // No peaks to process - terminal
            Vec::new()
        } else {
            // Select highest peak and request ProcessPeak
            let max_entry = metadata
                .unprocessed_peaks
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));

            if let Some((&idx, _)) = max_entry {
                metadata.unprocessed_peaks.remove(&idx);
                metadata.current_peak = Some(idx);

                vec![StageRequest::new(StageId::ProcessPeak, metadata.clone())]
            } else {
                Vec::new()
            }
        };

        sample.advance_stage();
        StageResult::with_requests(sample, metadata, requests)
    }
}

/// Filter peaks to ensure minimum distance between them.
/// Keeps higher peaks when there's a conflict.
fn filter_by_distance(mut peaks: Vec<crate::data::Peak>, min_distance: usize) -> Vec<crate::data::Peak> {
    // Sort by value (highest first)
    peaks.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));

    let mut kept = Vec::new();

    for peak in peaks {
        let too_close = kept.iter().any(|k: &crate::data::Peak| {
            (k.index as isize - peak.index as isize).unsigned_abs() < min_distance
        });

        if !too_close {
            kept.push(peak);
        }
    }

    // Sort back by index
    kept.sort_by_key(|p| p.index);
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample_with_peaks() -> Sample {
        // Create data with clear peaks
        let mut intensity = vec![0.0; 100];
        intensity[20] = 2.0; // Peak 1
        intensity[50] = 3.0; // Peak 2 (highest)
        intensity[80] = 1.5; // Peak 3

        Sample::new(
            "test",
            (0..100).map(|i| i as f64 * 0.01).collect(),
            intensity,
            vec![0.1; 100],
        )
        .unwrap()
    }

    #[test]
    fn test_find_peaks_stage() {
        let stage = FindPeakStage::new(FindPeakConfig {
            min_height: 1.0,
            min_prominence: 0.5,
            min_distance: 1,
        });

        let sample = make_sample_with_peaks();
        let metadata = FlowMetadata::new("test");

        let result = stage.process(sample, metadata);

        // Should find peaks and request ProcessPeak
        assert!(!result.requests.is_empty());
        assert_eq!(result.requests[0].stage_id, StageId::ProcessPeak);

        // Highest peak (index 50) should be selected as current
        assert_eq!(result.requests[0].metadata.current_peak, Some(50));
    }

    #[test]
    fn test_no_peaks_found() {
        let stage = FindPeakStage::new(FindPeakConfig {
            min_height: 10.0, // Too high threshold
            min_prominence: 0.0,
            min_distance: 1,
        });

        let sample = make_sample_with_peaks();
        let metadata = FlowMetadata::new("test");

        let result = stage.process(sample, metadata);

        // No peaks found, terminal result
        assert!(result.requests.is_empty());
    }

    #[test]
    fn test_distance_filtering() {
        let peaks = vec![
            crate::data::Peak::new(10, 2.0, 1.0),
            crate::data::Peak::new(12, 1.5, 1.0), // Too close to peak at 10
            crate::data::Peak::new(25, 3.0, 1.0),
        ];

        let filtered = filter_by_distance(peaks, 5);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|p| p.index == 10));
        assert!(filtered.iter().any(|p| p.index == 25));
    }
}
