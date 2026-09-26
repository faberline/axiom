use serde::{Deserialize, Serialize};

/// RGBA color representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_alpha")]
    pub a: u8,
}

fn default_alpha() -> u8 {
    255
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    /// Convert to CSS hex color string
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Parse from CSS hex color string
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Color::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Color::rgba(r, g, b, a))
            }
            _ => None,
        }
    }

    // Common colors
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 255, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

/// Horizontal text alignment
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HorizontalAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical text alignment
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

/// Border line style
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BorderType {
    #[default]
    None,
    Solid,
    Dashed,
    Dotted,
    Thick,
    Double,
    Hair,
}

/// Single border style with type and color
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorderStyle {
    pub style: BorderType,
    pub color: Color,
}

impl BorderStyle {
    pub fn new(style: BorderType, color: Color) -> Self {
        Self { style, color }
    }

    pub fn solid(color: Color) -> Self {
        Self::new(BorderType::Solid, color)
    }

    pub fn thick(color: Color) -> Self {
        Self::new(BorderType::Thick, color)
    }

    pub fn dashed(color: Color) -> Self {
        Self::new(BorderType::Dashed, color)
    }
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            style: BorderType::None,
            color: Color::BLACK,
        }
    }
}

/// Cell borders on all four sides
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellBorders {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<BorderStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<BorderStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<BorderStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<BorderStyle>,
}

impl CellBorders {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_top(mut self, style: BorderStyle) -> Self {
        self.top = Some(style);
        self
    }

    pub fn with_bottom(mut self, style: BorderStyle) -> Self {
        self.bottom = Some(style);
        self
    }

    pub fn with_left(mut self, style: BorderStyle) -> Self {
        self.left = Some(style);
        self
    }

    pub fn with_right(mut self, style: BorderStyle) -> Self {
        self.right = Some(style);
        self
    }

    /// Apply the same border to all sides
    pub fn all(style: BorderStyle) -> Self {
        Self {
            top: Some(style),
            bottom: Some(style),
            left: Some(style),
            right: Some(style),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.top.is_none() && self.bottom.is_none() && self.left.is_none() && self.right.is_none()
    }
}

/// Pattern fill type
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatternType {
    #[default]
    None,
    Solid,
    Gray125,
    LightGray,
    MediumGray,
    DarkGray,
    DarkVertical,
    DarkHorizontal,
    DarkDown,
    DarkUp,
    DarkGrid,
    DarkTrellis,
    LightVertical,
    LightHorizontal,
    LightDown,
    LightUp,
    LightGrid,
    LightTrellis,
}

/// Pattern fill with foreground and background colors
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillPattern {
    pub pattern_type: PatternType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<Color>,
}

impl FillPattern {
    pub fn solid(color: Color) -> Self {
        Self {
            pattern_type: PatternType::Solid,
            foreground_color: Some(color),
            background_color: None,
        }
    }

    pub fn pattern(pattern_type: PatternType, fg: Color, bg: Color) -> Self {
        Self {
            pattern_type,
            foreground_color: Some(fg),
            background_color: Some(bg),
        }
    }
}

/// Cell formatting properties
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CellFormat {
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub strikethrough: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_pattern: Option<FillPattern>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borders: Option<CellBorders>,
    #[serde(default, skip_serializing_if = "is_default_h_align")]
    pub horizontal_align: HorizontalAlign,
    #[serde(default, skip_serializing_if = "is_default_v_align")]
    pub vertical_align: VerticalAlign,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub wrap_text: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_default_h_align(a: &HorizontalAlign) -> bool {
    *a == HorizontalAlign::default()
}

fn is_default_v_align(a: &VerticalAlign) -> bool {
    *a == VerticalAlign::default()
}

impl CellFormat {
    /// Create a new format with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder pattern: set bold
    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// Builder pattern: set italic
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    /// Builder pattern: set text color
    pub fn with_text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Builder pattern: set background color
    pub fn with_background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Builder pattern: set horizontal alignment
    pub fn with_horizontal_align(mut self, align: HorizontalAlign) -> Self {
        self.horizontal_align = align;
        self
    }

    /// Builder pattern: set vertical alignment
    pub fn with_vertical_align(mut self, align: VerticalAlign) -> Self {
        self.vertical_align = align;
        self
    }

    /// Builder pattern: set font size
    pub fn with_font_size(mut self, size: u8) -> Self {
        self.font_size = Some(size);
        self
    }

    /// Builder pattern: set borders
    pub fn with_borders(mut self, borders: CellBorders) -> Self {
        self.borders = Some(borders);
        self
    }

    /// Builder pattern: set fill pattern
    pub fn with_fill_pattern(mut self, pattern: FillPattern) -> Self {
        self.fill_pattern = Some(pattern);
        self
    }

    /// Get the effective font size (default is 11)
    pub fn effective_font_size(&self) -> u8 {
        self.font_size.unwrap_or(11)
    }

    /// Get the effective font family (default is Arial)
    pub fn effective_font_family(&self) -> &str {
        self.font_family.as_deref().unwrap_or("Arial")
    }

    /// Merge another format into this one (other's values override)
    pub fn merge(&mut self, other: &CellFormat) {
        if other.bold {
            self.bold = true;
        }
        if other.italic {
            self.italic = true;
        }
        if other.underline {
            self.underline = true;
        }
        if other.strikethrough {
            self.strikethrough = true;
        }
        if other.font_size.is_some() {
            self.font_size = other.font_size;
        }
        if other.font_family.is_some() {
            self.font_family = other.font_family.clone();
        }
        if other.text_color.is_some() {
            self.text_color = other.text_color;
        }
        if other.background_color.is_some() {
            self.background_color = other.background_color;
        }
        if other.fill_pattern.is_some() {
            self.fill_pattern = other.fill_pattern;
        }
        if other.borders.is_some() {
            self.borders = other.borders;
        }
        if other.horizontal_align != HorizontalAlign::default() {
            self.horizontal_align = other.horizontal_align;
        }
        if other.vertical_align != VerticalAlign::default() {
            self.vertical_align = other.vertical_align;
        }
        if other.number_format.is_some() {
            self.number_format = other.number_format.clone();
        }
        if other.wrap_text {
            self.wrap_text = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_hex() {
        let color = Color::rgb(255, 128, 64);
        assert_eq!(color.to_hex(), "#ff8040");

        let parsed = Color::from_hex("#ff8040").unwrap();
        assert_eq!(parsed, color);
    }

    #[test]
    fn test_format_builder() {
        let format = CellFormat::new()
            .with_bold(true)
            .with_text_color(Color::RED)
            .with_font_size(14);

        assert!(format.bold);
        assert_eq!(format.text_color, Some(Color::RED));
        assert_eq!(format.effective_font_size(), 14);
    }

    #[test]
    fn test_borders() {
        let borders = CellBorders::new()
            .with_top(BorderStyle::thick(Color::RED))
            .with_bottom(BorderStyle::solid(Color::BLUE));

        assert!(borders.top.is_some());
        assert_eq!(borders.top.unwrap().style, BorderType::Thick);
        assert_eq!(borders.top.unwrap().color, Color::RED);
        assert!(borders.bottom.is_some());
        assert_eq!(borders.bottom.unwrap().style, BorderType::Solid);
        assert!(borders.left.is_none());

        // Test all borders
        let all_borders = CellBorders::all(BorderStyle::solid(Color::BLACK));
        assert!(!all_borders.is_empty());
        assert!(all_borders.top.is_some());
        assert!(all_borders.bottom.is_some());
        assert!(all_borders.left.is_some());
        assert!(all_borders.right.is_some());
    }

    #[test]
    fn test_fill_pattern() {
        let solid = FillPattern::solid(Color::GREEN);
        assert_eq!(solid.pattern_type, PatternType::Solid);
        assert_eq!(solid.foreground_color, Some(Color::GREEN));
        assert!(solid.background_color.is_none());

        let striped = FillPattern::pattern(PatternType::DarkVertical, Color::GREEN, Color::WHITE);
        assert_eq!(striped.pattern_type, PatternType::DarkVertical);
        assert_eq!(striped.foreground_color, Some(Color::GREEN));
        assert_eq!(striped.background_color, Some(Color::WHITE));
    }

    #[test]
    fn test_format_with_borders_and_fill() {
        let format = CellFormat::new()
            .with_bold(true)
            .with_borders(CellBorders::all(BorderStyle::solid(Color::BLACK)))
            .with_fill_pattern(FillPattern::solid(Color::rgb(255, 255, 200)));

        assert!(format.bold);
        assert!(format.borders.is_some());
        assert!(format.fill_pattern.is_some());
        assert_eq!(
            format.fill_pattern.unwrap().foreground_color,
            Some(Color::rgb(255, 255, 200))
        );
    }
}
