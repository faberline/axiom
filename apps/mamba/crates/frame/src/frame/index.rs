//! Index types for DataFrame rows and columns.

use super::error::{FrameError, Result};
use super::value::Value;
use std::collections::HashMap;

/// Index type - can be integer-based or label-based.
#[derive(Debug, Clone)]
pub enum Index {
    /// Range-based integer index (0..n).
    Range(usize),
    /// Explicit integer labels.
    Int(Vec<i64>),
    /// String labels.
    String(Vec<String>),
}

impl Index {
    /// Create a range index from 0 to n.
    pub fn range(n: usize) -> Self {
        Index::Range(n)
    }

    /// Create an integer index.
    pub fn int(labels: Vec<i64>) -> Self {
        Index::Int(labels)
    }

    /// Create a string index.
    pub fn string(labels: Vec<String>) -> Self {
        Index::String(labels)
    }

    /// Get the length of the index.
    pub fn len(&self) -> usize {
        match self {
            Index::Range(n) => *n,
            Index::Int(v) => v.len(),
            Index::String(v) => v.len(),
        }
    }

    /// Check if index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get position of a label in the index.
    pub fn get_loc(&self, label: &Value) -> Result<usize> {
        match (self, label) {
            (Index::Range(n), Value::Int(i)) => {
                let idx = *i as usize;
                if idx < *n {
                    Ok(idx)
                } else {
                    Err(FrameError::IndexOutOfBounds {
                        index: idx,
                        length: *n,
                    })
                }
            }
            (Index::Int(labels), Value::Int(target)) => labels
                .iter()
                .position(|x| x == target)
                .ok_or_else(|| FrameError::LabelNotFound(target.to_string())),
            (Index::String(labels), Value::String(target)) => labels
                .iter()
                .position(|x| x == target)
                .ok_or_else(|| FrameError::LabelNotFound(target.clone())),
            _ => Err(FrameError::TypeMismatch(format!(
                "index type mismatch: {:?} vs {:?}",
                self, label
            ))),
        }
    }

    /// Get label at position.
    pub fn get_label(&self, pos: usize) -> Result<Value> {
        if pos >= self.len() {
            return Err(FrameError::IndexOutOfBounds {
                index: pos,
                length: self.len(),
            });
        }

        Ok(match self {
            Index::Range(_) => Value::Int(pos as i64),
            Index::Int(v) => Value::Int(v[pos]),
            Index::String(v) => Value::String(v[pos].clone()),
        })
    }

    /// Get all labels as Values.
    pub fn to_values(&self) -> Vec<Value> {
        match self {
            Index::Range(n) => (0..*n).map(|i| Value::Int(i as i64)).collect(),
            Index::Int(v) => v.iter().map(|&i| Value::Int(i)).collect(),
            Index::String(v) => v.iter().map(|s| Value::String(s.clone())).collect(),
        }
    }

    /// Create a mapping from labels to positions.
    pub fn to_map(&self) -> HashMap<Value, usize> {
        self.to_values()
            .into_iter()
            .enumerate()
            .map(|(i, v)| (v, i))
            .collect()
    }

    /// Slice the index by positions.
    pub fn slice(&self, positions: &[usize]) -> Result<Self> {
        for &pos in positions {
            if pos >= self.len() {
                return Err(FrameError::IndexOutOfBounds {
                    index: pos,
                    length: self.len(),
                });
            }
        }

        Ok(match self {
            Index::Range(_) => {
                // Convert to explicit int index
                Index::Int(positions.iter().map(|&i| i as i64).collect())
            }
            Index::Int(v) => Index::Int(positions.iter().map(|&i| v[i]).collect()),
            Index::String(v) => Index::String(positions.iter().map(|&i| v[i].clone()).collect()),
        })
    }

    /// Reset to a range index.
    pub fn reset(&self) -> Self {
        Index::Range(self.len())
    }
}

impl Default for Index {
    fn default() -> Self {
        Index::Range(0)
    }
}

impl From<usize> for Index {
    fn from(n: usize) -> Self {
        Index::Range(n)
    }
}

impl From<Vec<i64>> for Index {
    fn from(v: Vec<i64>) -> Self {
        Index::Int(v)
    }
}

impl From<Vec<String>> for Index {
    fn from(v: Vec<String>) -> Self {
        Index::String(v)
    }
}

impl From<Vec<&str>> for Index {
    fn from(v: Vec<&str>) -> Self {
        Index::String(v.into_iter().map(String::from).collect())
    }
}

#[cfg(test)]
mod tests {
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
