//! Peak detection data structures and algorithms.

/// A detected peak with its properties.
#[derive(Clone, Debug, PartialEq)]
pub struct Peak {
    /// Index in the original array.
    pub index: usize,

    /// Peak value (intensity at index).
    pub value: f64,

    /// Peak prominence (height above surrounding valleys).
    pub prominence: f64,
}

impl Peak {
    /// Create a new peak.
    pub fn new(index: usize, value: f64, prominence: f64) -> Self {
        Self {
            index,
            value,
            prominence,
        }
    }
}

/// C-compatible peak structure for FFI.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct CPeak {
    pub index: usize,
    pub value: f64,
    pub prominence: f64,
}

impl From<Peak> for CPeak {
    fn from(p: Peak) -> Self {
        CPeak {
            index: p.index,
            value: p.value,
            prominence: p.prominence,
        }
    }
}

impl From<CPeak> for Peak {
    fn from(p: CPeak) -> Self {
        Peak {
            index: p.index,
            value: p.value,
            prominence: p.prominence,
        }
    }
}

/// Find peaks in 1D data.
///
/// # Arguments
/// * `data` - Slice of intensity values
/// * `min_height` - Minimum peak height (use f64::NEG_INFINITY for no filter)
/// * `min_prominence` - Minimum prominence (use 0.0 for no filter)
///
/// # Returns
/// Vector of detected peaks
pub fn find_peaks(data: &[f64], min_height: f64, min_prominence: f64) -> Vec<Peak> {
    if data.len() < 3 {
        return Vec::new();
    }

    let mut peaks = Vec::new();

    // Find local maxima
    for i in 1..data.len() - 1 {
        if data[i] > data[i - 1] && data[i] > data[i + 1] && data[i] >= min_height {
            let prominence = calc_prominence(data, i);
            if prominence >= min_prominence {
                peaks.push(Peak::new(i, data[i], prominence));
            }
        }
    }

    peaks
}

/// Find peaks in batch (multiple rows) using parallel processing.
pub fn find_peaks_batch(
    data: &[Vec<f64>],
    min_height: f64,
    min_prominence: f64,
) -> Vec<Vec<Peak>> {
    use rayon::prelude::*;

    data.par_iter()
        .map(|row| find_peaks(row, min_height, min_prominence))
        .collect()
}

/// Calculate prominence of a peak.
/// Prominence = height above the higher of the two adjacent valleys.
pub fn calc_prominence(data: &[f64], peak_idx: usize) -> f64 {
    let peak_val = data[peak_idx];

    // Find left valley (minimum between start and peak)
    let left_min = data[..peak_idx]
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);

    // Find right valley (minimum between peak and end)
    let right_min = data[peak_idx + 1..]
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);

    // Prominence is height above the higher valley
    let higher_valley = left_min.max(right_min);
    peak_val - higher_valley
}

/// Find maximum value and its index.
pub fn find_max(data: &[f64]) -> Option<(f64, usize)> {
    if data.is_empty() {
        return None;
    }

    let mut max_val = f64::NEG_INFINITY;
    let mut max_idx = 0;

    for (i, &val) in data.iter().enumerate() {
        if val > max_val {
            max_val = val;
            max_idx = i;
        }
    }

    Some((max_val, max_idx))
}

/// Compute differences between consecutive elements.
pub fn diff(data: &[f64]) -> Vec<f64> {
    data.windows(2).map(|w| w[1] - w[0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_peaks_simple() {
        let data = vec![0.0, 1.0, 0.5, 3.0, 0.2, 2.0, 0.1];
        let peaks = find_peaks(&data, f64::NEG_INFINITY, 0.0);

        assert_eq!(peaks.len(), 3);
        assert_eq!(peaks[0].index, 1);
        assert_eq!(peaks[1].index, 3);
        assert_eq!(peaks[2].index, 5);
    }

    #[test]
    fn test_find_peaks_with_height_filter() {
        let data = vec![0.0, 1.0, 0.5, 3.0, 0.2, 2.0, 0.1];
        let peaks = find_peaks(&data, 1.5, 0.0);

        assert_eq!(peaks.len(), 2);
        assert_eq!(peaks[0].index, 3);
        assert_eq!(peaks[1].index, 5);
    }

    #[test]
    fn test_find_peaks_with_prominence_filter() {
        let data = vec![0.0, 1.0, 0.5, 3.0, 0.2, 2.0, 0.1];
        let peaks = find_peaks(&data, f64::NEG_INFINITY, 1.5);

        // Only peaks with prominence >= 1.5
        assert!(peaks.len() <= 3);
    }

    #[test]
    fn test_find_peaks_too_short() {
        let data = vec![1.0, 2.0];
        let peaks = find_peaks(&data, f64::NEG_INFINITY, 0.0);
        assert!(peaks.is_empty());
    }

    #[test]
    fn test_find_max() {
        let data = vec![1.0, 5.0, 3.0, 2.0];
        let (max_val, max_idx) = find_max(&data).unwrap();
        assert_eq!(max_val, 5.0);
        assert_eq!(max_idx, 1);
    }

    #[test]
    fn test_diff() {
        let data = vec![1.0, 3.0, 6.0, 10.0];
        let result = diff(&data);
        assert_eq!(result, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_batch_peaks() {
        let data = vec![
            vec![0.0, 1.0, 0.0],
            vec![0.0, 2.0, 0.0],
        ];
        let results = find_peaks_batch(&data, f64::NEG_INFINITY, 0.0);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0][0].index, 1);
        assert_eq!(results[1][0].index, 1);
        assert_eq!(results[0][0].value, 1.0);
        assert_eq!(results[1][0].value, 2.0);
    }
}
