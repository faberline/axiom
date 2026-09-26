//! Series - one-dimensional labeled array.

use super::error::{FrameError, Result};
use super::index::Index;
use super::value::Value;

/// A one-dimensional labeled array.
#[derive(Debug, Clone)]
pub struct Series {
    /// Series name.
    name: Option<String>,
    /// Data values.
    data: Vec<Value>,
    /// Row index.
    index: Index,
}

impl Series {
    /// Create a new Series from values.
    pub fn new<V: Into<Value>>(data: Vec<V>) -> Self {
        let data: Vec<Value> = data.into_iter().map(|v| v.into()).collect();
        let len = data.len();
        Self {
            name: None,
            data,
            index: Index::Range(len),
        }
    }

    /// Create a Series with a name.
    pub fn with_name<V: Into<Value>>(data: Vec<V>, name: impl Into<String>) -> Self {
        let mut series = Self::new(data);
        series.name = Some(name.into());
        series
    }

    /// Create a Series with a custom index.
    pub fn with_index<V: Into<Value>>(data: Vec<V>, index: Index) -> Result<Self> {
        let data: Vec<Value> = data.into_iter().map(|v| v.into()).collect();
        if data.len() != index.len() {
            return Err(FrameError::ShapeMismatch {
                expected: index.len(),
                actual: data.len(),
            });
        }
        Ok(Self {
            name: None,
            data,
            index,
        })
    }

    /// Get the series name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Set the series name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = Some(name.into());
    }

    /// Get the length of the series.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the series is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get the index.
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// Get value by positional index (iloc).
    pub fn iloc(&self, pos: usize) -> Result<&Value> {
        self.data.get(pos).ok_or(FrameError::IndexOutOfBounds {
            index: pos,
            length: self.len(),
        })
    }

    /// Get value by label (loc).
    pub fn loc(&self, label: &Value) -> Result<&Value> {
        let pos = self.index.get_loc(label)?;
        Ok(&self.data[pos])
    }

    /// Get multiple values by positions (iloc).
    pub fn iloc_many(&self, positions: &[usize]) -> Result<Series> {
        let data: Result<Vec<_>> = positions.iter().map(|&p| self.iloc(p).cloned()).collect();
        let new_index = self.index.slice(positions)?;
        Ok(Series {
            name: self.name.clone(),
            data: data?,
            index: new_index,
        })
    }

    /// Get the underlying data as a slice.
    pub fn values(&self) -> &[Value] {
        &self.data
    }

    /// Convert to Vec of f64 (for numeric operations).
    pub fn to_f64(&self) -> Result<Vec<f64>> {
        self.data
            .iter()
            .map(|v| {
                v.as_float()
                    .ok_or_else(|| FrameError::TypeMismatch("cannot convert to f64".into()))
            })
            .collect()
    }

    /// Convert to Vec of i64 (for integer operations).
    pub fn to_i64(&self) -> Result<Vec<i64>> {
        self.data
            .iter()
            .map(|v| {
                v.as_int()
                    .ok_or_else(|| FrameError::TypeMismatch("cannot convert to i64".into()))
            })
            .collect()
    }

    /// Sum of numeric values.
    pub fn sum(&self) -> Result<f64> {
        Ok(self.to_f64()?.into_iter().sum())
    }

    /// Mean of numeric values.
    pub fn mean(&self) -> Result<f64> {
        let vals = self.to_f64()?;
        if vals.is_empty() {
            return Err(FrameError::InvalidOperation("mean of empty series".into()));
        }
        Ok(vals.iter().sum::<f64>() / vals.len() as f64)
    }

    /// Minimum value.
    pub fn min(&self) -> Result<Value> {
        self.data
            .iter()
            .filter(|v| !v.is_null())
            .min()
            .cloned()
            .ok_or_else(|| FrameError::InvalidOperation("min of empty series".into()))
    }

    /// Maximum value.
    pub fn max(&self) -> Result<Value> {
        self.data
            .iter()
            .filter(|v| !v.is_null())
            .max()
            .cloned()
            .ok_or_else(|| FrameError::InvalidOperation("max of empty series".into()))
    }

    /// Count non-null values.
    pub fn count(&self) -> usize {
        self.data.iter().filter(|v| !v.is_null()).count()
    }

    /// Filter by boolean mask.
    pub fn filter(&self, mask: &[bool]) -> Result<Series> {
        if mask.len() != self.len() {
            return Err(FrameError::ShapeMismatch {
                expected: self.len(),
                actual: mask.len(),
            });
        }

        let positions: Vec<usize> = mask
            .iter()
            .enumerate()
            .filter_map(|(i, &b)| if b { Some(i) } else { None })
            .collect();

        self.iloc_many(&positions)
    }

    /// Apply a function to each value.
    pub fn map<F>(&self, f: F) -> Series
    where
        F: Fn(&Value) -> Value,
    {
        Series {
            name: self.name.clone(),
            data: self.data.iter().map(f).collect(),
            index: self.index.clone(),
        }
    }

    /// Reset index to range.
    pub fn reset_index(&mut self) {
        self.index = Index::Range(self.len());
    }

    /// Sort values.
    pub fn sort(&self, ascending: bool) -> Series {
        let mut indexed: Vec<(usize, &Value)> = self.data.iter().enumerate().collect();
        if ascending {
            indexed.sort_by(|a, b| a.1.cmp(b.1));
        } else {
            indexed.sort_by(|a, b| b.1.cmp(a.1));
        }

        let positions: Vec<usize> = indexed.iter().map(|(i, _)| *i).collect();
        // This unwrap is safe because positions come from valid indices
        self.iloc_many(&positions).unwrap()
    }

    /// Get unique values.
    pub fn unique(&self) -> Vec<Value> {
        let mut seen = std::collections::HashSet::new();
        self.data
            .iter()
            .filter(|v| seen.insert((*v).clone()))
            .cloned()
            .collect()
    }

    /// Count occurrences of each value.
    pub fn value_counts(&self) -> std::collections::HashMap<Value, usize> {
        let mut counts = std::collections::HashMap::new();
        for v in &self.data {
            *counts.entry(v.clone()).or_insert(0) += 1;
        }
        counts
    }

    // === Null handling (pandas-style) ===

    /// Return a boolean mask indicating which values are null.
    ///
    /// # Example
    /// ```ignore
    /// let s = Series::new(vec![Some(1), None, Some(3)]);
    /// assert_eq!(s.isna(), vec![false, true, false]);
    /// ```
    pub fn isna(&self) -> Vec<bool> {
        self.data.iter().map(|v| v.is_null()).collect()
    }

    /// Return a boolean mask indicating which values are not null.
    pub fn notna(&self) -> Vec<bool> {
        self.data.iter().map(|v| !v.is_null()).collect()
    }

    /// Fill null values with the given value.
    ///
    /// # Example
    /// ```ignore
    /// let s = Series::new(vec![Some(1), None, Some(3)]);
    /// let filled = s.fillna(Value::Int(0));
    /// ```
    pub fn fillna(&self, value: Value) -> Series {
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| {
                if v.is_null() {
                    value.clone()
                } else {
                    v.clone()
                }
            })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Fill null values using forward fill (propagate last valid value).
    pub fn ffill(&self) -> Series {
        let mut data = Vec::with_capacity(self.data.len());
        let mut last_valid: Option<Value> = None;

        for v in &self.data {
            if v.is_null() {
                data.push(last_valid.clone().unwrap_or(Value::Null));
            } else {
                last_valid = Some(v.clone());
                data.push(v.clone());
            }
        }

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Fill null values using backward fill (propagate next valid value).
    pub fn bfill(&self) -> Series {
        let mut data = vec![Value::Null; self.data.len()];
        let mut next_valid: Option<Value> = None;

        for (i, v) in self.data.iter().enumerate().rev() {
            if v.is_null() {
                data[i] = next_valid.clone().unwrap_or(Value::Null);
            } else {
                next_valid = Some(v.clone());
                data[i] = v.clone();
            }
        }

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Drop null values, returning a new Series.
    ///
    /// # Example
    /// ```ignore
    /// let s = Series::new(vec![Some(1), None, Some(3)]);
    /// let clean = s.dropna();
    /// assert_eq!(clean.len(), 2);
    /// ```
    pub fn dropna(&self) -> Series {
        let mask = self.notna();
        // filter will not fail because mask is same length as data
        self.filter(&mask).unwrap_or_else(|_| self.clone())
    }

    /// Interpolate missing values using linear interpolation.
    ///
    /// # Example
    /// ```ignore
    /// let s = Series::new(vec![Value::Float(1.0), Value::Null, Value::Float(3.0)]);
    /// let filled = s.interpolate();
    /// // filled[1] = 2.0
    /// ```
    pub fn interpolate(&self) -> Series {
        self.interpolate_method("linear")
    }

    /// Interpolate missing values using the specified method.
    ///
    /// Methods:
    /// - "linear": Linear interpolation between valid values
    /// - "ffill": Forward fill (same as ffill())
    /// - "bfill": Backward fill (same as bfill())
    /// - "nearest": Use nearest valid value
    pub fn interpolate_method(&self, method: &str) -> Series {
        match method {
            "ffill" => self.ffill(),
            "bfill" => self.bfill(),
            "nearest" => self.interpolate_nearest(),
            _ => self.interpolate_linear(),
        }
    }

    /// Linear interpolation.
    ///
    /// Only interpolates Null values. Non-null values (including non-numeric) are preserved.
    fn interpolate_linear(&self) -> Series {
        let n = self.data.len();
        let mut result = self.data.clone();

        // Find valid numeric indices and values (non-null and numeric)
        let valid: Vec<(usize, f64)> = self
            .data
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                if v.is_null() {
                    None
                } else {
                    v.as_float().map(|f| (i, f))
                }
            })
            .collect();

        if valid.len() < 2 {
            // Not enough points to interpolate
            return Series {
                name: self.name.clone(),
                data: result,
                index: self.index.clone(),
            };
        }

        // Only interpolate Null values, preserve non-null non-numeric values
        for i in 0..n {
            if result[i].is_null() {
                // Find surrounding valid numeric points
                let prev = valid.iter().rev().find(|(idx, _)| *idx < i);
                let next = valid.iter().find(|(idx, _)| *idx > i);

                match (prev, next) {
                    (Some(&(i0, v0)), Some(&(i1, v1))) => {
                        // Linear interpolation
                        let t = (i - i0) as f64 / (i1 - i0) as f64;
                        result[i] = Value::Float(v0 + t * (v1 - v0));
                    }
                    (Some(&(_, v)), None) => {
                        // Extrapolate forward (use last value)
                        result[i] = Value::Float(v);
                    }
                    (None, Some(&(_, v))) => {
                        // Extrapolate backward (use first value)
                        result[i] = Value::Float(v);
                    }
                    (None, None) => {
                        // No valid values
                    }
                }
            }
            // Non-null values (including non-numeric) are preserved as-is
        }

        Series {
            name: self.name.clone(),
            data: result,
            index: self.index.clone(),
        }
    }

    /// Nearest value interpolation.
    ///
    /// Only interpolates Null values. Non-null values (including non-numeric) are preserved.
    fn interpolate_nearest(&self) -> Series {
        let n = self.data.len();
        let mut result = self.data.clone();

        // Find valid numeric indices and values (non-null and numeric)
        let valid: Vec<(usize, f64)> = self
            .data
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                if v.is_null() {
                    None
                } else {
                    v.as_float().map(|f| (i, f))
                }
            })
            .collect();

        if valid.is_empty() {
            return Series {
                name: self.name.clone(),
                data: result,
                index: self.index.clone(),
            };
        }

        // Only interpolate Null values
        for i in 0..n {
            if result[i].is_null() {
                // Find nearest valid numeric point
                let nearest = valid
                    .iter()
                    .min_by_key(|(idx, _)| (*idx as isize - i as isize).unsigned_abs())
                    .map(|(_, v)| *v);

                if let Some(v) = nearest {
                    result[i] = Value::Float(v);
                }
            }
            // Non-null values are preserved as-is
        }

        Series {
            name: self.name.clone(),
            data: result,
            index: self.index.clone(),
        }
    }

    /// Count null values.
    pub fn null_count(&self) -> usize {
        self.data.iter().filter(|v| v.is_null()).count()
    }

    /// Check if any values are null.
    pub fn has_nulls(&self) -> bool {
        self.data.iter().any(|v| v.is_null())
    }

    /// Apply a function to each value.
    ///
    /// # Example
    /// ```ignore
    /// let s = Series::new(vec![1, 2, 3]);
    /// let doubled = s.apply(|v| Value::Float(v.as_float().unwrap_or(0.0) * 2.0));
    /// ```
    pub fn apply<F>(&self, f: F) -> Series
    where
        F: Fn(&Value) -> Value,
    {
        Series {
            name: self.name.clone(),
            data: self.data.iter().map(f).collect(),
            index: self.index.clone(),
        }
    }

    // === Cumulative operations ===

    /// Cumulative sum.
    pub fn cumsum(&self) -> Series {
        let mut acc = 0.0;
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| {
                if let Some(val) = v.as_float() {
                    acc += val;
                    Value::Float(acc)
                } else {
                    Value::Null
                }
            })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Cumulative product.
    pub fn cumprod(&self) -> Series {
        let mut acc = 1.0;
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| {
                if let Some(val) = v.as_float() {
                    acc *= val;
                    Value::Float(acc)
                } else {
                    Value::Null
                }
            })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Cumulative maximum.
    pub fn cummax(&self) -> Series {
        let mut max_val: Option<f64> = None;
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| {
                if let Some(val) = v.as_float() {
                    max_val = Some(match max_val {
                        None => val,
                        Some(m) => {
                            if val > m {
                                val
                            } else {
                                m
                            }
                        }
                    });
                    Value::Float(max_val.unwrap())
                } else {
                    Value::Null
                }
            })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Cumulative minimum.
    pub fn cummin(&self) -> Series {
        let mut min_val: Option<f64> = None;
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| {
                if let Some(val) = v.as_float() {
                    min_val = Some(match min_val {
                        None => val,
                        Some(m) => {
                            if val < m {
                                val
                            } else {
                                m
                            }
                        }
                    });
                    Value::Float(min_val.unwrap())
                } else {
                    Value::Null
                }
            })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Shift values by n periods.
    ///
    /// Positive n shifts down (introduces NaN at the beginning).
    /// Negative n shifts up (introduces NaN at the end).
    pub fn shift(&self, n: isize) -> Series {
        let len = self.data.len();
        let mut data = vec![Value::Null; len];

        if n >= 0 {
            let n = n as usize;
            for i in n..len {
                data[i] = self.data[i - n].clone();
            }
        } else {
            let n = (-n) as usize;
            for i in 0..len.saturating_sub(n) {
                data[i] = self.data[i + n].clone();
            }
        }

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Compute difference between consecutive elements.
    ///
    /// First element will be Null.
    pub fn diff(&self, periods: usize) -> Series {
        let len = self.data.len();
        let mut data = vec![Value::Null; len];

        for i in periods..len {
            match (self.data[i].as_float(), self.data[i - periods].as_float()) {
                (Some(curr), Some(prev)) => {
                    data[i] = Value::Float(curr - prev);
                }
                _ => {
                    data[i] = Value::Null;
                }
            }
        }

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Compute percentage change between consecutive elements.
    pub fn pct_change(&self, periods: usize) -> Series {
        let len = self.data.len();
        let mut data = vec![Value::Null; len];

        for i in periods..len {
            match (self.data[i].as_float(), self.data[i - periods].as_float()) {
                (Some(curr), Some(prev)) if prev != 0.0 => {
                    data[i] = Value::Float((curr - prev) / prev);
                }
                _ => {
                    data[i] = Value::Null;
                }
            }
        }

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    // === Selection ===

    /// Get the n largest values.
    pub fn nlargest(&self, n: usize) -> Series {
        let sorted = self.sort(false);
        let positions: Vec<usize> = (0..n.min(sorted.len())).collect();
        sorted
            .iloc_many(&positions)
            .unwrap_or_else(|_| self.clone())
    }

    /// Get the n smallest values.
    pub fn nsmallest(&self, n: usize) -> Series {
        let sorted = self.sort(true);
        let positions: Vec<usize> = (0..n.min(sorted.len())).collect();
        sorted
            .iloc_many(&positions)
            .unwrap_or_else(|_| self.clone())
    }

    /// Clip values to a range.
    pub fn clip(&self, lower: f64, upper: f64) -> Series {
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| {
                if let Some(val) = v.as_float() {
                    if val < lower {
                        Value::Float(lower)
                    } else if val > upper {
                        Value::Float(upper)
                    } else {
                        Value::Float(val)
                    }
                } else {
                    v.clone()
                }
            })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Standard deviation.
    pub fn std(&self) -> Result<f64> {
        let vals = self.to_f64()?;
        if vals.len() <= 1 {
            return Err(FrameError::InvalidOperation(
                "std requires at least 2 values".into(),
            ));
        }
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        let variance: f64 =
            vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
        Ok(variance.sqrt())
    }

    /// Variance.
    pub fn var(&self) -> Result<f64> {
        let vals = self.to_f64()?;
        if vals.len() <= 1 {
            return Err(FrameError::InvalidOperation(
                "var requires at least 2 values".into(),
            ));
        }
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        let variance: f64 =
            vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
        Ok(variance)
    }

    /// Median value.
    pub fn median(&self) -> Result<f64> {
        let mut vals = self.to_f64()?;
        if vals.is_empty() {
            return Err(FrameError::InvalidOperation(
                "median of empty series".into(),
            ));
        }
        vals.sort_by(|a, b| a.total_cmp(b));
        let mid = vals.len() / 2;
        if vals.len() % 2 == 0 {
            Ok((vals[mid - 1] + vals[mid]) / 2.0)
        } else {
            Ok(vals[mid])
        }
    }

    // === Advanced operations ===

    /// Rank values (1-based, ascending).
    pub fn rank(&self) -> Series {
        let n = self.len();
        let mut indexed: Vec<(usize, &Value)> = self.data.iter().enumerate().collect();
        indexed.sort_by(|a, b| a.1.cmp(b.1));

        let mut ranks = vec![Value::Null; n];
        for (rank, (original_idx, _)) in indexed.iter().enumerate() {
            ranks[*original_idx] = Value::Float((rank + 1) as f64);
        }

        Series {
            name: self.name.clone(),
            data: ranks,
            index: self.index.clone(),
        }
    }

    /// Check if values are duplicated.
    pub fn duplicated(&self, keep: &str) -> Vec<bool> {
        let mut seen = std::collections::HashMap::new();
        let mut result = vec![false; self.len()];

        match keep {
            "first" => {
                for (i, v) in self.data.iter().enumerate() {
                    if seen.contains_key(v) {
                        result[i] = true;
                    } else {
                        seen.insert(v.clone(), i);
                    }
                }
            }
            "last" => {
                // First pass: mark all as duplicated
                for (i, v) in self.data.iter().enumerate() {
                    if let Some(&prev_idx) = seen.get(v) {
                        result[prev_idx] = true;
                    }
                    seen.insert(v.clone(), i);
                }
            }
            _ => {
                // Mark all duplicates
                let mut counts = std::collections::HashMap::new();
                for v in &self.data {
                    *counts.entry(v.clone()).or_insert(0) += 1;
                }
                for (i, v) in self.data.iter().enumerate() {
                    if counts.get(v).map(|&c| c > 1).unwrap_or(false) {
                        result[i] = true;
                    }
                }
            }
        }

        result
    }

    /// Drop duplicate values.
    pub fn drop_duplicates(&self, keep: &str) -> Series {
        let mask: Vec<bool> = self.duplicated(keep).iter().map(|&d| !d).collect();
        self.filter(&mask).unwrap_or_else(|_| self.clone())
    }

    /// Check if values are in a set.
    pub fn isin(&self, values: &[Value]) -> Vec<bool> {
        let set: std::collections::HashSet<_> = values.iter().collect();
        self.data.iter().map(|v| set.contains(v)).collect()
    }

    /// Check if values are between lower and upper (inclusive).
    pub fn between(&self, lower: f64, upper: f64) -> Vec<bool> {
        self.data
            .iter()
            .map(|v| {
                v.as_float()
                    .map(|f| f >= lower && f <= upper)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Replace values.
    pub fn replace(&self, to_replace: &Value, value: &Value) -> Series {
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| {
                if v == to_replace {
                    value.clone()
                } else {
                    v.clone()
                }
            })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Replace multiple values.
    pub fn replace_map(&self, mapping: &std::collections::HashMap<Value, Value>) -> Series {
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| mapping.get(v).cloned().unwrap_or_else(|| v.clone()))
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Convert to dict (index -> value).
    pub fn to_dict(&self) -> std::collections::HashMap<Value, Value> {
        let mut result = std::collections::HashMap::new();
        for (i, v) in self.data.iter().enumerate() {
            let key = self.index.get_label(i).unwrap_or(Value::Int(i as i64));
            result.insert(key, v.clone());
        }
        result
    }

    /// Quantile (q between 0 and 1).
    pub fn quantile(&self, q: f64) -> Result<f64> {
        if q < 0.0 || q > 1.0 {
            return Err(FrameError::InvalidOperation(
                "q must be between 0 and 1".into(),
            ));
        }

        let mut vals = self.to_f64()?;
        if vals.is_empty() {
            return Err(FrameError::InvalidOperation(
                "quantile of empty series".into(),
            ));
        }

        vals.sort_by(|a, b| a.total_cmp(b));
        let idx = q * (vals.len() - 1) as f64;
        let lower = idx.floor() as usize;
        let upper = idx.ceil() as usize;

        if lower == upper {
            Ok(vals[lower])
        } else {
            let frac = idx - lower as f64;
            Ok(vals[lower] * (1.0 - frac) + vals[upper] * frac)
        }
    }

    /// Correlation with another series.
    pub fn corr(&self, other: &Series) -> Result<f64> {
        if self.len() != other.len() {
            return Err(FrameError::ShapeMismatch {
                expected: self.len(),
                actual: other.len(),
            });
        }

        let x = self.to_f64()?;
        let y = other.to_f64()?;
        let n = x.len() as f64;

        if n < 2.0 {
            return Err(FrameError::InvalidOperation(
                "need at least 2 values".into(),
            ));
        }

        let mean_x: f64 = x.iter().sum::<f64>() / n;
        let mean_y: f64 = y.iter().sum::<f64>() / n;

        let mut cov = 0.0;
        let mut var_x = 0.0;
        let mut var_y = 0.0;

        for (&xi, &yi) in x.iter().zip(y.iter()) {
            let dx = xi - mean_x;
            let dy = yi - mean_y;
            cov += dx * dy;
            var_x += dx * dx;
            var_y += dy * dy;
        }

        let std_x = var_x.sqrt();
        let std_y = var_y.sqrt();

        if std_x == 0.0 || std_y == 0.0 {
            return Ok(0.0);
        }

        Ok(cov / (std_x * std_y))
    }

    // === Rolling window ===

    /// Create a rolling window.
    pub fn rolling(&self, window: usize) -> SeriesRolling<'_> {
        SeriesRolling {
            series: self,
            window,
            min_periods: window,
        }
    }

    // === Type conversion ===

    /// Convert series to a specific type.
    pub fn astype(&self, dtype: &str) -> Series {
        let data: Vec<Value> = match dtype {
            "int" | "i64" => self
                .data
                .iter()
                .map(|v| v.as_int().map(Value::Int).unwrap_or(Value::Null))
                .collect(),
            "float" | "f64" => self
                .data
                .iter()
                .map(|v| v.as_float().map(Value::Float).unwrap_or(Value::Null))
                .collect(),
            "str" | "string" => self
                .data
                .iter()
                .map(|v| Value::String(v.to_string()))
                .collect(),
            "bool" => self
                .data
                .iter()
                .map(|v| v.as_bool().map(Value::Bool).unwrap_or(Value::Null))
                .collect(),
            _ => self.data.clone(),
        };

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Conditional replacement: where condition is false, replace with other.
    pub fn where_cond(&self, cond: &[bool], other: &Value) -> Series {
        let data: Vec<Value> = self
            .data
            .iter()
            .zip(cond.iter())
            .map(|(v, &c)| if c { v.clone() } else { other.clone() })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Mask: where condition is true, replace with other.
    pub fn mask(&self, cond: &[bool], other: &Value) -> Series {
        let data: Vec<Value> = self
            .data
            .iter()
            .zip(cond.iter())
            .map(|(v, &c)| if c { other.clone() } else { v.clone() })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Absolute value.
    pub fn abs(&self) -> Series {
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| match v {
                Value::Int(i) => Value::Int(i.abs()),
                Value::Float(f) => Value::Float(f.abs()),
                _ => v.clone(),
            })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }

    /// Round values.
    pub fn round(&self, decimals: i32) -> Series {
        let factor = 10f64.powi(decimals);
        let data: Vec<Value> = self
            .data
            .iter()
            .map(|v| match v.as_float() {
                Some(f) => Value::Float((f * factor).round() / factor),
                None => v.clone(),
            })
            .collect();

        Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        }
    }
}

/// Rolling window for Series.
pub struct SeriesRolling<'a> {
    series: &'a Series,
    window: usize,
    min_periods: usize,
}

impl<'a> SeriesRolling<'a> {
    /// Set minimum periods.
    pub fn min_periods(mut self, min_periods: usize) -> Self {
        self.min_periods = min_periods;
        self
    }

    /// Rolling sum.
    pub fn sum(&self) -> Series {
        self.apply(|vals| vals.iter().sum())
    }

    /// Rolling mean.
    pub fn mean(&self) -> Series {
        self.apply(|vals| vals.iter().sum::<f64>() / vals.len() as f64)
    }

    /// Rolling min.
    pub fn min(&self) -> Series {
        self.apply(|vals| vals.iter().cloned().fold(f64::INFINITY, f64::min))
    }

    /// Rolling max.
    pub fn max(&self) -> Series {
        self.apply(|vals| vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
    }

    /// Rolling std.
    pub fn std(&self) -> Series {
        self.apply(|vals| {
            if vals.len() < 2 {
                return f64::NAN;
            }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            let var =
                vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
            var.sqrt()
        })
    }

    /// Rolling var.
    pub fn var(&self) -> Series {
        self.apply(|vals| {
            if vals.len() < 2 {
                return f64::NAN;
            }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64
        })
    }

    /// Apply a function to rolling windows.
    fn apply<F>(&self, f: F) -> Series
    where
        F: Fn(&[f64]) -> f64,
    {
        let vals: Vec<f64> = self
            .series
            .data
            .iter()
            .map(|v| v.as_float().unwrap_or(f64::NAN))
            .collect();

        let n = vals.len();
        let mut result = Vec::with_capacity(n);

        for i in 0..n {
            if i + 1 < self.min_periods {
                result.push(Value::Null);
            } else {
                let start = i.saturating_sub(self.window - 1);
                let window_vals: Vec<f64> = vals[start..=i]
                    .iter()
                    .filter(|x| !x.is_nan())
                    .cloned()
                    .collect();

                if window_vals.len() < self.min_periods {
                    result.push(Value::Null);
                } else {
                    result.push(Value::Float(f(&window_vals)));
                }
            }
        }

        Series {
            name: self.series.name.clone(),
            data: result,
            index: self.series.index.clone(),
        }
    }
}

// Arithmetic operations for numeric Series
impl std::ops::Add for &Series {
    type Output = Result<Series>;

    fn add(self, other: &Series) -> Self::Output {
        if self.len() != other.len() {
            return Err(FrameError::ShapeMismatch {
                expected: self.len(),
                actual: other.len(),
            });
        }

        let data: Vec<Value> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| match (a.as_float(), b.as_float()) {
                (Some(x), Some(y)) => Value::Float(x + y),
                _ => Value::Null,
            })
            .collect();

        Ok(Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        })
    }
}

impl std::ops::Sub for &Series {
    type Output = Result<Series>;

    fn sub(self, other: &Series) -> Self::Output {
        if self.len() != other.len() {
            return Err(FrameError::ShapeMismatch {
                expected: self.len(),
                actual: other.len(),
            });
        }

        let data: Vec<Value> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| match (a.as_float(), b.as_float()) {
                (Some(x), Some(y)) => Value::Float(x - y),
                _ => Value::Null,
            })
            .collect();

        Ok(Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        })
    }
}

impl std::ops::Mul for &Series {
    type Output = Result<Series>;

    fn mul(self, other: &Series) -> Self::Output {
        if self.len() != other.len() {
            return Err(FrameError::ShapeMismatch {
                expected: self.len(),
                actual: other.len(),
            });
        }

        let data: Vec<Value> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| match (a.as_float(), b.as_float()) {
                (Some(x), Some(y)) => Value::Float(x * y),
                _ => Value::Null,
            })
            .collect();

        Ok(Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        })
    }
}

impl std::ops::Div for &Series {
    type Output = Result<Series>;

    fn div(self, other: &Series) -> Self::Output {
        if self.len() != other.len() {
            return Err(FrameError::ShapeMismatch {
                expected: self.len(),
                actual: other.len(),
            });
        }

        let data: Vec<Value> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| match (a.as_float(), b.as_float()) {
                (Some(x), Some(y)) if y != 0.0 => Value::Float(x / y),
                _ => Value::Null,
            })
            .collect();

        Ok(Series {
            name: self.name.clone(),
            data,
            index: self.index.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_series_basic() {
        let s = Series::new(vec![1, 2, 3, 4, 5]);
        assert_eq!(s.len(), 5);
        assert_eq!(s.iloc(0).unwrap(), &Value::Int(1));
        assert_eq!(s.iloc(4).unwrap(), &Value::Int(5));
    }

    #[test]
    fn test_series_with_name() {
        let s = Series::with_name(vec![1.0, 2.0, 3.0], "values");
        assert_eq!(s.name(), Some("values"));
    }

    #[test]
    fn test_series_sum_mean() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(s.sum().unwrap(), 15.0);
        assert_eq!(s.mean().unwrap(), 3.0);
    }

    #[test]
    fn test_series_filter() {
        let s = Series::new(vec![1, 2, 3, 4, 5]);
        let mask = vec![true, false, true, false, true];
        let filtered = s.filter(&mask).unwrap();
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered.iloc(0).unwrap(), &Value::Int(1));
        assert_eq!(filtered.iloc(1).unwrap(), &Value::Int(3));
        assert_eq!(filtered.iloc(2).unwrap(), &Value::Int(5));
    }

    #[test]
    fn test_series_arithmetic() {
        let a = Series::new(vec![1.0, 2.0, 3.0]);
        let b = Series::new(vec![10.0, 20.0, 30.0]);
        let sum = (&a + &b).unwrap();
        assert_eq!(sum.iloc(0).unwrap(), &Value::Float(11.0));
        assert_eq!(sum.iloc(2).unwrap(), &Value::Float(33.0));
    }

    #[test]
    fn test_series_sort() {
        let s = Series::new(vec![3, 1, 4, 1, 5, 9, 2, 6]);
        let sorted = s.sort(true);
        assert_eq!(sorted.iloc(0).unwrap(), &Value::Int(1));
        assert_eq!(sorted.iloc(1).unwrap(), &Value::Int(1));
        assert_eq!(sorted.iloc(2).unwrap(), &Value::Int(2));
    }

    #[test]
    fn test_series_unique() {
        let s = Series::new(vec![1, 2, 2, 3, 3, 3]);
        let unique = s.unique();
        assert_eq!(unique.len(), 3);
    }

    #[test]
    fn test_series_isna() {
        let s = Series::new(vec![Value::Int(1), Value::Null, Value::Int(3), Value::Null]);
        let mask = s.isna();
        assert_eq!(mask, vec![false, true, false, true]);
    }

    #[test]
    fn test_series_fillna() {
        let s = Series::new(vec![Value::Int(1), Value::Null, Value::Int(3), Value::Null]);
        let filled = s.fillna(Value::Int(0));
        assert_eq!(filled.iloc(0).unwrap(), &Value::Int(1));
        assert_eq!(filled.iloc(1).unwrap(), &Value::Int(0));
        assert_eq!(filled.iloc(2).unwrap(), &Value::Int(3));
        assert_eq!(filled.iloc(3).unwrap(), &Value::Int(0));
    }

    #[test]
    fn test_series_ffill() {
        let s = Series::new(vec![Value::Int(1), Value::Null, Value::Null, Value::Int(4)]);
        let filled = s.ffill();
        assert_eq!(filled.iloc(1).unwrap(), &Value::Int(1));
        assert_eq!(filled.iloc(2).unwrap(), &Value::Int(1));
    }

    #[test]
    fn test_series_bfill() {
        let s = Series::new(vec![Value::Null, Value::Null, Value::Int(3), Value::Int(4)]);
        let filled = s.bfill();
        assert_eq!(filled.iloc(0).unwrap(), &Value::Int(3));
        assert_eq!(filled.iloc(1).unwrap(), &Value::Int(3));
    }

    #[test]
    fn test_series_dropna() {
        let s = Series::new(vec![Value::Int(1), Value::Null, Value::Int(3), Value::Null]);
        let clean = s.dropna();
        assert_eq!(clean.len(), 2);
        assert_eq!(clean.iloc(0).unwrap(), &Value::Int(1));
        assert_eq!(clean.iloc(1).unwrap(), &Value::Int(3));
    }

    #[test]
    fn test_series_apply() {
        let s = Series::new(vec![1.0, 2.0, 3.0]);
        let doubled = s.apply(|v| Value::Float(v.as_float().unwrap_or(0.0) * 2.0));
        assert_eq!(doubled.iloc(0).unwrap(), &Value::Float(2.0));
        assert_eq!(doubled.iloc(2).unwrap(), &Value::Float(6.0));
    }

    #[test]
    fn test_series_cumsum() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0]);
        let result = s.cumsum();
        assert_eq!(result.iloc(0).unwrap(), &Value::Float(1.0));
        assert_eq!(result.iloc(1).unwrap(), &Value::Float(3.0));
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(6.0));
        assert_eq!(result.iloc(3).unwrap(), &Value::Float(10.0));
    }

    #[test]
    fn test_series_cumprod() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0]);
        let result = s.cumprod();
        assert_eq!(result.iloc(0).unwrap(), &Value::Float(1.0));
        assert_eq!(result.iloc(1).unwrap(), &Value::Float(2.0));
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(6.0));
        assert_eq!(result.iloc(3).unwrap(), &Value::Float(24.0));
    }

    #[test]
    fn test_series_shift() {
        let s = Series::new(vec![1, 2, 3, 4, 5]);
        let shifted = s.shift(2);
        assert!(shifted.iloc(0).unwrap().is_null());
        assert!(shifted.iloc(1).unwrap().is_null());
        assert_eq!(shifted.iloc(2).unwrap(), &Value::Int(1));
        assert_eq!(shifted.iloc(4).unwrap(), &Value::Int(3));
    }

    #[test]
    fn test_series_diff() {
        let s = Series::new(vec![1.0, 3.0, 6.0, 10.0]);
        let result = s.diff(1);
        assert!(result.iloc(0).unwrap().is_null());
        assert_eq!(result.iloc(1).unwrap(), &Value::Float(2.0));
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(3.0));
        assert_eq!(result.iloc(3).unwrap(), &Value::Float(4.0));
    }

    #[test]
    fn test_series_pct_change() {
        let s = Series::new(vec![100.0, 110.0, 121.0]);
        let result = s.pct_change(1);
        assert!(result.iloc(0).unwrap().is_null());
        assert!(f64::abs(result.iloc(1).unwrap().as_float().unwrap() - 0.1) < 1e-10);
        assert!(f64::abs(result.iloc(2).unwrap().as_float().unwrap() - 0.1) < 1e-10);
    }

    #[test]
    fn test_series_nlargest_nsmallest() {
        let s = Series::new(vec![3, 1, 4, 1, 5, 9, 2, 6]);
        let largest = s.nlargest(3);
        assert_eq!(largest.len(), 3);
        assert_eq!(largest.iloc(0).unwrap(), &Value::Int(9));
        assert_eq!(largest.iloc(1).unwrap(), &Value::Int(6));
        assert_eq!(largest.iloc(2).unwrap(), &Value::Int(5));

        let smallest = s.nsmallest(3);
        assert_eq!(smallest.iloc(0).unwrap(), &Value::Int(1));
    }

    #[test]
    fn test_series_clip() {
        let s = Series::new(vec![1.0, 5.0, 10.0, 15.0]);
        let clipped = s.clip(3.0, 12.0);
        assert_eq!(clipped.iloc(0).unwrap(), &Value::Float(3.0));
        assert_eq!(clipped.iloc(1).unwrap(), &Value::Float(5.0));
        assert_eq!(clipped.iloc(2).unwrap(), &Value::Float(10.0));
        assert_eq!(clipped.iloc(3).unwrap(), &Value::Float(12.0));
    }

    #[test]
    fn test_series_std_var() {
        let s = Series::new(vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        let variance = s.var().unwrap();
        let std_dev = s.std().unwrap();
        assert!(f64::abs(variance - 4.571428571428571) < 1e-10);
        assert!(f64::abs(std_dev - 2.138089935299395) < 1e-10);
    }

    #[test]
    fn test_series_median() {
        let s = Series::new(vec![1.0, 3.0, 5.0, 7.0, 9.0]);
        assert_eq!(s.median().unwrap(), 5.0);

        let s2 = Series::new(vec![1.0, 3.0, 5.0, 7.0]);
        assert_eq!(s2.median().unwrap(), 4.0);
    }

    #[test]
    fn test_series_rank() {
        let s = Series::new(vec![3, 1, 4, 1, 5]);
        let ranked = s.rank();
        // Values: 3, 1, 4, 1, 5 -> sorted indices: 1, 3, 0, 2, 4
        // Ranks for original positions: 3, 1, 4, 2, 5
        assert_eq!(ranked.iloc(0).unwrap(), &Value::Float(3.0));
        assert_eq!(ranked.iloc(1).unwrap(), &Value::Float(1.0));
    }

    #[test]
    fn test_series_duplicated() {
        let s = Series::new(vec![1, 2, 2, 3, 3, 3]);
        let dups = s.duplicated("first");
        assert_eq!(dups, vec![false, false, true, false, true, true]);
    }

    #[test]
    fn test_series_drop_duplicates() {
        let s = Series::new(vec![1, 2, 2, 3, 3, 3]);
        let unique = s.drop_duplicates("first");
        assert_eq!(unique.len(), 3);
    }

    #[test]
    fn test_series_isin() {
        let s = Series::new(vec![1, 2, 3, 4, 5]);
        let mask = s.isin(&[Value::Int(2), Value::Int(4)]);
        assert_eq!(mask, vec![false, true, false, true, false]);
    }

    #[test]
    fn test_series_between() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let mask = s.between(2.0, 4.0);
        assert_eq!(mask, vec![false, true, true, true, false]);
    }

    #[test]
    fn test_series_replace() {
        let s = Series::new(vec![1, 2, 3, 2, 1]);
        let replaced = s.replace(&Value::Int(2), &Value::Int(99));
        assert_eq!(replaced.iloc(1).unwrap(), &Value::Int(99));
        assert_eq!(replaced.iloc(3).unwrap(), &Value::Int(99));
    }

    #[test]
    fn test_series_quantile() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(s.quantile(0.0).unwrap(), 1.0);
        assert_eq!(s.quantile(0.5).unwrap(), 3.0);
        assert_eq!(s.quantile(1.0).unwrap(), 5.0);
    }

    #[test]
    fn test_series_corr() {
        let x = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let y = Series::new(vec![2.0, 4.0, 6.0, 8.0, 10.0]);
        let corr = x.corr(&y).unwrap();
        assert!(f64::abs(corr - 1.0) < 1e-10); // Perfect correlation
    }

    // Phase 2 interpolation tests
    #[test]
    fn test_series_interpolate_linear() {
        let s = Series::new(vec![
            Value::Float(1.0),
            Value::Null,
            Value::Float(3.0),
            Value::Null,
            Value::Float(5.0),
        ]);
        let filled = s.interpolate();
        assert_eq!(filled.iloc(1).unwrap(), &Value::Float(2.0));
        assert_eq!(filled.iloc(3).unwrap(), &Value::Float(4.0));
    }

    #[test]
    fn test_series_interpolate_nearest() {
        let s = Series::new(vec![
            Value::Float(1.0),
            Value::Null,
            Value::Null,
            Value::Float(10.0),
        ]);
        let filled = s.interpolate_method("nearest");
        // Position 1 is closer to 0 (1.0), position 2 is closer to 3 (10.0)
        assert!(filled.iloc(1).unwrap().as_float().is_some());
        assert!(filled.iloc(2).unwrap().as_float().is_some());
    }

    #[test]
    fn test_series_null_count() {
        let s = Series::new(vec![Value::Int(1), Value::Null, Value::Int(3), Value::Null]);
        assert_eq!(s.null_count(), 2);
        assert!(s.has_nulls());
    }

    #[test]
    fn test_series_no_nulls() {
        let s = Series::new(vec![1, 2, 3]);
        assert_eq!(s.null_count(), 0);
        assert!(!s.has_nulls());
    }

    #[test]
    fn test_series_cummax() {
        let s = Series::new(vec![1.0, 3.0, 2.0, 5.0, 4.0]);
        let result = s.cummax();
        assert_eq!(result.iloc(0).unwrap(), &Value::Float(1.0));
        assert_eq!(result.iloc(1).unwrap(), &Value::Float(3.0));
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(3.0));
        assert_eq!(result.iloc(3).unwrap(), &Value::Float(5.0));
        assert_eq!(result.iloc(4).unwrap(), &Value::Float(5.0));
    }

    #[test]
    fn test_series_cummin() {
        let s = Series::new(vec![5.0, 3.0, 4.0, 1.0, 2.0]);
        let result = s.cummin();
        assert_eq!(result.iloc(0).unwrap(), &Value::Float(5.0));
        assert_eq!(result.iloc(1).unwrap(), &Value::Float(3.0));
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(3.0));
        assert_eq!(result.iloc(3).unwrap(), &Value::Float(1.0));
        assert_eq!(result.iloc(4).unwrap(), &Value::Float(1.0));
    }

    #[test]
    fn test_series_value_counts() {
        let s = Series::new(vec!["a", "b", "a", "c", "a", "b"]);
        let counts = s.value_counts();
        assert_eq!(counts.get(&Value::String("a".into())), Some(&3));
        assert_eq!(counts.get(&Value::String("b".into())), Some(&2));
        assert_eq!(counts.get(&Value::String("c".into())), Some(&1));
    }

    #[test]
    fn test_series_notna() {
        let s = Series::new(vec![Value::Int(1), Value::Null, Value::Int(3)]);
        let result = s.notna();
        assert_eq!(result, vec![true, false, true]);
    }

    #[test]
    fn test_series_values() {
        let s = Series::new(vec![1, 2, 3]);
        let vals = s.values();
        assert_eq!(vals.len(), 3);
        assert_eq!(vals[0], Value::Int(1));
    }

    #[test]
    fn test_series_to_f64() {
        let s = Series::new(vec![1.0, 2.0, 3.0]);
        let vals = s.to_f64().unwrap();
        assert_eq!(vals, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_series_to_i64() {
        let s = Series::new(vec![1, 2, 3]);
        let vals = s.to_i64().unwrap();
        assert_eq!(vals, vec![1, 2, 3]);
    }

    #[test]
    fn test_series_iloc_many() {
        let s = Series::new(vec![10, 20, 30, 40, 50]);
        let result = s.iloc_many(&[0, 2, 4]).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.iloc(0).unwrap(), &Value::Int(10));
        assert_eq!(result.iloc(1).unwrap(), &Value::Int(30));
        assert_eq!(result.iloc(2).unwrap(), &Value::Int(50));
    }

    #[test]
    fn test_series_set_name() {
        let mut s = Series::new(vec![1, 2, 3]);
        s.set_name("my_series");
        assert_eq!(s.name(), Some("my_series"));
    }

    #[test]
    fn test_series_with_index() {
        let s = Series::with_index(
            vec![1, 2, 3],
            Index::String(vec!["a".into(), "b".into(), "c".into()]),
        )
        .unwrap();
        assert_eq!(s.index().len(), 3);
    }

    #[test]
    fn test_series_astype() {
        let s = Series::new(vec![1, 2, 3]);
        let floats = s.astype("float");
        assert_eq!(floats.iloc(0).unwrap(), &Value::Float(1.0));
    }

    #[test]
    fn test_series_round() {
        let s = Series::new(vec![1.234, 2.567, 3.891]);
        let rounded = s.round(1);
        assert_eq!(rounded.iloc(0).unwrap(), &Value::Float(1.2));
        assert_eq!(rounded.iloc(1).unwrap(), &Value::Float(2.6));
    }

    #[test]
    fn test_series_mask() {
        let s = Series::new(vec![1, 2, 3, 4, 5]);
        let masked = s.mask(&[false, true, false, true, false], &Value::Int(0));
        assert_eq!(masked.iloc(0).unwrap(), &Value::Int(1));
        assert_eq!(masked.iloc(1).unwrap(), &Value::Int(0));
        assert_eq!(masked.iloc(2).unwrap(), &Value::Int(3));
        assert_eq!(masked.iloc(3).unwrap(), &Value::Int(0));
    }

    #[test]
    fn test_series_where_cond() {
        let s = Series::new(vec![1, 2, 3, 4, 5]);
        let result = s.where_cond(&[true, false, true, false, true], &Value::Int(-1));
        assert_eq!(result.iloc(0).unwrap(), &Value::Int(1));
        assert_eq!(result.iloc(1).unwrap(), &Value::Int(-1));
        assert_eq!(result.iloc(2).unwrap(), &Value::Int(3));
    }

    #[test]
    fn test_series_replace_map() {
        let s = Series::new(vec![1, 2, 3, 2, 1]);
        let mut replacements = std::collections::HashMap::new();
        replacements.insert(Value::Int(1), Value::Int(10));
        replacements.insert(Value::Int(2), Value::Int(20));
        let result = s.replace_map(&replacements);
        assert_eq!(result.iloc(0).unwrap(), &Value::Int(10));
        assert_eq!(result.iloc(1).unwrap(), &Value::Int(20));
        assert_eq!(result.iloc(2).unwrap(), &Value::Int(3)); // unchanged
    }

    #[test]
    fn test_series_rolling() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let result = s.rolling(3).mean();
        // First two values should be null (not enough data)
        assert!(result.iloc(0).unwrap().is_null());
        assert!(result.iloc(1).unwrap().is_null());
        // Third value: mean(1, 2, 3) = 2.0
        assert_eq!(result.iloc(2).unwrap(), &Value::Float(2.0));
    }

    #[test]
    fn test_series_min_periods() {
        let s = Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let result = s.rolling(3).min_periods(2).mean();
        // With min_periods=2, we should get results starting from index 1
        assert!(result.iloc(0).unwrap().is_null()); // Only 1 value
        assert!(!result.iloc(1).unwrap().is_null()); // 2 values >= min_periods
    }

    #[test]
    fn test_series_loc() {
        let s = Series::with_index(
            vec![10, 20, 30],
            Index::String(vec!["a".into(), "b".into(), "c".into()]),
        )
        .unwrap();

        let val = s.loc(&Value::String("b".into())).unwrap();
        assert_eq!(val, &Value::Int(20));
    }

    #[test]
    fn test_series_reset_index() {
        let mut s = Series::with_index(
            vec![10, 20, 30],
            Index::String(vec!["a".into(), "b".into(), "c".into()]),
        )
        .unwrap();

        s.reset_index();
        assert_eq!(s.index().len(), 3);
        // Index should now be Range type
    }
}
