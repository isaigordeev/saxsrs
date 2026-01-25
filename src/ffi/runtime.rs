//! FFI functions for Runtime management.

use super::sample::SampleHandle;
use super::types::{CompletionCallback, ProgressCallback, SampleCallback, SaxsStatus};
use crate::data::Sample;
use crate::runtime::{Runtime, RuntimeConfig};
use std::ffi::c_void;

/// Opaque handle to a Runtime.
pub type RuntimeHandle = *mut Runtime;

/// Configuration for creating a runtime.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CRuntimeConfig {
    /// Number of worker threads (0 = auto-detect).
    pub worker_count: usize,
    /// Maximum stages per sample (0 = unlimited).
    pub max_stages: u32,
}

impl Default for CRuntimeConfig {
    fn default() -> Self {
        Self {
            worker_count: 0,
            max_stages: 0,
        }
    }
}

impl From<CRuntimeConfig> for RuntimeConfig {
    fn from(c: CRuntimeConfig) -> Self {
        RuntimeConfig {
            worker_count: if c.worker_count == 0 {
                num_cpus::get()
            } else {
                c.worker_count
            },
            max_stages: if c.max_stages == 0 {
                None
            } else {
                Some(c.max_stages)
            },
        }
    }
}

/// Create a new runtime.
///
/// # Safety
/// out_handle must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_create(
    config: *const CRuntimeConfig,
    out_handle: *mut RuntimeHandle,
) -> SaxsStatus {
    if out_handle.is_null() {
        return SaxsStatus::NullPointer;
    }

    let cfg = if config.is_null() {
        RuntimeConfig::default()
    } else {
        (*config).clone().into()
    };

    let runtime = Runtime::new(cfg);
    *out_handle = Box::into_raw(Box::new(runtime));

    SaxsStatus::Ok
}

/// Free a runtime handle.
///
/// # Safety
/// Handle must be valid or null.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_free(handle: RuntimeHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Add a sample to the runtime batch.
///
/// # Safety
/// Both handles must be valid. Sample ownership is transferred to runtime.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_add_sample(
    runtime: RuntimeHandle,
    sample: SampleHandle,
) -> SaxsStatus {
    if runtime.is_null() || sample.is_null() {
        return SaxsStatus::NullPointer;
    }

    let rt = &mut *runtime;
    let sample = Box::from_raw(sample);
    rt.add_sample(*sample);

    SaxsStatus::Ok
}

/// Set checkpoint stages.
///
/// # Safety
/// Runtime handle and stages pointer must be valid.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_set_checkpoints(
    runtime: RuntimeHandle,
    stages: *const u32,
    stages_len: usize,
) -> SaxsStatus {
    if runtime.is_null() {
        return SaxsStatus::NullPointer;
    }

    let rt = &mut *runtime;

    if stages.is_null() || stages_len == 0 {
        rt.clear_checkpoints();
    } else {
        let checkpoints = std::slice::from_raw_parts(stages, stages_len);
        rt.set_checkpoints(checkpoints);
    }

    SaxsStatus::Ok
}

/// Run the batch processing asynchronously.
///
/// This function returns immediately. The completion callback will be
/// invoked when all samples have been processed.
///
/// # Safety
/// Runtime handle must be valid. Callbacks and user_data must remain valid
/// until the completion callback is invoked.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_run_async(
    runtime: RuntimeHandle,
    on_complete: CompletionCallback,
    on_progress: ProgressCallback,
    on_sample: SampleCallback,
    user_data: *mut c_void,
) -> SaxsStatus {
    if runtime.is_null() {
        return SaxsStatus::NullPointer;
    }

    let rt = &mut *runtime;

    // Create callback wrappers that are Send + Sync
    let user_data = user_data as usize; // Convert to usize for Send

    let complete_cb = move |status: SaxsStatus| {
        let ud = user_data as *mut c_void;
        on_complete(ud, status, std::ptr::null_mut());
    };

    let progress_cb = move |stage: u32, completed: usize, total: usize| {
        let ud = user_data as *mut c_void;
        on_progress(ud, stage, completed, total);
    };

    let sample_cb = move |sample: Sample| {
        let ud = user_data as *mut c_void;
        let id_cstring = std::ffi::CString::new(sample.id.clone()).unwrap();
        let sample_handle = Box::into_raw(Box::new(sample));
        on_sample(ud, id_cstring.as_ptr(), sample_handle as *mut c_void);
    };

    rt.run_async(complete_cb, progress_cb, sample_cb);

    SaxsStatus::Ok
}

/// Run the batch processing synchronously (blocking).
///
/// # Safety
/// Runtime handle must be valid.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_run_sync(runtime: RuntimeHandle) -> SaxsStatus {
    if runtime.is_null() {
        return SaxsStatus::NullPointer;
    }

    let rt = &mut *runtime;
    rt.run_sync();

    SaxsStatus::Ok
}

/// Get the number of completed samples.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_completed_count(runtime: RuntimeHandle) -> usize {
    if runtime.is_null() {
        return 0;
    }
    (*runtime).completed_count()
}

/// Get the number of pending samples.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_pending_count(runtime: RuntimeHandle) -> usize {
    if runtime.is_null() {
        return 0;
    }
    (*runtime).pending_count()
}

/// Collect completed samples at or above a minimum stage.
///
/// # Safety
/// Runtime handle and output arrays must be valid.
/// out_handles must have capacity for at least `max_count` pointers.
/// Returns the number of samples collected.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_regroup(
    runtime: RuntimeHandle,
    min_stage: u32,
    out_handles: *mut SampleHandle,
    max_count: usize,
    out_count: *mut usize,
) -> SaxsStatus {
    if runtime.is_null() || out_handles.is_null() || out_count.is_null() {
        return SaxsStatus::NullPointer;
    }

    let rt = &mut *runtime;
    let samples = rt.regroup(min_stage, max_count);

    let count = samples.len().min(max_count);
    for (i, sample) in samples.into_iter().take(count).enumerate() {
        *out_handles.add(i) = Box::into_raw(Box::new(sample));
    }

    *out_count = count;
    SaxsStatus::Ok
}

/// Cancel all pending operations.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_cancel(runtime: RuntimeHandle) -> SaxsStatus {
    if runtime.is_null() {
        return SaxsStatus::NullPointer;
    }

    (*runtime).cancel();
    SaxsStatus::Ok
}

/// Reset the runtime for reuse.
#[no_mangle]
pub unsafe extern "C" fn saxs_runtime_reset(runtime: RuntimeHandle) -> SaxsStatus {
    if runtime.is_null() {
        return SaxsStatus::NullPointer;
    }

    (*runtime).reset();
    SaxsStatus::Ok
}
