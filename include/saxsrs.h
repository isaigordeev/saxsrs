#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Result status codes for FFI functions.
 */
typedef enum SaxsStatus {
  /**
   * Operation succeeded.
   */
  Ok = 0,
  /**
   * Null pointer was passed.
   */
  NullPointer = 1,
  /**
   * Invalid argument.
   */
  InvalidArgument = 2,
  /**
   * Array length mismatch.
   */
  LengthMismatch = 3,
  /**
   * Invalid UTF-8 string.
   */
  InvalidUtf8 = 4,
  /**
   * Runtime error.
   */
  RuntimeError = 5,
  /**
   * Operation was cancelled.
   */
  Cancelled = 6,
  /**
   * Resource not found.
   */
  NotFound = 7,
} SaxsStatus;

/**
 * Main runtime for SAXS batch processing.
 */
typedef struct Runtime Runtime;

/**
 * A SAXS sample containing measurement data.
 */
typedef struct Sample Sample;

/**
 * Configuration for creating a runtime.
 */
typedef struct CRuntimeConfig {
  /**
   * Number of worker threads (0 = auto-detect).
   */
  uintptr_t worker_count;
  /**
   * Maximum stages per sample (0 = unlimited).
   */
  uint32_t max_stages;
} CRuntimeConfig;

/**
 * Opaque handle to a Runtime.
 */
typedef struct Runtime *RuntimeHandle;

/**
 * Opaque handle to a Sample.
 */
typedef struct Sample *SampleHandle;

/**
 * Callback function type for completion notifications.
 *
 * # Arguments
 * * `user_data` - User-provided context pointer
 * * `status` - Operation status
 * * `result_handle` - Handle to the result (opaque pointer)
 */
typedef void (*CompletionCallback)(void *user_data, enum SaxsStatus status, void *result_handle);

/**
 * Callback for progress updates.
 *
 * # Arguments
 * * `user_data` - User-provided context pointer
 * * `stage` - Current stage number
 * * `completed` - Number of completed items
 * * `total` - Total number of items
 */
typedef void (*ProgressCallback)(void *user_data,
                                 uint32_t stage,
                                 uintptr_t completed,
                                 uintptr_t total);

/**
 * Callback for per-sample completion.
 *
 * # Arguments
 * * `user_data` - User-provided context pointer
 * * `sample_id` - C string with sample ID
 * * `sample_handle` - Handle to completed sample
 */
typedef void (*SampleCallback)(void *user_data, const char *sample_id, void *sample_handle);

/**
 * C-compatible array view (pointer + length).
 */
typedef struct CArrayView {
  const double *data;
  uintptr_t len;
} CArrayView;

/**
 * C-compatible peak result.
 */
typedef struct CPeakResult {
  uintptr_t index;
  double value;
  double prominence;
} CPeakResult;

/**
 * C-compatible peak array result.
 */
typedef struct CPeakArray {
  struct CPeakResult *data;
  uintptr_t len;
  uintptr_t capacity;
} CPeakArray;

/**
 * Create a new runtime.
 *
 * # Safety
 * out_handle must be a valid pointer.
 */
enum SaxsStatus saxs_runtime_create(const struct CRuntimeConfig *config, RuntimeHandle *out_handle);

/**
 * Free a runtime handle.
 *
 * # Safety
 * Handle must be valid or null.
 */
void saxs_runtime_free(RuntimeHandle handle);

/**
 * Add a sample to the runtime batch.
 *
 * # Safety
 * Both handles must be valid. Sample ownership is transferred to runtime.
 */
enum SaxsStatus saxs_runtime_add_sample(RuntimeHandle runtime, SampleHandle sample);

/**
 * Set checkpoint stages.
 *
 * # Safety
 * Runtime handle and stages pointer must be valid.
 */
enum SaxsStatus saxs_runtime_set_checkpoints(RuntimeHandle runtime,
                                             const uint32_t *stages,
                                             uintptr_t stages_len);

/**
 * Run the batch processing asynchronously.
 *
 * This function returns immediately. The completion callback will be
 * invoked when all samples have been processed.
 *
 * # Safety
 * Runtime handle must be valid. Callbacks and user_data must remain valid
 * until the completion callback is invoked.
 */
enum SaxsStatus saxs_runtime_run_async(RuntimeHandle runtime,
                                       CompletionCallback on_complete,
                                       ProgressCallback on_progress,
                                       SampleCallback on_sample,
                                       void *user_data);

/**
 * Run the batch processing synchronously (blocking).
 *
 * # Safety
 * Runtime handle must be valid.
 */
enum SaxsStatus saxs_runtime_run_sync(RuntimeHandle runtime);

/**
 * Get the number of completed samples.
 */
uintptr_t saxs_runtime_completed_count(RuntimeHandle runtime);

/**
 * Get the number of pending samples.
 */
uintptr_t saxs_runtime_pending_count(RuntimeHandle runtime);

/**
 * Collect completed samples at or above a minimum stage.
 *
 * # Safety
 * Runtime handle and output arrays must be valid.
 * out_handles must have capacity for at least `max_count` pointers.
 * Returns the number of samples collected.
 */
enum SaxsStatus saxs_runtime_regroup(RuntimeHandle runtime,
                                     uint32_t min_stage,
                                     SampleHandle *out_handles,
                                     uintptr_t max_count,
                                     uintptr_t *out_count);

/**
 * Cancel all pending operations.
 */
enum SaxsStatus saxs_runtime_cancel(RuntimeHandle runtime);

/**
 * Reset the runtime for reuse.
 */
enum SaxsStatus saxs_runtime_reset(RuntimeHandle runtime);

/**
 * Create a new sample from raw arrays.
 *
 * # Safety
 * All pointers must be valid and arrays must have `len` elements.
 */
enum SaxsStatus saxs_sample_create(const char *id,
                                   const double *q_values,
                                   const double *intensity,
                                   const double *intensity_err,
                                   uintptr_t len,
                                   SampleHandle *out_handle);

/**
 * Free a sample handle.
 *
 * # Safety
 * Handle must be valid or null.
 */
void saxs_sample_free(SampleHandle handle);

/**
 * Get sample ID.
 *
 * # Safety
 * Handle must be valid. Returned pointer is valid until sample is modified or freed.
 */
const char *saxs_sample_get_id(SampleHandle handle);

/**
 * Get sample ID into a buffer.
 *
 * # Safety
 * Handle and buffer must be valid.
 */
enum SaxsStatus saxs_sample_get_id_buf(SampleHandle handle,
                                       char *buffer,
                                       uintptr_t buffer_len,
                                       uintptr_t *out_len);

/**
 * Get sample length (number of data points).
 */
uintptr_t saxs_sample_len(SampleHandle handle);

/**
 * Get sample stage number.
 */
uint32_t saxs_sample_get_stage(SampleHandle handle);

/**
 * Get intensity array view.
 *
 * # Safety
 * Handle must be valid. Returned view is valid until sample is modified or freed.
 */
struct CArrayView saxs_sample_get_intensity(SampleHandle handle);

/**
 * Get q values array view.
 */
struct CArrayView saxs_sample_get_q_values(SampleHandle handle);

/**
 * Get intensity error array view.
 */
struct CArrayView saxs_sample_get_intensity_err(SampleHandle handle);

/**
 * Get number of processed peaks.
 */
uintptr_t saxs_sample_processed_peaks_count(SampleHandle handle);

/**
 * Get number of unprocessed peaks.
 */
uintptr_t saxs_sample_unprocessed_peaks_count(SampleHandle handle);

/**
 * Find peaks in an array.
 *
 * # Safety
 * Data pointer must be valid with `len` elements.
 * Caller must free the result with `saxs_peaks_free`.
 */
enum SaxsStatus saxs_find_peaks(const double *data,
                                uintptr_t len,
                                double min_height,
                                double min_prominence,
                                struct CPeakArray *out_peaks);

/**
 * Free a peak array.
 *
 * # Safety
 * Peaks must have been allocated by saxs_find_peaks or be zeroed.
 */
void saxs_peaks_free(struct CPeakArray *peaks);

/**
 * Find maximum value and index.
 *
 * # Safety
 * Data pointer must be valid with `len` elements.
 */
enum SaxsStatus saxs_find_max(const double *data,
                              uintptr_t len,
                              double *out_value,
                              uintptr_t *out_index);

/**
 * Compute differences between consecutive elements.
 *
 * # Safety
 * Data pointer must be valid. Output buffer must have len-1 elements.
 */
enum SaxsStatus saxs_diff(const double *data, uintptr_t len, double *out, uintptr_t out_len);
