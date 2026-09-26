//! Morton encoding (Z-order curve) for 2D coordinate mapping
//!
//! Maps 2D coordinates (row, col) to 1D keys while preserving spatial locality.
//! This enables efficient range queries on the underlying KV store.

use serde::{Deserialize, Serialize};

/// Morton-encoded key for 2D coordinates
///
/// Uses Z-order curve to map (row, col) pairs to a single u64 key.
/// This encoding preserves spatial locality, making range queries efficient.
///
/// # Example
///
/// ```rust
/// use jet_grid::db::storage::MortonKey;
///
/// let key = MortonKey::encode(10, 20);
/// let (row, col) = key.decode();
/// assert_eq!((row, col), (10, 20));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MortonKey(u64);

impl MortonKey {
    /// Encode 2D coordinates into a Morton key
    ///
    /// # Arguments
    ///
    /// * `row` - Row coordinate (0-2^32)
    /// * `col` - Column coordinate (0-2^32)
    ///
    /// # Returns
    ///
    /// Morton-encoded key that preserves spatial locality
    #[inline]
    pub fn encode(row: u32, col: u32) -> Self {
        Self(interleave_bits(row, col))
    }

    /// Decode Morton key back to 2D coordinates
    ///
    /// # Returns
    ///
    /// Tuple of (row, col)
    #[inline]
    pub fn decode(&self) -> (u32, u32) {
        deinterleave_bits(self.0)
    }

    /// Get the raw u64 value
    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// Create from raw u64 value
    #[inline]
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }

    /// Calculate Morton key range for a rectangular region
    ///
    /// Due to the Z-curve nature, a rectangle may span multiple disjoint
    /// Morton ranges. This function computes a minimal set of ranges that
    /// cover the rectangle.
    ///
    /// # Arguments
    ///
    /// * `start_row` - Starting row (inclusive)
    /// * `start_col` - Starting column (inclusive)
    /// * `end_row` - Ending row (inclusive)
    /// * `end_col` - Ending column (inclusive)
    ///
    /// # Returns
    ///
    /// Vector of (start_key, end_key) ranges that cover the rectangle
    pub fn range_for_rect(
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Vec<(MortonKey, MortonKey)> {
        // For small rectangles or when exact ranges aren't critical,
        // use a simple bounding approach: min corner to max corner
        let min_key = Self::encode(start_row, start_col);
        let max_key = Self::encode(end_row, end_col);

        // For larger rectangles, we could use BIGMIN/LITMAX algorithm
        // to compute tighter ranges, but for typical spreadsheet use cases
        // (viewport queries), the simple approach with post-filtering works well
        vec![(min_key, max_key)]
    }

    /// Check if a point is within a rectangular region
    #[inline]
    pub fn is_in_rect(&self, start_row: u32, start_col: u32, end_row: u32, end_col: u32) -> bool {
        let (row, col) = self.decode();
        row >= start_row && row <= end_row && col >= start_col && col <= end_col
    }
}

impl std::fmt::Display for MortonKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (row, col) = self.decode();
        write!(f, "MortonKey({}, {}) = 0x{:016x}", row, col, self.0)
    }
}

/// Spread bits of a 32-bit value to occupy even bit positions in 64-bit result
/// Example: 0b1111 -> 0b01010101
#[inline]
fn spread_bits(x: u32) -> u64 {
    let mut bits = x as u64;
    // Spread 32 bits into 64 bits using magic numbers
    // Each step doubles the spacing between bits
    bits = (bits | (bits << 16)) & 0x0000_FFFF_0000_FFFF;
    bits = (bits | (bits << 8)) & 0x00FF_00FF_00FF_00FF;
    bits = (bits | (bits << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    bits = (bits | (bits << 2)) & 0x3333_3333_3333_3333;
    bits = (bits | (bits << 1)) & 0x5555_5555_5555_5555;
    bits
}

/// Compact bits from even positions of a 64-bit value to 32-bit result
/// Example: 0b01010101 -> 0b1111
#[inline]
fn compact_bits(mut x: u64) -> u32 {
    // Extract even bits and compact them
    x &= 0x5555_5555_5555_5555;
    x = (x | (x >> 1)) & 0x3333_3333_3333_3333;
    x = (x | (x >> 2)) & 0x0F0F_0F0F_0F0F_0F0F;
    x = (x | (x >> 4)) & 0x00FF_00FF_00FF_00FF;
    x = (x | (x >> 8)) & 0x0000_FFFF_0000_FFFF;
    x = (x | (x >> 16)) & 0x0000_0000_FFFF_FFFF;
    x as u32
}

/// Interleave bits of two u32 values into a u64
/// Row bits go to even positions, col bits go to odd positions
#[inline]
fn interleave_bits(row: u32, col: u32) -> u64 {
    spread_bits(row) | (spread_bits(col) << 1)
}

/// Deinterleave bits of a u64 into two u32 values
#[inline]
fn deinterleave_bits(morton: u64) -> (u32, u32) {
    let row = compact_bits(morton);
    let col = compact_bits(morton >> 1);
    (row, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        // Test various coordinate pairs
        let test_cases = [
            (0, 0),
            (1, 0),
            (0, 1),
            (1, 1),
            (10, 20),
            (100, 200),
            (1000, 2000),
            (u32::MAX, u32::MAX),
            (0, u32::MAX),
            (u32::MAX, 0),
        ];

        for (row, col) in test_cases {
            let key = MortonKey::encode(row, col);
            let (decoded_row, decoded_col) = key.decode();
            assert_eq!(
                (decoded_row, decoded_col),
                (row, col),
                "Round-trip failed for ({}, {})",
                row,
                col
            );
        }
    }

    #[test]
    fn test_spatial_locality() {
        // Adjacent cells should have close Morton keys
        let key_00 = MortonKey::encode(0, 0);
        let key_01 = MortonKey::encode(0, 1);
        let key_10 = MortonKey::encode(1, 0);
        let key_11 = MortonKey::encode(1, 1);

        // All should be within the first few values
        assert!(key_00.as_u64() < 4);
        assert!(key_01.as_u64() < 4);
        assert!(key_10.as_u64() < 4);
        assert!(key_11.as_u64() < 4);
    }

    #[test]
    fn test_known_values() {
        // Known Morton encoding values
        // (0,0) -> 0b00 = 0
        assert_eq!(MortonKey::encode(0, 0).as_u64(), 0);
        // (1,0) -> 0b01 = 1 (row bit in even position)
        assert_eq!(MortonKey::encode(1, 0).as_u64(), 1);
        // (0,1) -> 0b10 = 2 (col bit in odd position)
        assert_eq!(MortonKey::encode(0, 1).as_u64(), 2);
        // (1,1) -> 0b11 = 3
        assert_eq!(MortonKey::encode(1, 1).as_u64(), 3);
        // (2,0) -> 0b0100 = 4
        assert_eq!(MortonKey::encode(2, 0).as_u64(), 4);
        // (0,2) -> 0b1000 = 8
        assert_eq!(MortonKey::encode(0, 2).as_u64(), 8);
    }

    #[test]
    fn test_is_in_rect() {
        let key = MortonKey::encode(5, 5);

        assert!(key.is_in_rect(0, 0, 10, 10));
        assert!(key.is_in_rect(5, 5, 5, 5));
        assert!(!key.is_in_rect(0, 0, 4, 4));
        assert!(!key.is_in_rect(6, 6, 10, 10));
    }

    #[test]
    fn test_range_for_rect() {
        let ranges = MortonKey::range_for_rect(0, 0, 10, 10);

        assert!(!ranges.is_empty());
        let (min, max) = ranges[0];
        assert!(min.as_u64() <= max.as_u64());
    }

    #[test]
    fn test_ordering() {
        // Morton keys should be orderable
        let keys: Vec<MortonKey> = (0..10)
            .flat_map(|r| (0..10).map(move |c| MortonKey::encode(r, c)))
            .collect();

        let mut sorted = keys.clone();
        sorted.sort();

        // Should maintain Z-order curve ordering
        assert_eq!(sorted[0], MortonKey::encode(0, 0));
    }

    #[test]
    fn test_display() {
        let key = MortonKey::encode(10, 20);
        let display = format!("{}", key);
        assert!(display.contains("10"));
        assert!(display.contains("20"));
    }
}
