//! C-compatible type definitions for FFI.

use std::ffi::c_char;

/// Result status codes for FFI functions.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaxsStatus {
    /// Operation succeeded.
    Ok = 0,
    /// Null pointer was passed.
    NullPointer = 1,
    /// Invalid argument.
    InvalidArgument = 2,
    /// Array length mismatch.
    LengthMismatch = 3,
    /// Invalid UTF-8 string.
    InvalidUtf8 = 4,
    /// Runtime error.
    RuntimeError = 5,
    /// Operation was cancelled.
    Cancelled = 6,
    /// Resource not found.
    NotFound = 7,
}

/// C-compatible array view (pointer + length).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CArrayView {
    pub data: *const f64,
    pub len: usize,
}

/// C-compatible mutable array view.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CMutArrayView {
    pub data: *mut f64,
    pub len: usize,
}

/// C-compatible peak result.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CPeakResult {
    pub index: usize,
    pub value: f64,
    pub prominence: f64,
}

/// C-compatible peak array result.
#[repr(C)]
pub struct CPeakArray {
    pub data: *mut CPeakResult,
    pub len: usize,
    pub capacity: usize,
}

impl CPeakArray {
    /// Create from Vec<Peak>.
    pub fn from_peaks(peaks: Vec<crate::data::Peak>) -> Self {
        let mut results: Vec<CPeakResult> = peaks
            .into_iter()
            .map(|p| CPeakResult {
                index: p.index,
                value: p.value,
                prominence: p.prominence,
            })
            .collect();

        let len = results.len();
        let capacity = results.capacity();
        let data = results.as_mut_ptr();
        std::mem::forget(results);

        Self { data, len, capacity }
    }
}

/// Callback function type for completion notifications.
///
/// # Arguments
/// * `user_data` - User-provided context pointer
/// * `status` - Operation status
/// * `result_handle` - Handle to the result (opaque pointer)
pub type CompletionCallback =
    extern "C" fn(user_data: *mut std::ffi::c_void, status: SaxsStatus, result_handle: *mut std::ffi::c_void);

/// Callback for progress updates.
///
/// # Arguments
/// * `user_data` - User-provided context pointer
/// * `stage` - Current stage number
/// * `completed` - Number of completed items
/// * `total` - Total number of items
pub type ProgressCallback =
    extern "C" fn(user_data: *mut std::ffi::c_void, stage: u32, completed: usize, total: usize);

/// Callback for per-sample completion.
///
/// # Arguments
/// * `user_data` - User-provided context pointer
/// * `sample_id` - C string with sample ID
/// * `sample_handle` - Handle to completed sample
pub type SampleCallback =
    extern "C" fn(user_data: *mut std::ffi::c_void, sample_id: *const c_char, sample_handle: *mut std::ffi::c_void);
