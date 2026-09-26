//! Integration tests for the frame module (pandas-like DataFrame).
//!
//! These tests were extracted from the inline `#[cfg(test)]` modules in the frame source files.

use frame::frame::io::{read_csv, write_csv};
use frame::frame::ops::JoinType;
use frame::frame::{DataFrame, Index, Series, Value};
use std::collections::HashMap;

// ============================================================================
// Value tests (from value.rs)
// ============================================================================

mod value_tests {
    use super::*;

    #[test]
    fn test_value_conversions() {
        assert_eq!(Value::from(42i32).as_int(), Some(42));
        assert_eq!(Value::from(3.14f64).as_float(), Some(3.14));
        assert_eq!(Value::from("hello").as_str(), Some("hello"));
        assert_eq!(Value::from(true).as_bool(), Some(true));
    }

    #[test]
    fn test_value_ordering() {
        let null = Value::Null;
        let int = Value::Int(10);
        let float = Value::Float(10.5);
        let string = Value::String("hello".to_string());

        assert!(null < int);
        assert!(int < float);
        assert!(float < string);
    }
}

// ============================================================================
// Index tests (from index.rs)
// ============================================================================

mod index_tests {
    use super::*;

    #[test]
    fn test_range_index() {
        let idx = Index::range(5);
        assert_eq!(idx.len(), 5);
        assert_eq!(idx.get_loc(&Value::Int(2)).unwrap(), 2);
        assert!(idx.get_loc(&Value::Int(10)).is_err());
    }

    #[test]
    fn test_int_index() {
        let idx = Index::int(vec![10, 20, 30]);
        assert_eq!(idx.len(), 3);
        assert_eq!(idx.get_loc(&Value::Int(20)).unwrap(), 1);
        assert!(idx.get_loc(&Value::Int(25)).is_err());
    }

    #[test]
    fn test_string_index() {
        let idx = Index::string(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(idx.len(), 3);
        assert_eq!(idx.get_loc(&Value::String("b".into())).unwrap(), 1);
        assert!(idx.get_loc(&Value::String("d".into())).is_err());
    }

    #[test]
    fn test_slice_index() {
        let idx = Index::string(vec!["a".into(), "b".into(), "c".into(), "d".into()]);
        let sliced = idx.slice(&[0, 2]).unwrap();
        assert_eq!(sliced.len(), 2);
        assert_eq!(sliced.get_label(0).unwrap(), Value::String("a".into()));
        assert_eq!(sliced.get_label(1).unwrap(), Value::String("c".into()));
    }
}

// ============================================================================
// Series tests (from series.rs)
// ============================================================================

mod series_tests {
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

// ============================================================================
// DataFrame tests (from dataframe.rs)
// ============================================================================

mod dataframe_tests {
    use super::*;

    fn create_test_df() -> DataFrame {
        DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2, 3, 4, 5])),
            ("b", Series::new(vec![10.0, 20.0, 30.0, 40.0, 50.0])),
            ("c", Series::new(vec!["x", "y", "z", "x", "y"])),
        ])
        .unwrap()
    }

    #[test]
    fn test_dataframe_basic() {
        let df = create_test_df();
        assert_eq!(df.shape(), (5, 3));
        assert_eq!(df.columns(), &["a", "b", "c"]);
    }

    #[test]
    fn test_dataframe_get_column() {
        let df = create_test_df();
        let col = df.get("a").unwrap();
        assert_eq!(col.len(), 5);
        assert_eq!(col.iloc(0).unwrap(), &Value::Int(1));
    }

    #[test]
    fn test_dataframe_iloc() {
        let df = create_test_df();
        let row = df.iloc_row(1).unwrap();
        assert_eq!(row.get("a").unwrap(), &Value::Int(2));
        assert_eq!(row.get("b").unwrap(), &Value::Float(20.0));
    }

    #[test]
    fn test_dataframe_filter() {
        let df = create_test_df();
        let mask = vec![true, false, true, false, true];
        let filtered = df.filter(&mask).unwrap();
        assert_eq!(filtered.nrows(), 3);
    }

    #[test]
    fn test_dataframe_sort() {
        let df = create_test_df();
        let sorted = df.sort_values("a", false).unwrap();
        assert_eq!(sorted.get("a").unwrap().iloc(0).unwrap(), &Value::Int(5));
    }

    #[test]
    fn test_dataframe_head_tail() {
        let df = create_test_df();
        let head = df.head(2).unwrap();
        assert_eq!(head.nrows(), 2);
        let tail = df.tail(2).unwrap();
        assert_eq!(tail.nrows(), 2);
        assert_eq!(tail.get("a").unwrap().iloc(0).unwrap(), &Value::Int(4));
    }

    #[test]
    fn test_dataframe_select() {
        let df = create_test_df();
        let selected = df.select(&["a", "c"]).unwrap();
        assert_eq!(selected.ncols(), 2);
        assert_eq!(selected.columns(), &["a", "c"]);
    }

    #[test]
    fn test_dataframe_isna() {
        let df = DataFrame::from_columns(vec![
            (
                "a",
                Series::new(vec![Value::Int(1), Value::Null, Value::Int(3)]),
            ),
            (
                "b",
                Series::new(vec![Value::Null, Value::Int(2), Value::Int(3)]),
            ),
        ])
        .unwrap();

        let isna = df.isna();
        assert_eq!(isna.get("a").unwrap(), &vec![false, true, false]);
        assert_eq!(isna.get("b").unwrap(), &vec![true, false, false]);
    }

    #[test]
    fn test_dataframe_isna_any() {
        let df = DataFrame::from_columns(vec![
            (
                "a",
                Series::new(vec![Value::Int(1), Value::Null, Value::Int(3)]),
            ),
            (
                "b",
                Series::new(vec![Value::Null, Value::Int(2), Value::Int(3)]),
            ),
        ])
        .unwrap();

        let mask = df.isna_any();
        assert_eq!(mask, vec![true, true, false]);
    }

    #[test]
    fn test_dataframe_fillna() {
        let df = DataFrame::from_columns(vec![
            (
                "a",
                Series::new(vec![Value::Int(1), Value::Null, Value::Int(3)]),
            ),
            (
                "b",
                Series::new(vec![Value::Float(1.0), Value::Null, Value::Float(3.0)]),
            ),
        ])
        .unwrap();

        let filled = df.fillna(Value::Int(0));
        assert_eq!(filled.get("a").unwrap().iloc(1).unwrap(), &Value::Int(0));
        assert_eq!(filled.get("b").unwrap().iloc(1).unwrap(), &Value::Int(0));
    }

    #[test]
    fn test_dataframe_dropna() {
        let df = DataFrame::from_columns(vec![
            (
                "a",
                Series::new(vec![Value::Int(1), Value::Null, Value::Int(3)]),
            ),
            (
                "b",
                Series::new(vec![Value::Int(1), Value::Int(2), Value::Null]),
            ),
        ])
        .unwrap();

        let clean = df.dropna().unwrap();
        assert_eq!(clean.nrows(), 1);
        assert_eq!(clean.get("a").unwrap().iloc(0).unwrap(), &Value::Int(1));
    }

    #[test]
    fn test_dataframe_apply() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1.0, 2.0, 3.0])),
            ("b", Series::new(vec![10.0, 20.0, 30.0])),
        ])
        .unwrap();

        let sums = df.apply(|col| col.sum().unwrap_or(0.0));
        assert_eq!(*sums.get("a").unwrap(), 6.0);
        assert_eq!(*sums.get("b").unwrap(), 60.0);
    }

    #[test]
    fn test_dataframe_applymap() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1.0, 2.0])),
            ("b", Series::new(vec![3.0, 4.0])),
        ])
        .unwrap();

        let doubled = df.applymap(|v| Value::Float(v.as_float().unwrap_or(0.0) * 2.0));
        assert_eq!(
            doubled.get("a").unwrap().iloc(0).unwrap(),
            &Value::Float(2.0)
        );
        assert_eq!(
            doubled.get("b").unwrap().iloc(1).unwrap(),
            &Value::Float(8.0)
        );
    }

    #[test]
    fn test_dataframe_concat() {
        let df1 = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2])),
            ("b", Series::new(vec![3, 4])),
        ])
        .unwrap();

        let df2 = DataFrame::from_columns(vec![
            ("a", Series::new(vec![5, 6])),
            ("b", Series::new(vec![7, 8])),
        ])
        .unwrap();

        let combined = DataFrame::concat(&[&df1, &df2]).unwrap();
        assert_eq!(combined.nrows(), 4);
        assert_eq!(combined.get("a").unwrap().iloc(2).unwrap(), &Value::Int(5));
    }

    #[test]
    fn test_dataframe_from_records() {
        let records = vec![
            {
                let mut m = HashMap::new();
                m.insert("a".to_string(), Value::Int(1));
                m.insert("b".to_string(), Value::Int(2));
                m
            },
            {
                let mut m = HashMap::new();
                m.insert("a".to_string(), Value::Int(3));
                m.insert("b".to_string(), Value::Int(4));
                m
            },
        ];

        let df = DataFrame::from_records(&records).unwrap();
        assert_eq!(df.nrows(), 2);
        assert_eq!(df.ncols(), 2);
    }

    #[test]
    fn test_dataframe_to_dict() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2])),
            ("b", Series::new(vec![3, 4])),
        ])
        .unwrap();

        let dict = df.to_dict();
        assert_eq!(dict.get("a").unwrap().len(), 2);
    }

    #[test]
    fn test_dataframe_duplicated() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 1, 2, 2])),
            ("b", Series::new(vec![1, 1, 2, 3])),
        ])
        .unwrap();

        let dups = df.duplicated(None, "first").unwrap();
        assert_eq!(dups, vec![false, true, false, false]);
    }

    #[test]
    fn test_dataframe_drop_duplicates() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 1, 2])),
            ("b", Series::new(vec![1, 1, 2])),
        ])
        .unwrap();

        let unique = df.drop_duplicates(None, "first").unwrap();
        assert_eq!(unique.nrows(), 2);
    }

    #[test]
    fn test_dataframe_corr() {
        let df = DataFrame::from_columns(vec![
            ("x", Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0])),
            ("y", Series::new(vec![2.0, 4.0, 6.0, 8.0, 10.0])),
        ])
        .unwrap();

        let corr = df.corr().unwrap();
        assert_eq!(corr.nrows(), 2);
        // Perfect correlation
        let x_y_corr = corr.get("y").unwrap().iloc(0).unwrap().as_float().unwrap();
        assert!(f64::abs(x_y_corr - 1.0) < 1e-10);
    }

    #[test]
    fn test_dataframe_melt() {
        let df = DataFrame::from_columns(vec![
            ("id", Series::new(vec!["a", "b"])),
            ("x", Series::new(vec![1, 2])),
            ("y", Series::new(vec![3, 4])),
        ])
        .unwrap();

        let melted = df.melt(&["id"], &["x", "y"]).unwrap();
        assert_eq!(melted.nrows(), 4);
        assert_eq!(melted.ncols(), 3); // id, variable, value
    }

    #[test]
    fn test_dataframe_sample() {
        let df = DataFrame::from_columns(vec![("a", Series::new(vec![1, 2, 3, 4, 5]))]).unwrap();

        let sample = df.sample(3).unwrap();
        assert_eq!(sample.nrows(), 3);
    }

    #[test]
    fn test_dataframe_assign() {
        let df = DataFrame::from_columns(vec![("a", Series::new(vec![1, 2, 3]))]).unwrap();

        let df2 = df.assign(vec![("b", Series::new(vec![4, 5, 6]))]).unwrap();
        assert_eq!(df2.ncols(), 2);
        assert!(df2.get("b").is_ok());
    }

    #[test]
    fn test_dataframe_describe() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1.0, 2.0, 3.0, 4.0, 5.0])),
            ("b", Series::new(vec![10.0, 20.0, 30.0, 40.0, 50.0])),
        ])
        .unwrap();

        let desc = df.describe();
        assert!(desc.contains_key("a"));
        assert!(desc.contains_key("b"));
        assert_eq!(desc.get("a").unwrap().get("count"), Some(&5.0));
        assert_eq!(desc.get("a").unwrap().get("mean"), Some(&3.0));
    }

    #[test]
    fn test_dataframe_query() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2, 3, 4, 5])),
            ("b", Series::new(vec![10, 20, 30, 40, 50])),
        ])
        .unwrap();

        let result = df
            .query(|row| row.get("a").and_then(|v| v.as_int()).unwrap_or(0) > 2)
            .unwrap();
        assert_eq!(result.nrows(), 3); // rows where a > 2
    }

    #[test]
    fn test_dataframe_sort_values_by() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![2, 1, 2, 1])),
            ("b", Series::new(vec![20, 10, 10, 20])),
        ])
        .unwrap();

        let sorted = df.sort_values_by(&["a", "b"], &[true, true]).unwrap();
        assert_eq!(sorted.nrows(), 4);
        // Should be sorted by a then b
    }

    #[test]
    fn test_dataframe_reset_index() {
        let mut df = DataFrame::from_columns(vec![("a", Series::new(vec![1, 2, 3]))]).unwrap();

        df.reset_index();
        // Index should be range
        assert_eq!(df.nrows(), 3);
    }

    #[test]
    fn test_dataframe_iterrows() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2])),
            ("b", Series::new(vec![3, 4])),
        ])
        .unwrap();

        let rows: Vec<_> = df.iterrows().collect();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].1.contains_key("a"));
        assert!(rows[0].1.contains_key("b"));
    }

    #[test]
    fn test_dataframe_isna_all() {
        let df = DataFrame::from_columns(vec![
            (
                "a",
                Series::new(vec![Value::Null, Value::Int(2), Value::Null]),
            ),
            (
                "b",
                Series::new(vec![Value::Null, Value::Int(3), Value::Int(4)]),
            ),
        ])
        .unwrap();

        let mask = df.isna_all();
        assert_eq!(mask, vec![true, false, false]); // Only first row has all nulls
    }

    #[test]
    fn test_dataframe_fillna_column() {
        let df = DataFrame::from_columns(vec![
            (
                "a",
                Series::new(vec![Value::Int(1), Value::Null, Value::Int(3)]),
            ),
            (
                "b",
                Series::new(vec![Value::Float(1.0), Value::Null, Value::Float(3.0)]),
            ),
        ])
        .unwrap();

        let filled = df.fillna_column("a", Value::Int(99)).unwrap();
        assert_eq!(filled.get("a").unwrap().iloc(1).unwrap(), &Value::Int(99));
        // b should still have null
        assert!(filled.get("b").unwrap().iloc(1).unwrap().is_null());
    }

    #[test]
    fn test_dataframe_dropna_subset() {
        let df = DataFrame::from_columns(vec![
            (
                "a",
                Series::new(vec![Value::Int(1), Value::Null, Value::Int(3)]),
            ),
            (
                "b",
                Series::new(vec![Value::Int(1), Value::Int(2), Value::Null]),
            ),
        ])
        .unwrap();

        let clean = df.dropna_subset(&["a"]).unwrap();
        assert_eq!(clean.nrows(), 2); // Only row with null in 'a' dropped
    }

    #[test]
    fn test_dataframe_col() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2, 3])),
            ("b", Series::new(vec![4, 5, 6])),
        ])
        .unwrap();

        let col_a = df.col("a").unwrap();
        assert_eq!(col_a.len(), 3);
    }

    #[test]
    fn test_dataframe_rename() {
        let mut df = DataFrame::from_columns(vec![("a", Series::new(vec![1, 2, 3]))]).unwrap();

        let mut mapping = HashMap::new();
        mapping.insert("a", "x");
        df.rename(mapping);
        assert!(df.get("x").is_ok());
        assert!(df.get("a").is_err());
    }

    #[test]
    fn test_dataframe_loc() {
        let mut df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2, 3])),
            ("b", Series::new(vec![4, 5, 6])),
        ])
        .unwrap();

        let _ = df.set_index(Index::String(vec!["x".into(), "y".into(), "z".into()]));
        let rows = df.loc(&[Value::String("y".into())]).unwrap();
        assert_eq!(rows.nrows(), 1);
        assert!(rows.get("a").is_ok());
    }

    #[test]
    fn test_dataframe_set_index() {
        let mut df = DataFrame::from_columns(vec![("a", Series::new(vec![1, 2, 3]))]).unwrap();

        let _ = df.set_index(Index::String(vec!["x".into(), "y".into(), "z".into()]));
        assert_eq!(df.nrows(), 3);
    }

    #[test]
    fn test_dataframe_to_records() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2])),
            ("b", Series::new(vec![3, 4])),
        ])
        .unwrap();

        let records = df.to_records();
        assert_eq!(records.len(), 2);
        assert!(records[0].contains_key("a"));
    }

    #[test]
    fn test_dataframe_pivot() {
        let df = DataFrame::from_columns(vec![
            ("row", Series::new(vec!["a", "a", "b", "b"])),
            ("col", Series::new(vec!["x", "y", "x", "y"])),
            ("val", Series::new(vec![1.0, 2.0, 3.0, 4.0])),
        ])
        .unwrap();

        let pivoted = df.pivot("row", "col", "val").unwrap();
        assert!(pivoted.ncols() >= 2); // At least row + some value columns
    }

    #[test]
    fn test_dataframe_from_map() {
        let mut data = HashMap::new();
        data.insert("a", vec![1, 2, 3]);
        data.insert("b", vec![4, 5, 6]);
        let df = DataFrame::from_map(data).unwrap();
        assert_eq!(df.nrows(), 3);
        assert_eq!(df.ncols(), 2);
    }

    #[test]
    fn test_dataframe_is_empty() {
        let df = DataFrame::from_columns(vec![]).unwrap();
        assert!(df.is_empty());

        let df2 = DataFrame::from_columns(vec![("a", Series::new(vec![1, 2, 3]))]).unwrap();
        assert!(!df2.is_empty());
    }

    #[test]
    fn test_dataframe_index() {
        let df = DataFrame::from_columns(vec![("a", Series::new(vec![1, 2, 3]))]).unwrap();
        assert_eq!(df.index().len(), 3);
    }

    #[test]
    fn test_dataframe_get_columns() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2, 3])),
            ("b", Series::new(vec![4, 5, 6])),
            ("c", Series::new(vec![7, 8, 9])),
        ])
        .unwrap();

        let subset = df.get_columns(&["a", "c"]).unwrap();
        assert_eq!(subset.ncols(), 2);
        assert!(subset.get("a").is_ok());
        assert!(subset.get("c").is_ok());
        assert!(subset.get("b").is_err());
    }

    #[test]
    fn test_dataframe_insert() {
        let mut df = DataFrame::from_columns(vec![("a", Series::new(vec![1, 2, 3]))]).unwrap();

        df.insert("b", Series::new(vec![4, 5, 6])).unwrap();
        assert_eq!(df.ncols(), 2);
        assert!(df.get("b").is_ok());
    }

    #[test]
    fn test_dataframe_drop() {
        let mut df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2, 3])),
            ("b", Series::new(vec![4, 5, 6])),
        ])
        .unwrap();

        let dropped = df.drop("b").unwrap();
        assert_eq!(dropped.len(), 3);
        assert_eq!(df.ncols(), 1);
        assert!(df.get("b").is_err());
    }
}

// ============================================================================
// GroupBy tests (from ops/groupby.rs)
// ============================================================================

mod groupby_tests {
    use super::*;

    fn create_test_df() -> DataFrame {
        DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "B", "A", "B", "A"])),
            ("value", Series::new(vec![10.0, 20.0, 30.0, 40.0, 50.0])),
        ])
        .unwrap()
    }

    #[test]
    fn test_groupby_sum() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().sum().unwrap();

        assert_eq!(result.nrows(), 2);
        // A: 10 + 30 + 50 = 90, B: 20 + 40 = 60
    }

    #[test]
    fn test_groupby_mean() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().mean().unwrap();

        assert_eq!(result.nrows(), 2);
        // A: (10 + 30 + 50) / 3 = 30, B: (20 + 40) / 2 = 30
    }

    #[test]
    fn test_groupby_count() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().count().unwrap();

        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_ngroups() {
        let df = create_test_df();
        let gb = df.groupby(&["category"]).unwrap();
        assert_eq!(gb.ngroups(), 2);
    }

    // Phase 2 GroupBy tests
    #[test]
    fn test_groupby_var() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("value", Series::new(vec![1.0, 3.0, 10.0, 20.0])),
        ])
        .unwrap();
        let result = df.groupby(&["category"]).unwrap().var().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_std() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("value", Series::new(vec![1.0, 3.0, 10.0, 20.0])),
        ])
        .unwrap();
        let result = df.groupby(&["category"]).unwrap().std().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_median() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "A", "B", "B"])),
            ("value", Series::new(vec![1.0, 2.0, 3.0, 10.0, 20.0])),
        ])
        .unwrap();
        let result = df.groupby(&["category"]).unwrap().median().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_transform() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("value", Series::new(vec![10.0, 20.0, 100.0, 200.0])),
        ])
        .unwrap();

        // Transform: demean within group
        let result = df
            .groupby(&["category"])
            .unwrap()
            .transform(|series, indices| {
                let vals: Vec<f64> = indices
                    .iter()
                    .filter_map(|&i| series.iloc(i).ok()?.as_float())
                    .collect();
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                indices
                    .iter()
                    .map(|&i| {
                        series
                            .iloc(i)
                            .ok()?
                            .as_float()
                            .map(|v| Value::Float(v - mean))
                    })
                    .collect()
            })
            .unwrap();

        assert_eq!(result.nrows(), 4);
    }

    #[test]
    fn test_groupby_filter() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("value", Series::new(vec![10.0, 20.0, 100.0, 200.0])),
        ])
        .unwrap();

        // Filter: keep groups with mean > 50
        let result = df
            .groupby(&["category"])
            .unwrap()
            .filter_groups(|df, indices| {
                let vals: Vec<f64> = indices
                    .iter()
                    .filter_map(|&i| df.get("value").ok()?.iloc(i).ok()?.as_float())
                    .collect();
                vals.iter().sum::<f64>() / vals.len() as f64 > 50.0
            })
            .unwrap();

        assert_eq!(result.nrows(), 2); // Only group B
    }

    #[test]
    fn test_groupby_min() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().min().unwrap();
        assert_eq!(result.nrows(), 2);
        // A: min(10, 30, 50) = 10, B: min(20, 40) = 20
    }

    #[test]
    fn test_groupby_max() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().max().unwrap();
        assert_eq!(result.nrows(), 2);
        // A: max(10, 30, 50) = 50, B: max(20, 40) = 40
    }

    #[test]
    fn test_groupby_first() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().first().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_last() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().last().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_groups() {
        let df = create_test_df();
        let gb = df.groupby(&["category"]).unwrap();
        let groups = gb.groups();
        assert_eq!(groups.len(), 2);
        // Should have keys for A and B
    }

    #[test]
    fn test_groupby_aggregate() {
        let df = create_test_df();
        let result = df
            .groupby(&["category"])
            .unwrap()
            .aggregate(|series, indices| {
                let sum: f64 = indices
                    .iter()
                    .filter_map(|&i| series.iloc(i).ok()?.as_float())
                    .sum();
                Value::Float(sum * 2.0) // Custom: double the sum
            })
            .unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_agg() {
        let df = create_test_df();
        let mut aggs = HashMap::new();
        aggs.insert("value", "sum");
        let result = df.groupby(&["category"]).unwrap().agg(aggs).unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_agg_multiple() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("x", Series::new(vec![1.0, 2.0, 3.0, 4.0])),
            ("y", Series::new(vec![10.0, 20.0, 30.0, 40.0])),
        ])
        .unwrap();
        let mut aggs = HashMap::new();
        aggs.insert("x", "mean");
        aggs.insert("y", "sum");
        let result = df.groupby(&["category"]).unwrap().agg(aggs).unwrap();
        assert_eq!(result.nrows(), 2);
    }
}

// ============================================================================
// Join tests (from ops/join.rs)
// ============================================================================

mod join_tests {
    use super::*;

    fn create_left_df() -> DataFrame {
        DataFrame::from_columns(vec![
            ("key", Series::new(vec!["a", "b", "c"])),
            ("left_val", Series::new(vec![1, 2, 3])),
        ])
        .unwrap()
    }

    fn create_right_df() -> DataFrame {
        DataFrame::from_columns(vec![
            ("key", Series::new(vec!["b", "c", "d"])),
            ("right_val", Series::new(vec![20, 30, 40])),
        ])
        .unwrap()
    }

    #[test]
    fn test_inner_join() {
        let left = create_left_df();
        let right = create_right_df();
        let result = left.join(&right, &["key"], JoinType::Inner).unwrap();

        assert_eq!(result.nrows(), 2); // b, c
        assert_eq!(result.ncols(), 3); // key, left_val, right_val_right
    }

    #[test]
    fn test_left_join() {
        let left = create_left_df();
        let right = create_right_df();
        let result = left.left_join(&right, &["key"]).unwrap();

        assert_eq!(result.nrows(), 3); // a, b, c
    }

    #[test]
    fn test_right_join() {
        let left = create_left_df();
        let right = create_right_df();
        let result = left.right_join(&right, &["key"]).unwrap();

        assert_eq!(result.nrows(), 3); // b, c, d
    }

    #[test]
    fn test_outer_join() {
        let left = create_left_df();
        let right = create_right_df();
        let result = left.outer_join(&right, &["key"]).unwrap();

        assert_eq!(result.nrows(), 4); // a, b, c, d
    }
}

// ============================================================================
// CSV I/O tests (from io/csv.rs)
// ============================================================================

mod csv_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn parse_csv_line(line: &str, delimiter: char, quote: char) -> Vec<String> {
        let mut fields = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            if c == quote {
                if in_quotes {
                    // Check for escaped quote
                    if chars.peek() == Some(&quote) {
                        current.push(quote);
                        chars.next();
                    } else {
                        in_quotes = false;
                    }
                } else {
                    in_quotes = true;
                }
            } else if c == delimiter && !in_quotes {
                fields.push(current.trim().to_string());
                current = String::new();
            } else {
                current.push(c);
            }
        }

        fields.push(current.trim().to_string());
        fields
    }

    fn parse_value(s: &str) -> Value {
        if s.is_empty() {
            return Value::Null;
        }

        // Try to parse as integer
        if let Ok(i) = s.parse::<i64>() {
            return Value::Int(i);
        }

        // Try to parse as float
        if let Ok(f) = s.parse::<f64>() {
            return Value::Float(f);
        }

        // Try to parse as boolean
        match s.to_lowercase().as_str() {
            "true" | "yes" | "1" => return Value::Bool(true),
            "false" | "no" | "0" => return Value::Bool(false),
            _ => {}
        }

        // Default to string
        Value::String(s.to_string())
    }

    #[test]
    fn test_parse_csv_line() {
        let line = "a,b,c";
        let fields = parse_csv_line(line, ',', '"');
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_csv_line_quoted() {
        let line = r#""hello, world",b,c"#;
        let fields = parse_csv_line(line, ',', '"');
        assert_eq!(fields, vec!["hello, world", "b", "c"]);
    }

    #[test]
    fn test_parse_value() {
        assert_eq!(parse_value("42"), Value::Int(42));
        assert_eq!(parse_value("3.14"), Value::Float(3.14));
        assert_eq!(parse_value("true"), Value::Bool(true));
        assert_eq!(parse_value("hello"), Value::String("hello".to_string()));
        assert_eq!(parse_value(""), Value::Null);
    }

    #[test]
    fn test_read_write_csv() {
        // Create test CSV
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "name,age,score").unwrap();
        writeln!(temp, "Alice,25,95.5").unwrap();
        writeln!(temp, "Bob,30,87.0").unwrap();
        temp.flush().unwrap();

        // Read it back
        let df = read_csv(temp.path()).unwrap();
        assert_eq!(df.shape(), (2, 3));
        assert_eq!(df.columns(), &["name", "age", "score"]);

        // Write to new file
        let output = NamedTempFile::new().unwrap();
        write_csv(&df, output.path()).unwrap();

        // Read again and verify
        let df2 = read_csv(output.path()).unwrap();
        assert_eq!(df2.shape(), (2, 3));
    }
}
