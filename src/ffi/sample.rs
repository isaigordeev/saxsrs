//! FFI functions for Sample manipulation.

use super::types::{CArrayView, CPeakArray, SaxsStatus};
use crate::data::{find_peaks, Sample};
use std::ffi::{c_char, CStr};

/// Opaque handle to a Sample.
pub type SampleHandle = *mut Sample;

/// Create a new sample from raw arrays.
///
/// # Safety
/// All pointers must be valid and arrays must have `len` elements.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_create(
    id: *const c_char,
    q_values: *const f64,
    intensity: *const f64,
    intensity_err: *const f64,
    len: usize,
    out_handle: *mut SampleHandle,
) -> SaxsStatus {
    if id.is_null()
        || q_values.is_null()
        || intensity.is_null()
        || intensity_err.is_null()
        || out_handle.is_null()
    {
        return SaxsStatus::NullPointer;
    }

    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return SaxsStatus::InvalidUtf8,
    };

    let q = std::slice::from_raw_parts(q_values, len).to_vec();
    let i = std::slice::from_raw_parts(intensity, len).to_vec();
    let e = std::slice::from_raw_parts(intensity_err, len).to_vec();

    match Sample::new(id_str, q, i, e) {
        Ok(sample) => {
            let boxed = Box::new(sample);
            *out_handle = Box::into_raw(boxed);
            SaxsStatus::Ok
        }
        Err(_) => SaxsStatus::LengthMismatch,
    }
}

/// Free a sample handle.
///
/// # Safety
/// Handle must be valid or null.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_free(handle: SampleHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Get sample ID.
///
/// # Safety
/// Handle must be valid. Returned pointer is valid until sample is modified or freed.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_get_id(handle: SampleHandle) -> *const c_char {
    if handle.is_null() {
        return std::ptr::null();
    }
    let sample = &*handle;
    // Note: This is not ideal - the string needs to be null-terminated
    // For now, return null and use a buffer-based API instead
    std::ptr::null()
}

/// Get sample ID into a buffer.
///
/// # Safety
/// Handle and buffer must be valid.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_get_id_buf(
    handle: SampleHandle,
    buffer: *mut c_char,
    buffer_len: usize,
    out_len: *mut usize,
) -> SaxsStatus {
    if handle.is_null() || buffer.is_null() || out_len.is_null() {
        return SaxsStatus::NullPointer;
    }

    let sample = &*handle;
    let id_bytes = sample.id.as_bytes();
    let copy_len = id_bytes.len().min(buffer_len.saturating_sub(1));

    std::ptr::copy_nonoverlapping(id_bytes.as_ptr(), buffer as *mut u8, copy_len);
    *buffer.add(copy_len) = 0; // Null terminate
    *out_len = sample.id.len();

    SaxsStatus::Ok
}

/// Get sample length (number of data points).
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_len(handle: SampleHandle) -> usize {
    if handle.is_null() {
        return 0;
    }
    (*handle).len()
}

/// Get sample stage number.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_get_stage(handle: SampleHandle) -> u32 {
    if handle.is_null() {
        return 0;
    }
    (*handle).stage_num
}

/// Get intensity array view.
///
/// # Safety
/// Handle must be valid. Returned view is valid until sample is modified or freed.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_get_intensity(handle: SampleHandle) -> CArrayView {
    if handle.is_null() {
        return CArrayView {
            data: std::ptr::null(),
            len: 0,
        };
    }
    let sample = &*handle;
    CArrayView {
        data: sample.intensity.as_ptr(),
        len: sample.intensity.len(),
    }
}

/// Get q values array view.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_get_q_values(handle: SampleHandle) -> CArrayView {
    if handle.is_null() {
        return CArrayView {
            data: std::ptr::null(),
            len: 0,
        };
    }
    let sample = &*handle;
    CArrayView {
        data: sample.q_values.as_ptr(),
        len: sample.q_values.len(),
    }
}

/// Get intensity error array view.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_get_intensity_err(handle: SampleHandle) -> CArrayView {
    if handle.is_null() {
        return CArrayView {
            data: std::ptr::null(),
            len: 0,
        };
    }
    let sample = &*handle;
    CArrayView {
        data: sample.intensity_err.as_ptr(),
        len: sample.intensity_err.len(),
    }
}

/// Get number of processed peaks.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_processed_peaks_count(handle: SampleHandle) -> usize {
    if handle.is_null() {
        return 0;
    }
    (*handle).metadata.processed_peaks.len()
}

/// Get number of unprocessed peaks.
#[no_mangle]
pub unsafe extern "C" fn saxs_sample_unprocessed_peaks_count(handle: SampleHandle) -> usize {
    if handle.is_null() {
        return 0;
    }
    (*handle).metadata.unprocessed_peaks.len()
}

// ============================================================================
// Peak finding functions (stateless)
// ============================================================================

/// Find peaks in an array.
///
/// # Safety
/// Data pointer must be valid with `len` elements.
/// Caller must free the result with `saxs_peaks_free`.
#[no_mangle]
pub unsafe extern "C" fn saxs_find_peaks(
    data: *const f64,
    len: usize,
    min_height: f64,
    min_prominence: f64,
    out_peaks: *mut CPeakArray,
) -> SaxsStatus {
    if data.is_null() || out_peaks.is_null() {
        return SaxsStatus::NullPointer;
    }

    let slice = std::slice::from_raw_parts(data, len);
    let peaks = find_peaks(slice, min_height, min_prominence);
    *out_peaks = CPeakArray::from_peaks(peaks);

    SaxsStatus::Ok
}

/// Free a peak array.
///
/// # Safety
/// Peaks must have been allocated by saxs_find_peaks or be zeroed.
#[no_mangle]
pub unsafe extern "C" fn saxs_peaks_free(peaks: *mut CPeakArray) {
    if peaks.is_null() {
        return;
    }

    let arr = &*peaks;
    if !arr.data.is_null() && arr.capacity > 0 {
        let _ = Vec::from_raw_parts(arr.data, arr.len, arr.capacity);
    }

    (*peaks).data = std::ptr::null_mut();
    (*peaks).len = 0;
    (*peaks).capacity = 0;
}

/// Find maximum value and index.
///
/// # Safety
/// Data pointer must be valid with `len` elements.
#[no_mangle]
pub unsafe extern "C" fn saxs_find_max(
    data: *const f64,
    len: usize,
    out_value: *mut f64,
    out_index: *mut usize,
) -> SaxsStatus {
    if data.is_null() || out_value.is_null() || out_index.is_null() {
        return SaxsStatus::NullPointer;
    }

    if len == 0 {
        return SaxsStatus::InvalidArgument;
    }

    let slice = std::slice::from_raw_parts(data, len);
    if let Some((val, idx)) = crate::data::find_max(slice) {
        *out_value = val;
        *out_index = idx;
        SaxsStatus::Ok
    } else {
        SaxsStatus::InvalidArgument
    }
}

/// Compute differences between consecutive elements.
///
/// # Safety
/// Data pointer must be valid. Output buffer must have len-1 elements.
#[no_mangle]
pub unsafe extern "C" fn saxs_diff(
    data: *const f64,
    len: usize,
    out: *mut f64,
    out_len: usize,
) -> SaxsStatus {
    if data.is_null() || out.is_null() {
        return SaxsStatus::NullPointer;
    }

    if len < 2 || out_len < len - 1 {
        return SaxsStatus::InvalidArgument;
    }

    let slice = std::slice::from_raw_parts(data, len);
    let result = crate::data::diff(slice);

    std::ptr::copy_nonoverlapping(result.as_ptr(), out, result.len());
    SaxsStatus::Ok
}
