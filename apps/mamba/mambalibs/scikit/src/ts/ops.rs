//! Time series operations: ewma, expanding window, lag/lead helpers.
//!
//! Many basic ops (shift, diff, pct_change, rolling) already live in
//! `Series`. This module adds EWMA and expanding-window aggregations.

use frame::frame::Series;
use frame::frame::Value;

// ============================================================================
// EWMA (Exponentially Weighted Moving Average)
// ============================================================================

/// Compute EWMA over a float slice.
///
/// `alpha` is the smoothing factor (0 < alpha <= 1).
/// Higher alpha gives more weight to recent observations.
///
/// Formula: `ewma[0] = x[0]; ewma[t] = alpha * x[t] + (1 - alpha) * ewma[t-1]`
pub fn ewma(data: &[f64], alpha: f64) -> Vec<f64> {
    if data.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(data.len());
    result.push(data[0]);
    for i in 1..data.len() {
        let prev = result[i - 1];
        result.push(alpha * data[i] + (1.0 - alpha) * prev);
    }
    result
}

/// Compute EWMA from a span parameter.
///
/// `span` corresponds to pandas `ewm(span=N)`: `alpha = 2 / (span + 1)`.
pub fn ewma_span(data: &[f64], span: f64) -> Vec<f64> {
    let alpha = 2.0 / (span + 1.0);
    ewma(data, alpha)
}

/// Compute EWMA from a half-life parameter.
///
/// `halflife`: `alpha = 1 - exp(-ln(2) / halflife)`.
pub fn ewma_halflife(data: &[f64], halflife: f64) -> Vec<f64> {
    let alpha = 1.0 - (-f64::ln(2.0) / halflife).exp();
    ewma(data, alpha)
}

// ============================================================================
// Expanding window
// ============================================================================

/// Expanding-window mean: `result[i] = mean(data[0..=i])`.
pub fn expanding_mean(data: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(data.len());
    let mut sum = 0.0;
    for (i, &x) in data.iter().enumerate() {
        sum += x;
        result.push(sum / (i + 1) as f64);
    }
    result
}

/// Expanding-window sum: `result[i] = sum(data[0..=i])`.
pub fn expanding_sum(data: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(data.len());
    let mut sum = 0.0;
    for &x in data {
        sum += x;
        result.push(sum);
    }
    result
}

/// Expanding-window std (population): `result[i] = std(data[0..=i])`.
pub fn expanding_std(data: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(data.len());
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    for (i, &x) in data.iter().enumerate() {
        sum += x;
        sum_sq += x * x;
        let n = (i + 1) as f64;
        if n < 2.0 {
            result.push(f64::NAN);
        } else {
            let mean = sum / n;
            let var = sum_sq / n - mean * mean;
            result.push(var.max(0.0).sqrt());
        }
    }
    result
}

/// Expanding-window min: `result[i] = min(data[0..=i])`.
pub fn expanding_min(data: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(data.len());
    let mut cur_min = f64::INFINITY;
    for &x in data {
        cur_min = cur_min.min(x);
        result.push(cur_min);
    }
    result
}

/// Expanding-window max: `result[i] = max(data[0..=i])`.
pub fn expanding_max(data: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(data.len());
    let mut cur_max = f64::NEG_INFINITY;
    for &x in data {
        cur_max = cur_max.max(x);
        result.push(cur_max);
    }
    result
}

// ============================================================================
// Lag / Lead helpers (thin wrappers over Series)
// ============================================================================

/// Create lagged values (shift forward, inserting Null at the start).
pub fn lag(series: &Series, periods: usize) -> Series {
    series.shift(periods as isize)
}

/// Create lead values (shift backward, inserting Null at the end).
pub fn lead(series: &Series, periods: usize) -> Series {
    series.shift(-(periods as isize))
}

// ============================================================================
// Series extension trait
// ============================================================================

/// Extract f64 values from a Series, replacing non-numeric with 0.0.
fn series_to_f64(s: &Series) -> Vec<f64> {
    s.to_f64().unwrap_or_else(|_| vec![0.0; s.len()])
}

fn f64_to_series(data: Vec<f64>) -> Series {
    Series::new(data.into_iter().map(Value::Float).collect::<Vec<_>>())
}

/// Extension trait for time-series operations on `Series`.
pub trait SeriesTimeSeriesExt {
    /// Exponentially weighted moving average.
    fn ewm(&self, alpha: f64) -> Series;
    /// Exponentially weighted moving average by span.
    fn ewm_span(&self, span: f64) -> Series;
    /// Expanding mean.
    fn expanding_mean(&self) -> Series;
    /// Expanding sum.
    fn expanding_sum(&self) -> Series;
    /// Expanding standard deviation (population).
    fn expanding_std(&self) -> Series;
    /// Expanding min.
    fn expanding_min(&self) -> Series;
    /// Expanding max.
    fn expanding_max(&self) -> Series;
}

impl SeriesTimeSeriesExt for Series {
    fn ewm(&self, alpha: f64) -> Series {
        f64_to_series(ewma(&series_to_f64(self), alpha))
    }

    fn ewm_span(&self, span: f64) -> Series {
        f64_to_series(ewma_span(&series_to_f64(self), span))
    }

    fn expanding_mean(&self) -> Series {
        f64_to_series(expanding_mean(&series_to_f64(self)))
    }

    fn expanding_sum(&self) -> Series {
        f64_to_series(expanding_sum(&series_to_f64(self)))
    }

    fn expanding_std(&self) -> Series {
        f64_to_series(expanding_std(&series_to_f64(self)))
    }

    fn expanding_min(&self) -> Series {
        f64_to_series(expanding_min(&series_to_f64(self)))
    }

    fn expanding_max(&self) -> Series {
        f64_to_series(expanding_max(&series_to_f64(self)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewma_basic() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = ewma(&data, 0.5);
        assert_eq!(result.len(), 5);
        assert!((result[0] - 1.0).abs() < 1e-10);
        // ewma[1] = 0.5 * 2 + 0.5 * 1 = 1.5
        assert!((result[1] - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_ewma_span() {
        let data = vec![1.0, 2.0, 3.0];
        // span=3 => alpha = 2/(3+1) = 0.5
        let r1 = ewma_span(&data, 3.0);
        let r2 = ewma(&data, 0.5);
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert!((a - b).abs() < 1e-10);
        }
    }

    #[test]
    fn test_expanding_mean() {
        let data = vec![2.0, 4.0, 6.0, 8.0];
        let result = expanding_mean(&data);
        assert!((result[0] - 2.0).abs() < 1e-10);
        assert!((result[1] - 3.0).abs() < 1e-10);
        assert!((result[2] - 4.0).abs() < 1e-10);
        assert!((result[3] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_expanding_sum() {
        let data = vec![1.0, 2.0, 3.0];
        let result = expanding_sum(&data);
        assert!((result[0] - 1.0).abs() < 1e-10);
        assert!((result[1] - 3.0).abs() < 1e-10);
        assert!((result[2] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_expanding_min_max() {
        let data = vec![3.0, 1.0, 4.0, 1.0, 5.0];
        let mins = expanding_min(&data);
        assert!((mins[0] - 3.0).abs() < 1e-10);
        assert!((mins[1] - 1.0).abs() < 1e-10);
        assert!((mins[4] - 1.0).abs() < 1e-10);

        let maxs = expanding_max(&data);
        assert!((maxs[0] - 3.0).abs() < 1e-10);
        assert!((maxs[2] - 4.0).abs() < 1e-10);
        assert!((maxs[4] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_lag_lead() {
        let s = Series::new(vec![1, 2, 3, 4, 5]);
        let lagged = lag(&s, 2);
        assert!(lagged.iloc(0).unwrap().is_null());
        assert!(lagged.iloc(1).unwrap().is_null());
        assert_eq!(lagged.iloc(2).unwrap(), &Value::Int(1));

        let led = lead(&s, 1);
        assert_eq!(led.iloc(0).unwrap(), &Value::Int(2));
        assert!(led.iloc(4).unwrap().is_null());
    }

    #[test]
    fn test_series_ewm() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0]);
        let result = s.ewm(0.5);
        assert_eq!(result.len(), 4);
    }
}
