//! ProcessPeak stage implementation.

use super::traits::{Stage, StageId, StageRequest, StageResult};
use crate::data::{FlowMetadata, Sample};

/// Configuration for peak processing.
#[derive(Debug, Clone)]
pub struct ProcessPeakConfig {
    /// Range (in indices) around peak for parabola fitting.
    pub parabola_range: usize,
    /// Multiplier for Gaussian fit range based on sigma.
    pub gaussian_range_multiplier: f64,
}

impl Default for ProcessPeakConfig {
    fn default() -> Self {
        Self {
            parabola_range: 5,
            gaussian_range_multiplier: 3.0,
        }
    }
}

/// Stage for processing (fitting and subtracting) a single peak.
pub struct ProcessPeakStage {
    config: ProcessPeakConfig,
}

impl ProcessPeakStage {
    /// Create with custom configuration.
    pub fn new(config: ProcessPeakConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::default()
    }
}

impl Default for ProcessPeakStage {
    fn default() -> Self {
        Self {
            config: ProcessPeakConfig::default(),
        }
    }
}

impl Stage for ProcessPeakStage {
    fn id(&self) -> StageId {
        StageId::ProcessPeak
    }

    fn process(&self, mut sample: Sample, mut metadata: FlowMetadata) -> StageResult {
        // Get current peak to process
        let peak_idx = match metadata.current_peak {
            Some(idx) => idx,
            None => {
                // No peak to process - should not happen, return terminal
                return StageResult::terminal(sample, metadata);
            }
        };

        if peak_idx >= sample.intensity.len() {
            // Invalid peak index
            metadata.current_peak = None;
            return StageResult::terminal(sample, metadata);
        }

        // Step 1: Fit parabola around peak
        let (mu, sigma, amplitude) = fit_parabola(
            &sample.q_values,
            &sample.intensity,
            peak_idx,
            self.config.parabola_range,
        );

        // Step 2: Fit Gaussian using parabola parameters as initial guess
        let (mu, sigma, amplitude) = fit_gaussian(
            &sample.q_values,
            &sample.intensity,
            peak_idx,
            mu,
            sigma,
            amplitude,
            self.config.gaussian_range_multiplier,
        );

        // Step 3: Subtract Gaussian from intensity
        // Clone q_values to avoid borrow conflict
        let q_values = sample.q_values.clone();
        subtract_gaussian(&mut sample.intensity, &q_values, mu, sigma, amplitude);

        // Mark peak as processed
        metadata.processed_peaks.insert(peak_idx, amplitude);
        metadata.current_peak = None;

        // Update sample metadata
        metadata.apply_to_sample(sample.metadata_mut());

        // Request FindPeak to look for more peaks
        let requests = vec![StageRequest::new(StageId::FindPeak, metadata.clone())];

        sample.advance_stage();
        StageResult::with_requests(sample, metadata, requests)
    }
}

/// Fit a parabola around a peak to estimate Gaussian parameters.
///
/// Returns (mu, sigma, amplitude).
fn fit_parabola(
    q: &[f64],
    intensity: &[f64],
    peak_idx: usize,
    range: usize,
) -> (f64, f64, f64) {
    let start = peak_idx.saturating_sub(range);
    let end = (peak_idx + range + 1).min(intensity.len());

    if end - start < 3 {
        // Not enough points, return simple estimate
        let mu = q.get(peak_idx).copied().unwrap_or(0.0);
        let amplitude = intensity.get(peak_idx).copied().unwrap_or(0.0);
        return (mu, 0.1, amplitude);
    }

    // Extract local data
    let local_q: Vec<f64> = q[start..end].to_vec();
    let local_i: Vec<f64> = intensity[start..end].to_vec();

    // Simple parabola fit: y = a(x - mu)^2 + c
    // Use least squares for a, mu, c

    // For simplicity, use the peak position as mu estimate
    let mu = q[peak_idx];
    let amplitude = intensity[peak_idx];

    // Estimate sigma from curvature
    // d²y/dx² = 2a, and for Gaussian: d²y/dx² = -y/sigma² at peak
    // So sigma ≈ sqrt(-amplitude / (2 * second_derivative))

    let sigma = if local_i.len() >= 3 {
        let mid = local_i.len() / 2;
        let delta_q = (local_q.last().unwrap_or(&1.0) - local_q.first().unwrap_or(&0.0))
            / (local_q.len() - 1) as f64;

        // Second derivative approximation
        let d2 = (local_i.get(mid + 1).copied().unwrap_or(0.0)
            - 2.0 * local_i.get(mid).copied().unwrap_or(0.0)
            + local_i.get(mid.saturating_sub(1)).copied().unwrap_or(0.0))
            / (delta_q * delta_q);

        if d2 < -1e-10 {
            (-amplitude / d2).sqrt().abs()
        } else {
            delta_q * 3.0 // Default estimate
        }
    } else {
        0.1
    };

    (mu, sigma.max(0.01), amplitude)
}

/// Refine Gaussian fit using initial estimates.
///
/// Returns (mu, sigma, amplitude).
fn fit_gaussian(
    q: &[f64],
    intensity: &[f64],
    peak_idx: usize,
    initial_mu: f64,
    initial_sigma: f64,
    initial_amplitude: f64,
    range_multiplier: f64,
) -> (f64, f64, f64) {
    // Determine fitting range based on sigma
    let delta_q = if q.len() > 1 {
        (q.last().unwrap_or(&1.0) - q.first().unwrap_or(&0.0)) / (q.len() - 1) as f64
    } else {
        0.01
    };

    let range_indices = ((initial_sigma * range_multiplier) / delta_q).ceil() as usize;
    let start = peak_idx.saturating_sub(range_indices);
    let end = (peak_idx + range_indices + 1).min(intensity.len());

    if end - start < 3 {
        return (initial_mu, initial_sigma, initial_amplitude);
    }

    // Simple iterative refinement (Gauss-Newton style, simplified)
    let mut mu = initial_mu;
    let mut sigma = initial_sigma;
    let mut amplitude = initial_amplitude;

    for _ in 0..5 {
        // Calculate weighted centroid for mu
        let mut sum_wi = 0.0;
        let mut sum_wiq = 0.0;
        let mut sum_w = 0.0;

        for i in start..end {
            let qi = q[i];
            let yi = intensity[i];
            let weight = yi.max(0.0);

            sum_w += weight;
            sum_wi += weight * yi;
            sum_wiq += weight * qi;
        }

        if sum_w > 1e-10 {
            mu = sum_wiq / sum_w;
        }

        // Recalculate sigma from second moment
        let mut sum_var = 0.0;
        for i in start..end {
            let qi = q[i];
            let yi = intensity[i].max(0.0);
            sum_var += yi * (qi - mu).powi(2);
        }

        if sum_wi > 1e-10 {
            sigma = (sum_var / sum_wi).sqrt().max(0.01);
        }

        // Update amplitude
        amplitude = intensity.get(peak_idx).copied().unwrap_or(initial_amplitude);
    }

    (mu, sigma, amplitude)
}

/// Subtract a Gaussian from intensity data.
fn subtract_gaussian(intensity: &mut [f64], q: &[f64], mu: f64, sigma: f64, amplitude: f64) {
    for (i, qi) in q.iter().enumerate() {
        let gaussian = amplitude * (-(qi - mu).powi(2) / (sigma.powi(2))).exp();
        intensity[i] = (intensity[i] - gaussian).max(0.0);
    }
}

/// Pure Gaussian function.
#[allow(dead_code)]
fn gaussian(x: f64, mu: f64, sigma: f64, amplitude: f64) -> f64 {
    amplitude * (-(x - mu).powi(2) / (sigma.powi(2))).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample_with_peak() -> Sample {
        // Create data with a clear Gaussian peak
        let q: Vec<f64> = (0..100).map(|i| i as f64 * 0.01).collect();
        let intensity: Vec<f64> = q
            .iter()
            .map(|&x| {
                // Gaussian peak at x=0.5, sigma=0.1, amplitude=2.0
                2.0 * (-(x - 0.5).powi(2) / 0.01).exp()
            })
            .collect();

        Sample::new("test", q, intensity, vec![0.1; 100]).unwrap()
    }

    #[test]
    fn test_process_peak_stage() {
        let stage = ProcessPeakStage::default();

        let sample = make_sample_with_peak();
        let mut metadata = FlowMetadata::new("test");
        metadata.current_peak = Some(50); // Peak is at index 50 (q=0.5)

        let result = stage.process(sample, metadata);

        // Should mark peak as processed
        assert!(result.metadata.processed_peaks.contains_key(&50));
        assert!(result.metadata.current_peak.is_none());

        // Should request FindPeak
        assert_eq!(result.requests.len(), 1);
        assert_eq!(result.requests[0].stage_id, StageId::FindPeak);

        // Intensity should be reduced where the peak was
        assert!(result.sample.get_intensity(50).unwrap() < 1.0);
    }

    #[test]
    fn test_gaussian_subtraction() {
        let q: Vec<f64> = (0..50).map(|i| i as f64 * 0.1).collect();
        let mut intensity: Vec<f64> = q.iter().map(|&x| gaussian(x, 2.5, 0.5, 1.0)).collect();

        let original_peak = intensity[25];
        subtract_gaussian(&mut intensity, &q, 2.5, 0.5, 1.0);

        // Peak should be near zero after subtraction
        assert!(intensity[25] < 0.01, "Peak value after subtraction: {}", intensity[25]);
        assert!(intensity[25] < original_peak * 0.1);
    }
}
