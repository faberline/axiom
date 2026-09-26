//! Window functions: expanding and exponentially weighted moving (EWM).

use crate::frame::series::Series;
use crate::frame::value::Value;

/// Expanding window: accumulates all data from the start up to each point.
pub struct Expanding<'a> {
    series: &'a Series,
    min_periods: usize,
}

impl<'a> Expanding<'a> {
    /// Create a new expanding window.
    pub fn new(series: &'a Series) -> Self {
        Self {
            series,
            min_periods: 1,
        }
    }

    /// Set minimum periods required.
    pub fn min_periods(mut self, min_periods: usize) -> Self {
        self.min_periods = min_periods;
        self
    }

    /// Expanding sum.
    pub fn sum(&self) -> Series {
        self.apply(|vals| vals.iter().sum())
    }

    /// Expanding mean.
    pub fn mean(&self) -> Series {
        self.apply(|vals| vals.iter().sum::<f64>() / vals.len() as f64)
    }

    /// Expanding min.
    pub fn min(&self) -> Series {
        self.apply(|vals| vals.iter().cloned().fold(f64::INFINITY, f64::min))
    }

    /// Expanding max.
    pub fn max(&self) -> Series {
        self.apply(|vals| vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
    }

    /// Expanding standard deviation (sample).
    pub fn std(&self) -> Series {
        self.apply(|vals| {
            if vals.len() < 2 {
                return f64::NAN;
            }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            let var: f64 =
                vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
            var.sqrt()
        })
    }

    /// Expanding variance (sample).
    pub fn var(&self) -> Series {
        self.apply(|vals| {
            if vals.len() < 2 {
                return f64::NAN;
            }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64
        })
    }

    /// Apply a custom function to the expanding window.
    fn apply<F>(&self, f: F) -> Series
    where
        F: Fn(&[f64]) -> f64,
    {
        let vals: Vec<f64> = self
            .series
            .values()
            .iter()
            .map(|v| v.as_float().unwrap_or(f64::NAN))
            .collect();

        let n = vals.len();
        let mut result = Vec::with_capacity(n);

        for i in 0..n {
            let window: Vec<f64> = vals[..=i].iter().filter(|x| !x.is_nan()).cloned().collect();

            if window.len() < self.min_periods {
                result.push(Value::Null);
            } else {
                result.push(Value::Float(f(&window)));
            }
        }

        Series::new(result)
    }
}

/// Exponentially Weighted Moving (EWM) window.
pub struct Ewm<'a> {
    series: &'a Series,
    /// Smoothing factor (0 < alpha <= 1). Higher = more weight on recent values.
    alpha: f64,
    /// Minimum number of non-null periods.
    min_periods: usize,
    /// Whether to adjust weights (divide by decaying adjustment factor).
    adjust: bool,
}

impl<'a> Ewm<'a> {
    /// Create EWM with a span parameter (alpha = 2 / (span + 1)).
    pub fn with_span(series: &'a Series, span: f64) -> Self {
        Self {
            series,
            alpha: 2.0 / (span + 1.0),
            min_periods: 1,
            adjust: true,
        }
    }

    /// Create EWM with an alpha parameter directly.
    pub fn with_alpha(series: &'a Series, alpha: f64) -> Self {
        Self {
            series,
            alpha: alpha.clamp(0.0, 1.0),
            min_periods: 1,
            adjust: true,
        }
    }

    /// Create EWM with a halflife parameter (alpha = 1 - exp(-ln(2) / halflife)).
    pub fn with_halflife(series: &'a Series, halflife: f64) -> Self {
        let alpha = 1.0 - (-f64::ln(2.0) / halflife).exp();
        Self {
            series,
            alpha,
            min_periods: 1,
            adjust: true,
        }
    }

    /// Set minimum periods.
    pub fn min_periods(mut self, min_periods: usize) -> Self {
        self.min_periods = min_periods;
        self
    }

    /// Set adjust flag.
    pub fn adjust(mut self, adjust: bool) -> Self {
        self.adjust = adjust;
        self
    }

    /// EWM mean.
    pub fn mean(&self) -> Series {
        let vals: Vec<f64> = self
            .series
            .values()
            .iter()
            .map(|v| v.as_float().unwrap_or(f64::NAN))
            .collect();

        let n = vals.len();
        let mut result = Vec::with_capacity(n);
        let mut count = 0usize;

        if self.adjust {
            // Adjusted: ewma = sum(w_i * x_i) / sum(w_i)
            // where w_i = (1-alpha)^i
            let mut weighted_sum = 0.0;
            let mut weight_sum = 0.0;
            let decay = 1.0 - self.alpha;

            for val in &vals {
                if val.is_nan() {
                    result.push(Value::Null);
                    continue;
                }
                count += 1;
                weighted_sum = weighted_sum * decay + val;
                weight_sum = weight_sum * decay + 1.0;

                if count >= self.min_periods {
                    result.push(Value::Float(weighted_sum / weight_sum));
                } else {
                    result.push(Value::Null);
                }
            }
        } else {
            // Non-adjusted: ewma_t = alpha * x_t + (1-alpha) * ewma_{t-1}
            let mut ewma = 0.0;
            let mut initialized = false;

            for val in &vals {
                if val.is_nan() {
                    result.push(Value::Null);
                    continue;
                }
                count += 1;

                if !initialized {
                    ewma = *val;
                    initialized = true;
                } else {
                    ewma = self.alpha * val + (1.0 - self.alpha) * ewma;
                }

                if count >= self.min_periods {
                    result.push(Value::Float(ewma));
                } else {
                    result.push(Value::Null);
                }
            }
        }

        Series::new(result)
    }

    /// EWM standard deviation.
    pub fn std(&self) -> Series {
        let var = self.var_internal();
        let data: Vec<Value> = var
            .iter()
            .map(|v| match v {
                Value::Float(f) if !f.is_nan() => Value::Float(f.sqrt()),
                _ => Value::Null,
            })
            .collect();
        Series::new(data)
    }

    /// EWM variance.
    pub fn var(&self) -> Series {
        Series::new(self.var_internal())
    }

    /// Internal: compute EWM variance values.
    fn var_internal(&self) -> Vec<Value> {
        let mean_series = self.mean();
        let vals: Vec<f64> = self
            .series
            .values()
            .iter()
            .map(|v| v.as_float().unwrap_or(f64::NAN))
            .collect();

        let n = vals.len();
        let mut result = Vec::with_capacity(n);
        let decay = 1.0 - self.alpha;
        let mut weighted_sq_sum = 0.0;
        let mut weight_sum = 0.0;
        let mut count = 0usize;

        for (i, val) in vals.iter().enumerate() {
            if val.is_nan() {
                result.push(Value::Null);
                continue;
            }
            count += 1;

            let mean_val = mean_series
                .iloc(i)
                .ok()
                .and_then(|v| v.as_float())
                .unwrap_or(f64::NAN);

            let diff = val - mean_val;
            weighted_sq_sum = weighted_sq_sum * decay + diff * diff;
            weight_sum = weight_sum * decay + 1.0;

            if count >= self.min_periods.max(2) {
                // Bias correction for sample variance
                let bias = weight_sum / (weight_sum - 1.0);
                result.push(Value::Float(weighted_sq_sum / weight_sum * bias));
            } else {
                result.push(Value::Null);
            }
        }

        result
    }
}

// Extension methods for Series
impl Series {
    /// Create an expanding window.
    pub fn expanding(&self) -> Expanding<'_> {
        Expanding::new(self)
    }

    /// Create an EWM window with span.
    pub fn ewm_span(&self, span: f64) -> Ewm<'_> {
        Ewm::with_span(self, span)
    }

    /// Create an EWM window with alpha.
    pub fn ewm_alpha(&self, alpha: f64) -> Ewm<'_> {
        Ewm::with_alpha(self, alpha)
    }

    /// Create an EWM window with halflife.
    pub fn ewm_halflife(&self, halflife: f64) -> Ewm<'_> {
        Ewm::with_halflife(self, halflife)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expanding_sum() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let result = s.expanding().sum();
        assert_eq!(result.iloc(0).unwrap(), &Value::Float(1.0));
        assert_eq!(result.iloc(1).unwrap(), &Value::Float(3.0));
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(6.0));
        assert_eq!(result.iloc(3).unwrap(), &Value::Float(10.0));
        assert_eq!(result.iloc(4).unwrap(), &Value::Float(15.0));
    }

    #[test]
    fn test_expanding_mean() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let result = s.expanding().mean();
        assert_eq!(result.iloc(0).unwrap(), &Value::Float(1.0));
        assert_eq!(result.iloc(1).unwrap(), &Value::Float(1.5));
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(2.0));
        assert_eq!(result.iloc(4).unwrap(), &Value::Float(3.0));
    }

    #[test]
    fn test_expanding_min_max() {
        let s = Series::new(vec![3.0, 1.0, 4.0, 1.0, 5.0]);
        let mins = s.expanding().min();
        assert_eq!(mins.iloc(0).unwrap(), &Value::Float(3.0));
        assert_eq!(mins.iloc(1).unwrap(), &Value::Float(1.0));
        assert_eq!(mins.iloc(2).unwrap(), &Value::Float(1.0));

        let maxs = s.expanding().max();
        assert_eq!(maxs.iloc(0).unwrap(), &Value::Float(3.0));
        assert_eq!(maxs.iloc(2).unwrap(), &Value::Float(4.0));
        assert_eq!(maxs.iloc(4).unwrap(), &Value::Float(5.0));
    }

    #[test]
    fn test_expanding_std() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0]);
        let result = s.expanding().min_periods(2).std();
        assert!(result.iloc(0).unwrap().is_null());
        // std of [1, 2] = sqrt(0.5) ~ 0.707
        let v = result.iloc(1).unwrap().as_float().unwrap();
        assert!((v - 0.7071067811865476).abs() < 1e-10);
    }

    #[test]
    fn test_expanding_min_periods() {
        let s = Series::new(vec![1.0, 2.0, 3.0]);
        let result = s.expanding().min_periods(3).sum();
        assert!(result.iloc(0).unwrap().is_null());
        assert!(result.iloc(1).unwrap().is_null());
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(6.0));
    }

    #[test]
    fn test_ewm_mean_span() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let result = s.ewm_span(3.0).mean();
        assert_eq!(result.len(), 5);
        // First value should be the value itself
        assert_eq!(result.iloc(0).unwrap(), &Value::Float(1.0));
        // Subsequent values should be weighted averages
        assert!(result.iloc(1).unwrap().as_float().is_some());
        assert!(result.iloc(4).unwrap().as_float().is_some());
    }

    #[test]
    fn test_ewm_mean_alpha() {
        let s = Series::new(vec![1.0, 2.0, 3.0]);
        let result = s.ewm_alpha(0.5).mean();
        assert_eq!(result.len(), 3);
        assert_eq!(result.iloc(0).unwrap(), &Value::Float(1.0));
    }

    #[test]
    fn test_ewm_non_adjusted() {
        let s = Series::new(vec![1.0, 2.0, 3.0]);
        let result = s.ewm_alpha(0.5).adjust(false).mean();
        assert_eq!(result.len(), 3);
        assert_eq!(result.iloc(0).unwrap(), &Value::Float(1.0));
        // ewma_1 = 0.5 * 2 + 0.5 * 1 = 1.5
        assert_eq!(result.iloc(1).unwrap(), &Value::Float(1.5));
        // ewma_2 = 0.5 * 3 + 0.5 * 1.5 = 2.25
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(2.25));
    }

    #[test]
    fn test_ewm_var_std() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let var_result = s.ewm_span(3.0).var();
        let std_result = s.ewm_span(3.0).std();
        assert_eq!(var_result.len(), 5);
        assert_eq!(std_result.len(), 5);
        // First value should be null (need at least 2 for variance)
        assert!(var_result.iloc(0).unwrap().is_null());
    }

    #[test]
    fn test_ewm_halflife() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0]);
        let result = s.ewm_halflife(1.0).mean();
        assert_eq!(result.len(), 4);
        assert!(result.iloc(0).unwrap().as_float().is_some());
    }
}
