//! Zero-copy viewport data bridge for efficient JS/WASM communication.
//!
//! This module provides structures and methods for transferring viewport data
//! from Rust to JavaScript without JSON serialization overhead. JavaScript
//! can directly read Rust memory through typed arrays.

/// Packed cell format flags (4 bytes / u32)
///
/// Bit layout:
/// - Bit 0: bold
/// - Bit 1: italic
/// - Bit 2: underline
/// - Bits 3-7: reserved
/// - Bits 8-15: font_size (0 = default)
/// - Bits 16-18: horizontal_align (0=left, 1=center, 2=right)
/// - Bits 19-21: vertical_align (0=middle, 1=top, 2=bottom)
/// - Bits 22-31: reserved for future use
#[inline]
pub fn pack_format(
    bold: bool,
    italic: bool,
    underline: bool,
    font_size: Option<u8>,
    h_align: u8,
    v_align: u8,
) -> u32 {
    let mut flags: u32 = 0;
    if bold {
        flags |= 1 << 0;
    }
    if italic {
        flags |= 1 << 1;
    }
    if underline {
        flags |= 1 << 2;
    }
    flags |= (font_size.unwrap_or(0) as u32) << 8;
    flags |= (h_align as u32 & 0x7) << 16;
    flags |= (v_align as u32 & 0x7) << 19;
    flags
}

/// Unpack format flags back to components
#[inline]
#[allow(dead_code)]
pub fn unpack_format(flags: u32) -> (bool, bool, bool, u8, u8, u8) {
    let bold = (flags & (1 << 0)) != 0;
    let italic = (flags & (1 << 1)) != 0;
    let underline = (flags & (1 << 2)) != 0;
    let font_size = ((flags >> 8) & 0xFF) as u8;
    let h_align = ((flags >> 16) & 0x7) as u8;
    let v_align = ((flags >> 19) & 0x7) as u8;
    (bold, italic, underline, font_size, h_align, v_align)
}

/// Viewport buffer for zero-copy data transfer.
///
/// Stores cell data in flat arrays that can be directly accessed
/// from JavaScript via typed arrays.
#[derive(Default)]
pub struct ViewportBuffer {
    /// Row indices for each cell
    pub rows: Vec<u32>,
    /// Column indices for each cell
    pub cols: Vec<u32>,
    /// Numeric values (f64::NAN for non-numeric cells)
    pub values: Vec<f64>,
    /// Packed format flags
    pub formats: Vec<u32>,
    /// Display strings (still need JSON for strings)
    pub display_values: Vec<String>,
}

#[allow(dead_code)]
impl ViewportBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            rows: Vec::with_capacity(capacity),
            cols: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
            formats: Vec::with_capacity(capacity),
            display_values: Vec::with_capacity(capacity),
        }
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.cols.clear();
        self.values.clear();
        self.formats.clear();
        self.display_values.clear();
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Push a cell into the buffer
    pub fn push(
        &mut self,
        row: u32,
        col: u32,
        numeric_value: f64,
        format_flags: u32,
        display: String,
    ) {
        self.rows.push(row);
        self.cols.push(col);
        self.values.push(numeric_value);
        self.formats.push(format_flags);
        self.display_values.push(display);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_format() {
        let flags = pack_format(true, false, true, Some(14), 1, 2);
        let (bold, italic, underline, font_size, h_align, v_align) = unpack_format(flags);

        assert!(bold);
        assert!(!italic);
        assert!(underline);
        assert_eq!(font_size, 14);
        assert_eq!(h_align, 1);
        assert_eq!(v_align, 2);
    }

    #[test]
    fn test_viewport_buffer() {
        let mut buf = ViewportBuffer::new();
        assert!(buf.is_empty());

        buf.push(0, 0, 42.0, 0, "42".to_string());
        buf.push(1, 1, f64::NAN, 1, "text".to_string());

        assert_eq!(buf.len(), 2);
        assert_eq!(buf.rows, vec![0, 1]);
        assert_eq!(buf.cols, vec![0, 1]);
        assert_eq!(buf.values[0], 42.0);
        assert!(buf.values[1].is_nan());
    }
}
