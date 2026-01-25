//! SAXS Sample data structure.

use super::metadata::SampleMetadata;

/// A SAXS sample containing measurement data.
#[derive(Clone, Debug)]
pub struct Sample {
    /// Unique identifier for this sample.
    pub id: String,

    /// Scattering vector values (q).
    pub q_values: Vec<f64>,

    /// Measured intensity values.
    pub intensity: Vec<f64>,

    /// Intensity error/uncertainty values.
    pub intensity_err: Vec<f64>,

    /// Current stage number in the pipeline.
    pub stage_num: u32,

    /// Sample-specific metadata.
    pub metadata: SampleMetadata,
}

impl Sample {
    /// Create a new sample.
    pub fn new(
        id: impl Into<String>,
        q_values: Vec<f64>,
        intensity: Vec<f64>,
        intensity_err: Vec<f64>,
    ) -> Result<Self, SampleError> {
        let len = q_values.len();
        if intensity.len() != len || intensity_err.len() != len {
            return Err(SampleError::LengthMismatch {
                q_len: len,
                intensity_len: intensity.len(),
                err_len: intensity_err.len(),
            });
        }

        Ok(Self {
            id: id.into(),
            q_values,
            intensity,
            intensity_err,
            stage_num: 0,
            metadata: SampleMetadata::default(),
        })
    }

    /// Create from raw arrays (for FFI).
    ///
    /// # Safety
    /// Caller must ensure pointers are valid and lengths are correct.
    pub unsafe fn from_raw(
        id: *const std::ffi::c_char,
        q_ptr: *const f64,
        intensity_ptr: *const f64,
        err_ptr: *const f64,
        len: usize,
    ) -> Result<Self, SampleError> {
        use std::ffi::CStr;
        use std::slice;

        if id.is_null() || q_ptr.is_null() || intensity_ptr.is_null() || err_ptr.is_null() {
            return Err(SampleError::NullPointer);
        }

        let id_str = CStr::from_ptr(id)
            .to_str()
            .map_err(|_| SampleError::InvalidUtf8)?
            .to_string();

        let q_values = slice::from_raw_parts(q_ptr, len).to_vec();
        let intensity = slice::from_raw_parts(intensity_ptr, len).to_vec();
        let intensity_err = slice::from_raw_parts(err_ptr, len).to_vec();

        Ok(Self {
            id: id_str,
            q_values,
            intensity,
            intensity_err,
            stage_num: 0,
            metadata: SampleMetadata::default(),
        })
    }

    /// Get the number of data points.
    #[inline]
    pub fn len(&self) -> usize {
        self.q_values.len()
    }

    /// Check if sample has no data points.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.q_values.is_empty()
    }

    /// Get intensity at a specific index.
    #[inline]
    pub fn get_intensity(&self, index: usize) -> Option<f64> {
        self.intensity.get(index).copied()
    }

    /// Set intensity at a specific index.
    #[inline]
    pub fn set_intensity(&mut self, index: usize, value: f64) -> bool {
        if let Some(v) = self.intensity.get_mut(index) {
            *v = value;
            true
        } else {
            false
        }
    }

    /// Get q value at a specific index.
    #[inline]
    pub fn get_q(&self, index: usize) -> Option<f64> {
        self.q_values.get(index).copied()
    }

    /// Get mutable reference to intensity data.
    #[inline]
    pub fn intensity_mut(&mut self) -> &mut Vec<f64> {
        &mut self.intensity
    }

    /// Get reference to intensity data.
    #[inline]
    pub fn intensity_ref(&self) -> &[f64] {
        &self.intensity
    }

    /// Get reference to q values.
    #[inline]
    pub fn q_ref(&self) -> &[f64] {
        &self.q_values
    }

    /// Get mutable reference to metadata.
    #[inline]
    pub fn metadata_mut(&mut self) -> &mut SampleMetadata {
        &mut self.metadata
    }

    /// Increment stage number.
    #[inline]
    pub fn advance_stage(&mut self) {
        self.stage_num += 1;
    }
}

/// Errors that can occur when creating/manipulating samples.
#[derive(Debug, Clone, PartialEq)]
pub enum SampleError {
    /// Array lengths don't match.
    LengthMismatch {
        q_len: usize,
        intensity_len: usize,
        err_len: usize,
    },
    /// Null pointer passed to FFI function.
    NullPointer,
    /// Invalid UTF-8 in string.
    InvalidUtf8,
    /// Index out of bounds.
    IndexOutOfBounds { index: usize, len: usize },
}

impl std::fmt::Display for SampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SampleError::LengthMismatch {
                q_len,
                intensity_len,
                err_len,
            } => write!(
                f,
                "Array length mismatch: q={}, intensity={}, err={}",
                q_len, intensity_len, err_len
            ),
            SampleError::NullPointer => write!(f, "Null pointer passed"),
            SampleError::InvalidUtf8 => write!(f, "Invalid UTF-8 in string"),
            SampleError::IndexOutOfBounds { index, len } => {
                write!(f, "Index {} out of bounds for length {}", index, len)
            }
        }
    }
}

impl std::error::Error for SampleError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_creation() {
        let sample = Sample::new(
            "test",
            vec![1.0, 2.0, 3.0],
            vec![10.0, 20.0, 30.0],
            vec![0.1, 0.2, 0.3],
        )
        .unwrap();

        assert_eq!(sample.id, "test");
        assert_eq!(sample.len(), 3);
        assert_eq!(sample.stage_num, 0);
    }

    #[test]
    fn test_sample_length_mismatch() {
        let result = Sample::new(
            "test",
            vec![1.0, 2.0],
            vec![10.0, 20.0, 30.0],
            vec![0.1, 0.2],
        );

        assert!(matches!(result, Err(SampleError::LengthMismatch { .. })));
    }

    #[test]
    fn test_intensity_access() {
        let mut sample = Sample::new(
            "test",
            vec![1.0, 2.0, 3.0],
            vec![10.0, 20.0, 30.0],
            vec![0.1, 0.2, 0.3],
        )
        .unwrap();

        assert_eq!(sample.get_intensity(1), Some(20.0));
        assert!(sample.set_intensity(1, 25.0));
        assert_eq!(sample.get_intensity(1), Some(25.0));
        assert_eq!(sample.get_intensity(10), None);
    }
}
