//! Regrouping pool for collecting processed samples.

use crate::data::Sample;
use std::collections::{HashMap, HashSet};

/// Pool for collecting samples at various processing stages.
pub struct RegroupPool {
    /// Samples grouped by their current stage number.
    pools: HashMap<u32, Vec<Sample>>,
    /// Stages designated as checkpoints (require all samples to sync).
    checkpoints: HashSet<u32>,
    /// Expected total number of samples in the batch.
    expected_count: usize,
}

impl RegroupPool {
    /// Create a new empty regroup pool.
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            checkpoints: HashSet::new(),
            expected_count: 0,
        }
    }

    /// Create with expected sample count.
    pub fn with_expected_count(expected: usize) -> Self {
        Self {
            pools: HashMap::new(),
            checkpoints: HashSet::new(),
            expected_count: expected,
        }
    }

    /// Set the expected number of samples.
    pub fn set_expected_count(&mut self, count: usize) {
        self.expected_count = count;
    }

    /// Set checkpoint stages.
    pub fn set_checkpoints(&mut self, stages: impl IntoIterator<Item = u32>) {
        self.checkpoints = stages.into_iter().collect();
    }

    /// Add a checkpoint stage.
    pub fn add_checkpoint(&mut self, stage: u32) {
        self.checkpoints.insert(stage);
    }

    /// Clear all checkpoints.
    pub fn clear_checkpoints(&mut self) {
        self.checkpoints.clear();
    }

    /// Check if a stage is a checkpoint.
    pub fn is_checkpoint(&self, stage: u32) -> bool {
        self.checkpoints.contains(&stage)
    }

    /// Add a completed sample to the pool.
    pub fn add(&mut self, sample: Sample) {
        let stage = sample.stage_num;
        self.pools.entry(stage).or_insert_with(Vec::new).push(sample);
    }

    /// Check if a checkpoint is ready (all samples have reached it).
    pub fn checkpoint_ready(&self, stage: u32) -> bool {
        if !self.checkpoints.contains(&stage) {
            return false;
        }

        let count = self.pools.get(&stage).map(|v| v.len()).unwrap_or(0);
        count >= self.expected_count && self.expected_count > 0
    }

    /// Get the number of samples at a specific stage.
    pub fn count_at_stage(&self, stage: u32) -> usize {
        self.pools.get(&stage).map(|v| v.len()).unwrap_or(0)
    }

    /// Get total number of samples in the pool.
    pub fn total_count(&self) -> usize {
        self.pools.values().map(|v| v.len()).sum()
    }

    /// On-demand regroup: collect all samples at or above min_stage.
    ///
    /// Samples are removed from the pool.
    pub fn regroup(&mut self, min_stage: u32) -> Vec<Sample> {
        let mut result = Vec::new();

        let stages_to_drain: Vec<u32> = self
            .pools
            .keys()
            .filter(|&&s| s >= min_stage)
            .copied()
            .collect();

        for stage in stages_to_drain {
            if let Some(samples) = self.pools.remove(&stage) {
                result.extend(samples);
            }
        }

        result
    }

    /// Collect samples at a specific stage.
    ///
    /// Returns None if stage is a checkpoint and not all samples have arrived.
    pub fn collect_at_stage(&mut self, stage: u32) -> Option<Vec<Sample>> {
        if self.is_checkpoint(stage) && !self.checkpoint_ready(stage) {
            return None;
        }

        self.pools.remove(&stage)
    }

    /// Collect all samples from a checkpoint stage (blocking semantics).
    ///
    /// Only succeeds if checkpoint_ready returns true.
    pub fn collect_checkpoint(&mut self, stage: u32) -> Option<Vec<Sample>> {
        if !self.checkpoint_ready(stage) {
            return None;
        }

        self.pools.remove(&stage)
    }

    /// Peek at samples at a stage without removing them.
    pub fn peek_at_stage(&self, stage: u32) -> Option<&[Sample]> {
        self.pools.get(&stage).map(|v| v.as_slice())
    }

    /// Get all stage numbers that have samples.
    pub fn stages_with_samples(&self) -> Vec<u32> {
        let mut stages: Vec<u32> = self.pools.keys().copied().collect();
        stages.sort();
        stages
    }

    /// Clear all samples from the pool.
    pub fn clear(&mut self) {
        self.pools.clear();
    }

    /// Reset the pool completely.
    pub fn reset(&mut self) {
        self.pools.clear();
        self.expected_count = 0;
        // Keep checkpoints as they're configuration
    }
}

impl Default for RegroupPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample(id: &str, stage: u32) -> Sample {
        let mut s = Sample::new(id, vec![1.0], vec![1.0], vec![0.1]).unwrap();
        s.stage_num = stage;
        s
    }

    #[test]
    fn test_add_and_count() {
        let mut pool = RegroupPool::new();

        pool.add(make_sample("a", 3));
        pool.add(make_sample("b", 3));
        pool.add(make_sample("c", 5));

        assert_eq!(pool.count_at_stage(3), 2);
        assert_eq!(pool.count_at_stage(5), 1);
        assert_eq!(pool.total_count(), 3);
    }

    #[test]
    fn test_regroup() {
        let mut pool = RegroupPool::new();

        pool.add(make_sample("a", 3));
        pool.add(make_sample("b", 5));
        pool.add(make_sample("c", 7));

        let regrouped = pool.regroup(5);
        assert_eq!(regrouped.len(), 2); // stage 5 and 7

        assert_eq!(pool.count_at_stage(3), 1); // stage 3 remains
        assert_eq!(pool.total_count(), 1);
    }

    #[test]
    fn test_checkpoint() {
        let mut pool = RegroupPool::with_expected_count(3);
        pool.add_checkpoint(5);

        pool.add(make_sample("a", 5));
        pool.add(make_sample("b", 5));

        // Not ready yet (need 3 samples)
        assert!(!pool.checkpoint_ready(5));
        assert!(pool.collect_checkpoint(5).is_none());

        pool.add(make_sample("c", 5));

        // Now ready
        assert!(pool.checkpoint_ready(5));

        let samples = pool.collect_checkpoint(5).unwrap();
        assert_eq!(samples.len(), 3);
    }
}
