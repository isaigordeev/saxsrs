//! Async runtime executor for SAXS batch processing.

use super::policy::{AlwaysInsertPolicy, InsertionPolicy};
use super::regroup::RegroupPool;
use super::scheduler::{PriorityScheduler, WorkItem};
use crate::data::{FlowMetadata, Sample};
use crate::ffi::types::SaxsStatus;
use crate::stage::{StageId, StageRegistry};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime as TokioRuntime;

/// Configuration for the runtime.
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    /// Number of worker threads.
    pub worker_count: usize,
    /// Maximum stages per sample (None = unlimited).
    pub max_stages: Option<u32>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            worker_count: num_cpus::get(),
            max_stages: None,
        }
    }
}

/// Main runtime for SAXS batch processing.
pub struct Runtime {
    /// Configuration.
    config: RuntimeConfig,
    /// Stage registry.
    registry: Arc<StageRegistry>,
    /// Samples waiting to be processed.
    pending_samples: Vec<Sample>,
    /// Scheduler for work items.
    scheduler: Mutex<PriorityScheduler>,
    /// Pool for regrouping completed samples.
    regroup_pool: Mutex<RegroupPool>,
    /// Insertion policy.
    insertion_policy: Arc<dyn InsertionPolicy>,
    /// Completed samples (fully processed).
    completed: Mutex<Vec<Sample>>,
    /// Tokio runtime for async execution.
    tokio_runtime: TokioRuntime,
    /// Cancellation flag.
    cancelled: std::sync::atomic::AtomicBool,
}

impl Runtime {
    /// Create a new runtime with default configuration.
    pub fn new(config: RuntimeConfig) -> Self {
        let registry = Arc::new(StageRegistry::new_with_defaults());
        let scheduler = PriorityScheduler::new(registry.clone());

        let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.worker_count)
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        Self {
            config,
            registry,
            pending_samples: Vec::new(),
            scheduler: Mutex::new(scheduler),
            regroup_pool: Mutex::new(RegroupPool::new()),
            insertion_policy: Arc::new(AlwaysInsertPolicy),
            completed: Mutex::new(Vec::new()),
            tokio_runtime,
            cancelled: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Add a sample to be processed.
    pub fn add_sample(&mut self, sample: Sample) {
        self.pending_samples.push(sample);
    }

    /// Add multiple samples.
    pub fn add_samples(&mut self, samples: impl IntoIterator<Item = Sample>) {
        self.pending_samples.extend(samples);
    }

    /// Set checkpoint stages.
    pub fn set_checkpoints(&mut self, stages: &[u32]) {
        self.regroup_pool
            .lock()
            .unwrap()
            .set_checkpoints(stages.iter().copied());
    }

    /// Clear all checkpoints.
    pub fn clear_checkpoints(&mut self) {
        self.regroup_pool.lock().unwrap().clear_checkpoints();
    }

    /// Set the insertion policy.
    pub fn set_insertion_policy(&mut self, policy: Arc<dyn InsertionPolicy>) {
        self.insertion_policy = policy;
    }

    /// Get the number of pending samples.
    pub fn pending_count(&self) -> usize {
        self.pending_samples.len() + self.scheduler.lock().unwrap().len()
    }

    /// Get the number of completed samples.
    pub fn completed_count(&self) -> usize {
        self.completed.lock().unwrap().len()
    }

    /// Run batch processing synchronously (blocking).
    pub fn run_sync(&mut self) {
        self.cancelled
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // Initialize scheduler with all samples
        let sample_count = self.pending_samples.len();
        {
            let mut scheduler = self.scheduler.lock().unwrap();
            let mut pool = self.regroup_pool.lock().unwrap();

            pool.set_expected_count(sample_count);

            for sample in self.pending_samples.drain(..) {
                let metadata = FlowMetadata::from_sample(&sample.id, &sample.metadata);
                // Start with the first stage (e.g., Background or FindPeak depending on config)
                scheduler.enqueue(WorkItem::new(sample, metadata, StageId::FindPeak));
            }
        }

        // Process until done
        loop {
            if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            let result = {
                let mut scheduler = self.scheduler.lock().unwrap();
                if scheduler.is_empty() {
                    None
                } else {
                    scheduler.process_next()
                }
            };

            match result {
                Some(stage_result) => {
                    let policy = self.insertion_policy.clone();
                    let mut scheduler = self.scheduler.lock().unwrap();

                    // Handle stage requests
                    for request in &stage_result.requests {
                        if policy.should_insert(request) {
                            scheduler.enqueue(WorkItem::new(
                                stage_result.sample.clone(),
                                request.metadata.clone(),
                                request.stage_id,
                            ));
                        }
                    }

                    // If no more stages requested, sample is complete
                    if stage_result.requests.is_empty() {
                        let mut completed = self.completed.lock().unwrap();
                        completed.push(stage_result.sample);
                    } else {
                        // Add to regroup pool at current stage
                        let mut pool = self.regroup_pool.lock().unwrap();
                        pool.add(stage_result.sample);
                    }
                }
                None => break,
            }
        }
    }

    /// Run batch processing asynchronously with callbacks.
    pub fn run_async<F, P, S>(&mut self, on_complete: F, on_progress: P, on_sample: S)
    where
        F: FnOnce(SaxsStatus) + Send + 'static,
        P: Fn(u32, usize, usize) + Send + Sync + 'static,
        S: Fn(Sample) + Send + Sync + 'static,
    {
        self.cancelled
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // Move samples to scheduler
        let samples: Vec<Sample> = self.pending_samples.drain(..).collect();
        let sample_count = samples.len();

        // Clone Arc references for the async task
        let registry = self.registry.clone();
        let policy = self.insertion_policy.clone();

        self.tokio_runtime.spawn(async move {
            let scheduler = Arc::new(Mutex::new(PriorityScheduler::new(registry)));
            let completed = Arc::new(Mutex::new(0usize));

            // Initialize scheduler
            {
                let mut sched = scheduler.lock().unwrap();
                for sample in samples {
                    let metadata = FlowMetadata::from_sample(&sample.id, &sample.metadata);
                    sched.enqueue(WorkItem::new(sample, metadata, StageId::FindPeak));
                }
            }

            // Process loop
            loop {
                let result = {
                    let mut sched = scheduler.lock().unwrap();
                    if sched.is_empty() {
                        None
                    } else {
                        sched.process_next()
                    }
                };

                match result {
                    Some(stage_result) => {
                        // Handle stage requests
                        {
                            let mut sched = scheduler.lock().unwrap();
                            for request in &stage_result.requests {
                                if policy.should_insert(request) {
                                    sched.enqueue(WorkItem::new(
                                        stage_result.sample.clone(),
                                        request.metadata.clone(),
                                        request.stage_id,
                                    ));
                                }
                            }
                        }

                        // If complete, invoke callback
                        if stage_result.requests.is_empty() {
                            let mut count = completed.lock().unwrap();
                            *count += 1;
                            let c = *count;
                            drop(count);

                            on_progress(stage_result.sample.stage_num, c, sample_count);
                            on_sample(stage_result.sample);
                        }
                    }
                    None => break,
                }
            }

            on_complete(SaxsStatus::Ok);
        });
    }

    /// Regroup samples that have reached at least min_stage.
    pub fn regroup(&mut self, min_stage: u32, max_count: usize) -> Vec<Sample> {
        let mut pool = self.regroup_pool.lock().unwrap();
        let mut result = pool.regroup(min_stage);

        // Also include completed samples
        {
            let mut completed = self.completed.lock().unwrap();
            let matching: Vec<_> = completed
                .drain(..)
                .filter(|s| s.stage_num >= min_stage)
                .collect();
            result.extend(matching);
        }

        if result.len() > max_count {
            // Put excess back
            let excess: Vec<_> = result.drain(max_count..).collect();
            self.completed.lock().unwrap().extend(excess);
        }

        result
    }

    /// Cancel all pending operations.
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Reset the runtime for reuse.
    pub fn reset(&mut self) {
        self.pending_samples.clear();
        self.scheduler.lock().unwrap().clear();
        self.regroup_pool.lock().unwrap().reset();
        self.completed.lock().unwrap().clear();
        self.insertion_policy.reset();
        self.cancelled
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
