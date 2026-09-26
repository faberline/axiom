use serde::{Deserialize, Serialize};
use std::fmt;

/// Cell coordinate (0-indexed internally)
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellCoord {
    pub row: u32,
    pub col: u32,
}

impl CellCoord {
    pub const fn new(row: u32, col: u32) -> Self {
        CellCoord { row, col }
    }

    /// Create from A1 notation (e.g., "A1" -> (0, 0), "B2" -> (1, 1))
    pub fn from_a1(notation: &str) -> Option<Self> {
        let notation = notation.trim().to_uppercase();
        let mut col_str = String::new();
        let mut row_str = String::new();

        for c in notation.chars() {
            if c.is_ascii_alphabetic() {
                if !row_str.is_empty() {
                    return None; // Letters after numbers
                }
                col_str.push(c);
            } else if c.is_ascii_digit() {
                row_str.push(c);
            } else {
                return None; // Invalid character
            }
        }

        if col_str.is_empty() || row_str.is_empty() {
            return None;
        }

        let col = col_from_label(&col_str)?;
        let row: u32 = row_str.parse().ok()?;

        if row == 0 {
            return None; // Rows are 1-indexed in A1 notation
        }

        Some(CellCoord {
            row: row - 1, // Convert to 0-indexed
            col,
        })
    }

    /// Convert to A1 notation (e.g., (0, 0) -> "A1")
    pub fn to_a1(&self) -> String {
        format!("{}{}", col_to_label(self.col), self.row + 1)
    }

    /// Check if this coord is within bounds
    pub fn is_valid(&self, max_rows: u32, max_cols: u32) -> bool {
        self.row < max_rows && self.col < max_cols
    }
}

impl fmt::Display for CellCoord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_a1())
    }
}

/// Convert column index (0-indexed) to label (A, B, ..., Z, AA, AB, ...)
pub fn col_to_label(col: u32) -> String {
    let mut label = String::new();
    let mut n = col + 1; // 1-indexed for calculation

    while n > 0 {
        n -= 1;
        label.insert(0, char::from(b'A' + (n % 26) as u8));
        n /= 26;
    }

    label
}

/// Convert column label (A, B, ..., Z, AA, AB, ...) to index (0-indexed)
pub fn col_from_label(label: &str) -> Option<u32> {
    let mut col: u32 = 0;

    for c in label.chars() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        col = col * 26 + (c.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
    }

    if col == 0 {
        None
    } else {
        Some(col - 1) // Convert to 0-indexed
    }
}

/// A range of cells (e.g., A1:B10)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRange {
    pub start: CellCoord,
    pub end: CellCoord,
}

impl CellRange {
    pub fn new(start: CellCoord, end: CellCoord) -> Self {
        // Normalize so start is top-left and end is bottom-right
        CellRange {
            start: CellCoord::new(start.row.min(end.row), start.col.min(end.col)),
            end: CellCoord::new(start.row.max(end.row), start.col.max(end.col)),
        }
    }

    /// Create from A1:B1 notation
    pub fn from_a1(notation: &str) -> Option<Self> {
        let parts: Vec<&str> = notation.split(':').collect();
        match parts.len() {
            1 => {
                let coord = CellCoord::from_a1(parts[0])?;
                Some(CellRange::new(coord, coord))
            }
            2 => {
                let start = CellCoord::from_a1(parts[0])?;
                let end = CellCoord::from_a1(parts[1])?;
                Some(CellRange::new(start, end))
            }
            _ => None,
        }
    }

    /// Convert to A1:B1 notation
    pub fn to_a1(&self) -> String {
        if self.start == self.end {
            self.start.to_a1()
        } else {
            format!("{}:{}", self.start.to_a1(), self.end.to_a1())
        }
    }

    /// Check if a coordinate is within this range
    pub fn contains(&self, coord: CellCoord) -> bool {
        coord.row >= self.start.row
            && coord.row <= self.end.row
            && coord.col >= self.start.col
            && coord.col <= self.end.col
    }

    /// Get the number of rows in the range
    pub fn row_count(&self) -> u32 {
        self.end.row - self.start.row + 1
    }

    /// Get the number of columns in the range
    pub fn col_count(&self) -> u32 {
        self.end.col - self.start.col + 1
    }

    /// Get the total number of cells in the range
    pub fn cell_count(&self) -> u32 {
        self.row_count() * self.col_count()
    }

    /// Iterate over all coordinates in the range (row by row)
    pub fn iter(&self) -> CellRangeIter {
        CellRangeIter {
            range: *self,
            current_row: self.start.row,
            current_col: self.start.col,
        }
    }

    /// Check if this range intersects with another range
    pub fn intersects(&self, other: &CellRange) -> bool {
        !(self.end.row < other.start.row
            || self.start.row > other.end.row
            || self.end.col < other.start.col
            || self.start.col > other.end.col)
    }

    /// Check if this range is a single cell
    pub fn is_single_cell(&self) -> bool {
        self.start == self.end
    }

    /// Get the row span (number of rows)
    pub fn row_span(&self) -> u32 {
        self.end.row - self.start.row + 1
    }

    /// Get the column span (number of columns)
    pub fn col_span(&self) -> u32 {
        self.end.col - self.start.col + 1
    }
}

impl fmt::Display for CellRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_a1())
    }
}

impl IntoIterator for CellRange {
    type Item = CellCoord;
    type IntoIter = CellRangeIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over coordinates in a range
pub struct CellRangeIter {
    range: CellRange,
    current_row: u32,
    current_col: u32,
}

impl Iterator for CellRangeIter {
    type Item = CellCoord;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_row > self.range.end.row {
            return None;
        }

        let coord = CellCoord::new(self.current_row, self.current_col);

        self.current_col += 1;
        if self.current_col > self.range.end.col {
            self.current_col = self.range.start.col;
            self.current_row += 1;
        }

        Some(coord)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.range.cell_count() as usize;
        (count, Some(count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_to_label() {
        assert_eq!(col_to_label(0), "A");
        assert_eq!(col_to_label(25), "Z");
        assert_eq!(col_to_label(26), "AA");
        assert_eq!(col_to_label(27), "AB");
        assert_eq!(col_to_label(701), "ZZ");
        assert_eq!(col_to_label(702), "AAA");
    }

    #[test]
    fn test_col_from_label() {
        assert_eq!(col_from_label("A"), Some(0));
        assert_eq!(col_from_label("Z"), Some(25));
        assert_eq!(col_from_label("AA"), Some(26));
        assert_eq!(col_from_label("AB"), Some(27));
        assert_eq!(col_from_label("ZZ"), Some(701));
    }

    #[test]
    fn test_coord_a1() {
        let coord = CellCoord::from_a1("A1").unwrap();
        assert_eq!(coord, CellCoord::new(0, 0));

        let coord = CellCoord::from_a1("B2").unwrap();
        assert_eq!(coord, CellCoord::new(1, 1));

        let coord = CellCoord::from_a1("AA100").unwrap();
        assert_eq!(coord, CellCoord::new(99, 26));

        assert_eq!(coord.to_a1(), "AA100");
    }

    #[test]
    fn test_range_iteration() {
        let range = CellRange::from_a1("A1:B2").unwrap();
        let coords: Vec<_> = range.iter().collect();

        assert_eq!(coords.len(), 4);
        assert_eq!(coords[0], CellCoord::new(0, 0));
        assert_eq!(coords[1], CellCoord::new(0, 1));
        assert_eq!(coords[2], CellCoord::new(1, 0));
        assert_eq!(coords[3], CellCoord::new(1, 1));
    }
}
