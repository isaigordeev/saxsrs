//! Priority-based scheduler for SAXS processing.

use crate::data::{FlowMetadata, Sample};
use crate::stage::{Stage, StageId, StageRegistry, StageRequest, StageResult};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;

/// A unit of work in the scheduler queue.
#[derive(Clone)]
pub struct WorkItem {
    /// The sample being processed.
    pub sample: Sample,
    /// Flow metadata for this processing step.
    pub metadata: FlowMetadata,
    /// The stage to execute.
    pub stage_id: StageId,
    /// Priority modifier (higher = more priority).
    pub priority_boost: i32,
}

impl WorkItem {
    pub fn new(sample: Sample, metadata: FlowMetadata, stage_id: StageId) -> Self {
        Self {
            sample,
            metadata,
            stage_id,
            priority_boost: 0,
        }
    }

    pub fn with_priority(mut self, boost: i32) -> Self {
        self.priority_boost = boost;
        self
    }
}

// Implement ordering for priority queue.
// Lower stage_num = higher priority (processed first).
impl Eq for WorkItem {}

impl PartialEq for WorkItem {
    fn eq(&self, other: &Self) -> bool {
        self.sample.stage_num == other.sample.stage_num
            && self.priority_boost == other.priority_boost
    }
}

impl Ord for WorkItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Lower stage number = higher priority
        // Higher priority_boost = higher priority
        match other.sample.stage_num.cmp(&self.sample.stage_num) {
            Ordering::Equal => self.priority_boost.cmp(&other.priority_boost),
            ord => ord,
        }
    }
}

impl PartialOrd for WorkItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Priority-based scheduler using a binary heap.
pub struct PriorityScheduler {
    /// Work items ordered by priority.
    queue: BinaryHeap<WorkItem>,
    /// Stage registry for executing stages.
    registry: Arc<StageRegistry>,
    /// Total items ever enqueued (for stats).
    total_enqueued: usize,
    /// Total items processed.
    total_processed: usize,
}

impl PriorityScheduler {
    /// Create a new scheduler with the given stage registry.
    pub fn new(registry: Arc<StageRegistry>) -> Self {
        Self {
            queue: BinaryHeap::new(),
            registry,
            total_enqueued: 0,
            total_processed: 0,
        }
    }

    /// Enqueue a work item.
    pub fn enqueue(&mut self, item: WorkItem) {
        self.queue.push(item);
        self.total_enqueued += 1;
    }

    /// Enqueue multiple work items.
    pub fn enqueue_all(&mut self, items: impl IntoIterator<Item = WorkItem>) {
        for item in items {
            self.enqueue(item);
        }
    }

    /// Get the next work item without processing it.
    pub fn peek(&self) -> Option<&WorkItem> {
        self.queue.peek()
    }

    /// Pop the next work item.
    pub fn pop(&mut self) -> Option<WorkItem> {
        self.queue.pop()
    }

    /// Process the next work item and return the result.
    ///
    /// Returns `None` if the queue is empty or stage is not found.
    pub fn process_next(&mut self) -> Option<StageResult> {
        let item = self.queue.pop()?;

        let stage = self.registry.get(item.stage_id)?;
        let result = stage.process(item.sample, item.metadata);

        self.total_processed += 1;
        Some(result)
    }

    /// Process next and automatically enqueue stage requests.
    ///
    /// Returns the processed sample and metadata, or None if queue is empty.
    pub fn process_next_auto_enqueue<F>(
        &mut self,
        should_insert: F,
    ) -> Option<(Sample, FlowMetadata)>
    where
        F: Fn(&StageRequest) -> bool,
    {
        let result = self.process_next()?;

        // Enqueue approved stage requests
        for request in result.requests {
            if should_insert(&request) {
                self.enqueue(WorkItem::new(
                    result.sample.clone(),
                    request.metadata,
                    request.stage_id,
                ));
            }
        }

        Some((result.sample, result.metadata))
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get the number of pending work items.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Get total items processed.
    pub fn total_processed(&self) -> usize {
        self.total_processed
    }

    /// Get total items ever enqueued.
    pub fn total_enqueued(&self) -> usize {
        self.total_enqueued
    }

    /// Clear the queue.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Reset statistics.
    pub fn reset_stats(&mut self) {
        self.total_enqueued = 0;
        self.total_processed = 0;
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
    fn test_priority_ordering() {
        let item1 = WorkItem::new(
            make_sample("a", 5),
            FlowMetadata::new("a"),
            StageId::FindPeak,
        );
        let item2 = WorkItem::new(
            make_sample("b", 3),
            FlowMetadata::new("b"),
            StageId::FindPeak,
        );
        let item3 = WorkItem::new(
            make_sample("c", 7),
            FlowMetadata::new("c"),
            StageId::FindPeak,
        );

        // item2 (stage 3) should have highest priority
        assert!(item2 > item1);
        assert!(item2 > item3);
        assert!(item1 > item3);
    }

    #[test]
    fn test_scheduler_ordering() {
        let registry = Arc::new(StageRegistry::new());
        let mut scheduler = PriorityScheduler::new(registry);

        scheduler.enqueue(WorkItem::new(
            make_sample("a", 5),
            FlowMetadata::new("a"),
            StageId::FindPeak,
        ));
        scheduler.enqueue(WorkItem::new(
            make_sample("b", 3),
            FlowMetadata::new("b"),
            StageId::FindPeak,
        ));
        scheduler.enqueue(WorkItem::new(
            make_sample("c", 7),
            FlowMetadata::new("c"),
            StageId::FindPeak,
        ));

        // Should pop in order: b (3), a (5), c (7)
        assert_eq!(scheduler.pop().unwrap().sample.id, "b");
        assert_eq!(scheduler.pop().unwrap().sample.id, "a");
        assert_eq!(scheduler.pop().unwrap().sample.id, "c");
    }
}
