//! Shared blue/white visual theme used by every generated example workbook
//! (SPEC-0017). Defining the palette and the row-element formats in exactly
//! one place is what keeps the whole set of workbooks visually consistent.
//!
//! The six hex colors below are also recorded in `docs/architecture.md`
//! ("Example workbook and documentation color palette") so a future
//! `mdbook` custom-CSS pass can reuse the same palette for `docs/html`.

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder};

/// Dark blue used for the workbook title band.
pub const DARK_BLUE: Color = Color::RGB(0x1F4E78);
/// Mid blue used for table column headers.
pub const MID_BLUE: Color = Color::RGB(0x2E74B5);
/// Pale blue used for editable input cells.
pub const PALE_BLUE: Color = Color::RGB(0xDCE6F1);
/// Very pale blue used for the result/answer cells' fill.
pub const VERY_PALE_BLUE: Color = Color::RGB(0xEAF1FB);
/// Plain white used for narrative prose and formula cells.
pub const WHITE: Color = Color::White;

/// Default row height (Excel character units) for the title row.
pub const TITLE_ROW_HEIGHT: f64 = 30.0;
/// Default row height for wrapped narrative rows.
pub const NARRATIVE_ROW_HEIGHT: f64 = 34.0;

/// Column width (Excel character units) for the label column (A).
pub const LABEL_COLUMN_WIDTH: f64 = 34.0;
/// Column width for each value/formula column (B..E).
pub const VALUE_COLUMN_WIDTH: f64 = 18.0;
/// Column width for the trailing explanation/notes column.
pub const NOTE_COLUMN_WIDTH: f64 = 58.0;

/// Zero-based column index of the label column.
pub const LABEL_COL: u16 = 0;
/// Zero-based column index of the first value/formula column.
pub const VALUE_COL: u16 = 1;
/// Zero-based column index of the trailing explanation/notes column.
pub const NOTE_COL: u16 = 5;
/// Total number of columns used by the standard layout (A..F).
pub const COLUMN_COUNT: u16 = NOTE_COL + 1;

/// The shared blue/white format set. One method per row element in
/// SPEC-0017's theme table.
pub struct Theme;

impl Theme {
    /// Workbook title band: dark blue fill, bold white 16pt text.
    pub fn title() -> Format {
        Format::new()
            .set_background_color(DARK_BLUE)
            .set_font_color(Color::White)
            .set_bold()
            .set_font_size(16.0)
            .set_align(FormatAlign::Left)
            .set_align(FormatAlign::VerticalCenter)
    }

    /// Narrative/prose text: white background, black 11pt wrapped text.
    pub fn narrative() -> Format {
        Format::new()
            .set_background_color(WHITE)
            .set_font_color(Color::Black)
            .set_font_size(11.0)
            .set_text_wrap()
            .set_align(FormatAlign::Top)
    }

    /// Table column headers: mid blue fill, bold white 11pt text.
    pub fn header() -> Format {
        Format::new()
            .set_background_color(MID_BLUE)
            .set_font_color(Color::White)
            .set_bold()
            .set_font_size(11.0)
    }

    /// Row labels in column A: plain bold black text, no fill.
    pub fn label() -> Format {
        Format::new()
            .set_font_color(Color::Black)
            .set_bold()
            .set_font_size(11.0)
            .set_align(FormatAlign::Top)
    }

    /// Input cells (known numeric data): pale blue fill, black 11pt text.
    pub fn input() -> Format {
        Format::new()
            .set_background_color(PALE_BLUE)
            .set_font_color(Color::Black)
            .set_font_size(11.0)
    }

    /// Formula cells (`CVX.*` calls): white background, black 11pt
    /// monospace-style text so the formula reads clearly.
    pub fn formula() -> Format {
        Format::new()
            .set_background_color(WHITE)
            .set_font_color(Color::Black)
            .set_font_size(11.0)
            .set_font_name("Consolas")
    }

    /// Result/answer cells: very pale blue fill, bold black text, mid blue
    /// border.
    pub fn result() -> Format {
        Format::new()
            .set_background_color(VERY_PALE_BLUE)
            .set_font_color(Color::Black)
            .set_bold()
            .set_font_size(11.0)
            .set_border(FormatBorder::Thin)
            .set_border_color(MID_BLUE)
    }
}

/// Converts a zero-based column index (0..=25) to its Excel column letter.
pub fn col_letter(col: u16) -> char {
    debug_assert!(col <= 25, "column index out of the single-letter A-Z range");
    (b'A' + col as u8) as char
}

/// Builds an A1-style cell address from a zero-based row/column pair.
pub fn cell_addr(row: u32, col: u16) -> String {
    format!("{}{}", col_letter(col), row + 1)
}

/// Builds an A1-style range address from zero-based row/column pairs.
pub fn range_addr(row1: u32, col1: u16, row2: u32, col2: u16) -> String {
    format!("{}:{}", cell_addr(row1, col1), cell_addr(row2, col2))
}
