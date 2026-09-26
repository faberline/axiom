//! Multi-Index (hierarchical indexing) for DataFrame.
//!
//! Provides a MultiIndex type that supports multiple levels of indexing,
//! level-based selection, and operations like swaplevel and droplevel.

use super::error::{FrameError, Result};
use super::value::Value;
use std::collections::HashMap;

/// A hierarchical index with multiple levels.
#[derive(Debug, Clone)]
pub struct MultiIndex {
    /// Level names.
    names: Vec<String>,
    /// Level data: levels[level_idx][row_idx] = Value.
    levels: Vec<Vec<Value>>,
    /// Number of rows.
    length: usize,
}

impl MultiIndex {
    /// Create a MultiIndex from level data.
    ///
    /// Each level is a (name, values) pair. All levels must have the same length.
    pub fn new(levels: Vec<(&str, Vec<Value>)>) -> Result<Self> {
        if levels.is_empty() {
            return Ok(Self {
                names: Vec::new(),
                levels: Vec::new(),
                length: 0,
            });
        }

        let length = levels[0].1.len();
        for (name, vals) in &levels {
            if vals.len() != length {
                return Err(FrameError::ShapeMismatch {
                    expected: length,
                    actual: vals.len(),
                });
            }
            let _ = name; // Suppress unused variable warning
        }

        let names: Vec<String> = levels.iter().map(|(n, _)| n.to_string()).collect();
        let level_data: Vec<Vec<Value>> = levels.into_iter().map(|(_, v)| v).collect();

        Ok(Self {
            names,
            levels: level_data,
            length,
        })
    }

    /// Number of levels.
    pub fn nlevels(&self) -> usize {
        self.levels.len()
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.length
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Get level names.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Get values at a specific level.
    pub fn get_level_values(&self, level: usize) -> Result<&[Value]> {
        self.levels
            .get(level)
            .map(|v| v.as_slice())
            .ok_or_else(|| FrameError::IndexOutOfBounds {
                index: level,
                length: self.nlevels(),
            })
    }

    /// Get level index by name.
    pub fn get_level_index(&self, name: &str) -> Result<usize> {
        self.names
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| FrameError::LabelNotFound(name.to_string()))
    }

    /// Get values at a specific level by name.
    pub fn get_level_values_by_name(&self, name: &str) -> Result<&[Value]> {
        let idx = self.get_level_index(name)?;
        self.get_level_values(idx)
    }

    /// Get the full tuple of values at a row position.
    pub fn get_tuple(&self, pos: usize) -> Result<Vec<Value>> {
        if pos >= self.length {
            return Err(FrameError::IndexOutOfBounds {
                index: pos,
                length: self.length,
            });
        }
        Ok(self.levels.iter().map(|level| level[pos].clone()).collect())
    }

    /// Find row positions matching a partial key.
    ///
    /// The key can match any prefix of the index levels.
    pub fn get_locs(&self, key: &[Value]) -> Vec<usize> {
        let match_levels = key.len().min(self.nlevels());
        (0..self.length)
            .filter(|&i| (0..match_levels).all(|level| self.levels[level][i] == key[level]))
            .collect()
    }

    /// Select rows by a single level value.
    pub fn select_level(&self, level: usize, value: &Value) -> Vec<usize> {
        if level >= self.nlevels() {
            return Vec::new();
        }
        (0..self.length)
            .filter(|&i| &self.levels[level][i] == value)
            .collect()
    }

    /// Swap two levels.
    pub fn swaplevel(&self, level_a: usize, level_b: usize) -> Result<Self> {
        if level_a >= self.nlevels() || level_b >= self.nlevels() {
            return Err(FrameError::IndexOutOfBounds {
                index: level_a.max(level_b),
                length: self.nlevels(),
            });
        }

        let mut new_names = self.names.clone();
        let mut new_levels = self.levels.clone();
        new_names.swap(level_a, level_b);
        new_levels.swap(level_a, level_b);

        Ok(Self {
            names: new_names,
            levels: new_levels,
            length: self.length,
        })
    }

    /// Drop a level, returning a new MultiIndex.
    pub fn droplevel(&self, level: usize) -> Result<Self> {
        if level >= self.nlevels() {
            return Err(FrameError::IndexOutOfBounds {
                index: level,
                length: self.nlevels(),
            });
        }
        if self.nlevels() <= 1 {
            return Err(FrameError::InvalidOperation(
                "cannot drop the last level".into(),
            ));
        }

        let mut new_names = self.names.clone();
        let mut new_levels = self.levels.clone();
        new_names.remove(level);
        new_levels.remove(level);

        Ok(Self {
            names: new_names,
            levels: new_levels,
            length: self.length,
        })
    }

    /// Reorder levels.
    pub fn reorder_levels(&self, order: &[usize]) -> Result<Self> {
        if order.len() != self.nlevels() {
            return Err(FrameError::InvalidOperation(
                "order must have same length as number of levels".into(),
            ));
        }
        for &idx in order {
            if idx >= self.nlevels() {
                return Err(FrameError::IndexOutOfBounds {
                    index: idx,
                    length: self.nlevels(),
                });
            }
        }

        let new_names: Vec<String> = order.iter().map(|&i| self.names[i].clone()).collect();
        let new_levels: Vec<Vec<Value>> = order.iter().map(|&i| self.levels[i].clone()).collect();

        Ok(Self {
            names: new_names,
            levels: new_levels,
            length: self.length,
        })
    }

    /// Get unique tuples.
    pub fn unique(&self) -> Vec<Vec<Value>> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for i in 0..self.length {
            let tuple: Vec<Value> = self.levels.iter().map(|l| l[i].clone()).collect();
            if seen.insert(tuple.clone()) {
                result.push(tuple);
            }
        }
        result
    }

    /// Slice by row positions.
    pub fn slice(&self, positions: &[usize]) -> Result<Self> {
        for &pos in positions {
            if pos >= self.length {
                return Err(FrameError::IndexOutOfBounds {
                    index: pos,
                    length: self.length,
                });
            }
        }

        let new_levels: Vec<Vec<Value>> = self
            .levels
            .iter()
            .map(|level| positions.iter().map(|&i| level[i].clone()).collect())
            .collect();

        Ok(Self {
            names: self.names.clone(),
            levels: new_levels,
            length: positions.len(),
        })
    }

    /// Convert to a map: tuple -> list of row positions.
    pub fn to_group_map(&self) -> HashMap<Vec<Value>, Vec<usize>> {
        let mut map: HashMap<Vec<Value>, Vec<usize>> = HashMap::new();
        for i in 0..self.length {
            let key: Vec<Value> = self.levels.iter().map(|l| l[i].clone()).collect();
            map.entry(key).or_default().push(i);
        }
        map
    }

    /// Sort the multi-index by levels (returns new indices order).
    pub fn argsort(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.length).collect();
        indices.sort_by(|&a, &b| {
            for level in &self.levels {
                let ord = level[a].cmp(&level[b]);
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            std::cmp::Ordering::Equal
        });
        indices
    }
}

impl Default for MultiIndex {
    fn default() -> Self {
        Self {
            names: Vec::new(),
            levels: Vec::new(),
            length: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_multi_index() -> MultiIndex {
        MultiIndex::new(vec![
            (
                "region",
                vec![
                    Value::String("east".into()),
                    Value::String("east".into()),
                    Value::String("west".into()),
                    Value::String("west".into()),
                ],
            ),
            (
                "product",
                vec![
                    Value::String("a".into()),
                    Value::String("b".into()),
                    Value::String("a".into()),
                    Value::String("b".into()),
                ],
            ),
        ])
        .unwrap()
    }

    #[test]
    fn test_multi_index_basic() {
        let mi = create_test_multi_index();
        assert_eq!(mi.nlevels(), 2);
        assert_eq!(mi.len(), 4);
        assert_eq!(mi.names(), &["region", "product"]);
    }

    #[test]
    fn test_get_level_values() {
        let mi = create_test_multi_index();
        let vals = mi.get_level_values(0).unwrap();
        assert_eq!(vals[0], Value::String("east".into()));
        assert_eq!(vals[2], Value::String("west".into()));
    }

    #[test]
    fn test_get_level_values_by_name() {
        let mi = create_test_multi_index();
        let vals = mi.get_level_values_by_name("product").unwrap();
        assert_eq!(vals[0], Value::String("a".into()));
        assert_eq!(vals[1], Value::String("b".into()));
    }

    #[test]
    fn test_get_tuple() {
        let mi = create_test_multi_index();
        let t = mi.get_tuple(1).unwrap();
        assert_eq!(
            t,
            vec![Value::String("east".into()), Value::String("b".into()),]
        );
    }

    #[test]
    fn test_get_locs() {
        let mi = create_test_multi_index();
        let locs = mi.get_locs(&[Value::String("east".into())]);
        assert_eq!(locs, vec![0, 1]);

        let locs = mi.get_locs(&[Value::String("west".into()), Value::String("b".into())]);
        assert_eq!(locs, vec![3]);
    }

    #[test]
    fn test_select_level() {
        let mi = create_test_multi_index();
        let rows = mi.select_level(0, &Value::String("east".into()));
        assert_eq!(rows, vec![0, 1]);
    }

    #[test]
    fn test_swaplevel() {
        let mi = create_test_multi_index();
        let swapped = mi.swaplevel(0, 1).unwrap();
        assert_eq!(swapped.names(), &["product", "region"]);
        let t = swapped.get_tuple(0).unwrap();
        assert_eq!(t[0], Value::String("a".into()));
        assert_eq!(t[1], Value::String("east".into()));
    }

    #[test]
    fn test_droplevel() {
        let mi = create_test_multi_index();
        let dropped = mi.droplevel(1).unwrap();
        assert_eq!(dropped.nlevels(), 1);
        assert_eq!(dropped.names(), &["region"]);
    }

    #[test]
    fn test_droplevel_last_fails() {
        let mi = MultiIndex::new(vec![("x", vec![Value::Int(1), Value::Int(2)])]).unwrap();
        assert!(mi.droplevel(0).is_err());
    }

    #[test]
    fn test_reorder_levels() {
        let mi = create_test_multi_index();
        let reordered = mi.reorder_levels(&[1, 0]).unwrap();
        assert_eq!(reordered.names(), &["product", "region"]);
    }

    #[test]
    fn test_unique() {
        let mi = MultiIndex::new(vec![
            ("a", vec![Value::Int(1), Value::Int(1), Value::Int(2)]),
            ("b", vec![Value::Int(10), Value::Int(10), Value::Int(20)]),
        ])
        .unwrap();

        let unique = mi.unique();
        assert_eq!(unique.len(), 2);
    }

    #[test]
    fn test_slice() {
        let mi = create_test_multi_index();
        let sliced = mi.slice(&[0, 3]).unwrap();
        assert_eq!(sliced.len(), 2);
        assert_eq!(
            sliced.get_tuple(0).unwrap(),
            vec![Value::String("east".into()), Value::String("a".into())]
        );
        assert_eq!(
            sliced.get_tuple(1).unwrap(),
            vec![Value::String("west".into()), Value::String("b".into())]
        );
    }

    #[test]
    fn test_to_group_map() {
        let mi = MultiIndex::new(vec![
            ("a", vec![Value::Int(1), Value::Int(1), Value::Int(2)]),
            ("b", vec![Value::Int(10), Value::Int(20), Value::Int(10)]),
        ])
        .unwrap();

        let map = mi.to_group_map();
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn test_argsort() {
        let mi = MultiIndex::new(vec![
            ("a", vec![Value::Int(2), Value::Int(1), Value::Int(1)]),
            ("b", vec![Value::Int(1), Value::Int(2), Value::Int(1)]),
        ])
        .unwrap();

        let order = mi.argsort();
        assert_eq!(order, vec![2, 1, 0]);
    }

    #[test]
    fn test_empty_multi_index() {
        let mi = MultiIndex::new(vec![]).unwrap();
        assert!(mi.is_empty());
        assert_eq!(mi.nlevels(), 0);
    }
}
