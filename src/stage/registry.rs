//! Stage registry for managing available stages.

use super::traits::{Stage, StageId};
use super::{FindPeakStage, ProcessPeakStage};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of available stages.
pub struct StageRegistry {
    stages: HashMap<StageId, Arc<dyn Stage>>,
}

impl StageRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            stages: HashMap::new(),
        }
    }

    /// Create a registry with default stages registered.
    pub fn new_with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(FindPeakStage::default());
        registry.register(ProcessPeakStage::default());
        registry
    }

    /// Register a stage.
    pub fn register<S: Stage + 'static>(&mut self, stage: S) {
        self.stages.insert(stage.id(), Arc::new(stage));
    }

    /// Register a stage from an Arc.
    pub fn register_arc(&mut self, stage: Arc<dyn Stage>) {
        self.stages.insert(stage.id(), stage);
    }

    /// Get a stage by ID.
    pub fn get(&self, id: StageId) -> Option<Arc<dyn Stage>> {
        self.stages.get(&id).cloned()
    }

    /// Check if a stage is registered.
    pub fn contains(&self, id: StageId) -> bool {
        self.stages.contains_key(&id)
    }

    /// Get all registered stage IDs.
    pub fn stage_ids(&self) -> Vec<StageId> {
        self.stages.keys().copied().collect()
    }

    /// Remove a stage.
    pub fn remove(&mut self, id: StageId) -> Option<Arc<dyn Stage>> {
        self.stages.remove(&id)
    }

    /// Clear all stages.
    pub fn clear(&mut self) {
        self.stages.clear();
    }
}

impl Default for StageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_defaults() {
        let registry = StageRegistry::new_with_defaults();

        assert!(registry.contains(StageId::FindPeak));
        assert!(registry.contains(StageId::ProcessPeak));
        assert!(!registry.contains(StageId::Background));
    }

    #[test]
    fn test_registry_get() {
        let registry = StageRegistry::new_with_defaults();

        let stage = registry.get(StageId::FindPeak).unwrap();
        assert_eq!(stage.id(), StageId::FindPeak);
    }
}
