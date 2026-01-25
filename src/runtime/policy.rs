//! Insertion policies for dynamic stage requests.

use crate::stage::StageRequest;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Policy for deciding whether to insert dynamically requested stages.
pub trait InsertionPolicy: Send + Sync {
    /// Decide whether a stage request should be inserted into the queue.
    fn should_insert(&self, request: &StageRequest) -> bool;

    /// Reset the policy state (for reuse).
    fn reset(&self) {}
}

/// Always insert requested stages.
#[derive(Debug, Default)]
pub struct AlwaysInsertPolicy;

impl InsertionPolicy for AlwaysInsertPolicy {
    fn should_insert(&self, _request: &StageRequest) -> bool {
        true
    }
}

/// Never insert requested stages.
#[derive(Debug, Default)]
pub struct NeverInsertPolicy;

impl InsertionPolicy for NeverInsertPolicy {
    fn should_insert(&self, _request: &StageRequest) -> bool {
        false
    }
}

/// Insert up to N stages total.
pub struct SaturationPolicy {
    max_insertions: usize,
    current_count: AtomicUsize,
}

impl SaturationPolicy {
    pub fn new(max_insertions: usize) -> Self {
        Self {
            max_insertions,
            current_count: AtomicUsize::new(0),
        }
    }
}

impl InsertionPolicy for SaturationPolicy {
    fn should_insert(&self, _request: &StageRequest) -> bool {
        let current = self.current_count.fetch_add(1, Ordering::SeqCst);
        if current < self.max_insertions {
            true
        } else {
            // Undo the increment
            self.current_count.fetch_sub(1, Ordering::SeqCst);
            false
        }
    }

    fn reset(&self) {
        self.current_count.store(0, Ordering::SeqCst);
    }
}

/// Insert based on per-sample limits.
pub struct PerSampleLimitPolicy {
    max_per_sample: usize,
    counts: std::sync::RwLock<HashMap<String, usize>>,
}

impl PerSampleLimitPolicy {
    pub fn new(max_per_sample: usize) -> Self {
        Self {
            max_per_sample,
            counts: std::sync::RwLock::new(HashMap::new()),
        }
    }
}

impl InsertionPolicy for PerSampleLimitPolicy {
    fn should_insert(&self, request: &StageRequest) -> bool {
        let sample_id = &request.metadata.sample_id;
        let mut counts = self.counts.write().unwrap();

        let count = counts.entry(sample_id.clone()).or_insert(0);
        if *count < self.max_per_sample {
            *count += 1;
            true
        } else {
            false
        }
    }

    fn reset(&self) {
        self.counts.write().unwrap().clear();
    }
}

/// Composite policy: all sub-policies must approve.
pub struct AllPolicy {
    policies: Vec<Box<dyn InsertionPolicy>>,
}

impl AllPolicy {
    pub fn new(policies: Vec<Box<dyn InsertionPolicy>>) -> Self {
        Self { policies }
    }
}

impl InsertionPolicy for AllPolicy {
    fn should_insert(&self, request: &StageRequest) -> bool {
        self.policies.iter().all(|p| p.should_insert(request))
    }

    fn reset(&self) {
        for policy in &self.policies {
            policy.reset();
        }
    }
}

/// Composite policy: any sub-policy approving is sufficient.
pub struct AnyPolicy {
    policies: Vec<Box<dyn InsertionPolicy>>,
}

impl AnyPolicy {
    pub fn new(policies: Vec<Box<dyn InsertionPolicy>>) -> Self {
        Self { policies }
    }
}

impl InsertionPolicy for AnyPolicy {
    fn should_insert(&self, request: &StageRequest) -> bool {
        self.policies.iter().any(|p| p.should_insert(request))
    }

    fn reset(&self) {
        for policy in &self.policies {
            policy.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::FlowMetadata;
    use crate::stage::StageId;

    fn make_request(sample_id: &str) -> StageRequest {
        StageRequest {
            stage_id: StageId::FindPeak,
            metadata: FlowMetadata::new(sample_id),
        }
    }

    #[test]
    fn test_always_policy() {
        let policy = AlwaysInsertPolicy;
        assert!(policy.should_insert(&make_request("a")));
        assert!(policy.should_insert(&make_request("a")));
    }

    #[test]
    fn test_never_policy() {
        let policy = NeverInsertPolicy;
        assert!(!policy.should_insert(&make_request("a")));
    }

    #[test]
    fn test_saturation_policy() {
        let policy = SaturationPolicy::new(2);
        assert!(policy.should_insert(&make_request("a")));
        assert!(policy.should_insert(&make_request("b")));
        assert!(!policy.should_insert(&make_request("c")));

        policy.reset();
        assert!(policy.should_insert(&make_request("d")));
    }

    #[test]
    fn test_per_sample_limit() {
        let policy = PerSampleLimitPolicy::new(2);

        assert!(policy.should_insert(&make_request("a")));
        assert!(policy.should_insert(&make_request("a")));
        assert!(!policy.should_insert(&make_request("a"))); // 3rd for "a" rejected

        assert!(policy.should_insert(&make_request("b"))); // different sample ok
    }
}
