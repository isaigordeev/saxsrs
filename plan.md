Based on discussion with claude.

This document proposes a Rust runtime for the SAXS (Small-Angle X-ray Scattering) data processing pipeline. The runtime will replace the current Python-based scheduler and stage execution while maintaining a Python API for end users.

- **Performance**: Parallel batch processing of multiple SAXS samples
- **Determinism**: Priority-based scheduling ensures consistent processing order
- **Flexibility**: On-demand and checkpoint-based regrouping of processed samples
- **Compatibility**: Async Python API that integrates with existing workflows


┌─────────────────────────────────────────────────────────────┐
│                    Python Application                        │
│  async def process():                                        │
│      runtime = saxsrs.Runtime(workers=4)                     │
│      results = await runtime.run_batch(samples)              │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ PyO3 + pyo3-asyncio
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Rust Runtime                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  Scheduler  │  │   Stages    │  │   Regroup Pool      │ │
│  │  (Priority) │  │  (Registry) │  │   (Checkpoints)     │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
│                              │                               │
│                   ┌──────────┴──────────┐                   │
│                   │   Tokio Workers     │                   │
│                   │   (parallel exec)   │                   │
│                   └─────────────────────┘                   │
└─────────────────────────────────────────────────────────────┘

#### 3.2.1 Scheduler

**Type:** Priority Queue (BinaryHeap)

**Ordering:** Samples with lower stage numbers are processed first.

**Rationale:** This ensures slower samples "catch up" while faster samples wait in the regroup pool, naturally synchronizing batches without explicit barriers.

```rust
struct WorkItem {
    sample: Sample,
    metadata: FlowMetadata,
    stage_id: StageId,
}

// Ordering: lower stage_num = higher priority
impl Ord for WorkItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other.sample.stage_num.cmp(&self.sample.stage_num)
    }
}


```rust
pub trait Stage: Send + Sync {
    fn id(&self) -> StageId;
    fn process(&self, sample: Sample, metadata: FlowMetadata) -> StageResult;
}

pub enum StageId {
    Background,
    Cut,
    Filter,
    FindPeak,
    ProcessPeak,
    Phase,
}

stage res

**Stage Result:**
```rust
pub struct StageResult {
    pub sample: Sample,
    pub metadata: FlowMetadata,
    pub requests: Vec<StageRequest>,  // Dynamic next stages
}
```

**Dynamic Stage Injection:** Stages can request subsequent stages (e.g., FindPeak requests ProcessPeak, ProcessPeak requests FindPeak again if peaks remain).


#### 3.2.3 Regroup Pool

**Purpose:** Collect samples that have completed processing up to a certain point.

**Two Modes:**

1. **On-Demand Regrouping**
   ```rust
   // Get all samples at stage >= min_stage
   fn regroup(&mut self, min_stage: u32) -> Vec<Sample>
   ```
   Called from Python when the user needs to collect results.

2. **Checkpoint Regrouping**
   ```rust
   // Define stages where all samples must synchronize
   fn set_checkpoints(&mut self, stages: Vec<u32>)

   // Samples automatically held at checkpoint until all arrive
   fn checkpoint_ready(&self, stage: u32, total: usize) -> bool
   ```

**Data Structure:**
```rust
struct RegroupPool {
    pools: HashMap<u32, Vec<Sample>>,  // stage_num -> completed samples
    checkpoints: HashSet<u32>,         // stages requiring full sync
    expected_count: usize,             // total samples in batch
}


_____


#### 3.2.4 Async Executor

**Runtime:** Tokio with configurable worker count

**Execution Model:**
```rust
impl AsyncRuntime {
    pub async fn run_batch(&self, samples: Vec<Sample>) -> Vec<Sample> {
        // 1. Enqueue all samples at stage 0
        // 2. Spawn N worker tasks
        // 3. Workers pull from priority queue, process, re-enqueue
        // 4. Collect completed samples
        // 5. Return when all samples are fully processed
    }
}


----



## 4. Data Structures

### 4.1 Sample

```rust
#[pyclass]
#[derive(Clone)]
pub struct Sample {
    #[pyo3(get, set)]
    pub id: String,

    #[pyo3(get)]
    pub q_values: Vec<f64>,

    #[pyo3(get)]
    pub intensity: Vec<f64>,

    #[pyo3(get)]
    pub intensity_err: Vec<f64>,

    pub stage_num: u32,
    pub metadata: SampleMetadata,
}
```

**Conversion:** Bidirectional conversion with Python's `SAXSSample` via `From` traits.

### 4.2 FlowMetadata

```rust
#[pyclass]
#[derive(Clone, Default)]
pub struct FlowMetadata {
    pub sample_id: String,
    pub processed_peaks: HashMap<usize, f64>,   // peak_idx -> intensity
    pub unprocessed_peaks: HashMap<usize, f64>,
    pub current_peak: Option<usize>,
}
```

### 4.3 Peak (existing)

```rust
#[pyclass]
#[derive(Clone)]
pub struct Peak {
    pub index: usize,
    pub value: f64,
    pub prominence: f64,
}
```

---

