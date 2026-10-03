//! Shared layout helpers implementing SPEC-0017's "Per-workbook content
//! contract": every generated workbook sheet is a title row, a narrative
//! block, zero or more labeled input/formula/result rows, built with the
//! shared [`crate::theme::Theme`] formats so every workbook looks the same.

use rust_xlsxwriter::{Worksheet, XlsxError};

use crate::theme::{self, Theme};

/// A thin cursor over a single worksheet that writes one themed row at a
/// time and tracks the next free row.
pub struct Sheet<'a> {
    ws: &'a mut Worksheet,
    row: u32,
}

impl<'a> Sheet<'a> {
    /// Wraps a freshly added worksheet, applying the shared column widths
    /// used by every generated workbook (SPEC-0017's theme table).
    pub fn new(ws: &'a mut Worksheet) -> Result<Self, XlsxError> {
        ws.set_column_width(theme::LABEL_COL, theme::LABEL_COLUMN_WIDTH)?;
        for col in theme::VALUE_COL..theme::NOTE_COL {
            ws.set_column_width(col, theme::VALUE_COLUMN_WIDTH)?;
        }
        ws.set_column_width(theme::NOTE_COL, theme::NOTE_COLUMN_WIDTH)?;
        Ok(Sheet { ws, row: 0 })
    }

    /// Writes the workbook title, merged across every used column.
    pub fn title(&mut self, text: &str) -> Result<&mut Self, XlsxError> {
        self.ws.set_row_height(self.row, theme::TITLE_ROW_HEIGHT)?;
        self.ws.merge_range(
            self.row,
            theme::LABEL_COL,
            self.row,
            theme::COLUMN_COUNT - 1,
            text,
            &Theme::title(),
        )?;
        self.row += 1;
        Ok(self)
    }

    /// Writes a wrapped narrative block (one merged, wrapped row per
    /// paragraph) explaining the business scenario in plain language.
    pub fn narrative(&mut self, paragraphs: &[&str]) -> Result<&mut Self, XlsxError> {
        for paragraph in paragraphs {
            self.ws
                .set_row_height(self.row, theme::NARRATIVE_ROW_HEIGHT)?;
            self.ws.merge_range(
                self.row,
                theme::LABEL_COL,
                self.row,
                theme::COLUMN_COUNT - 1,
                paragraph,
                &Theme::narrative(),
            )?;
            self.row += 1;
        }
        Ok(self)
    }

    /// Freezes panes just above the current row, so the title and narrative
    /// stay visible while scrolling through the formula walkthrough below.
    pub fn freeze_here(&mut self) -> Result<&mut Self, XlsxError> {
        self.ws.set_freeze_panes(self.row, 0)?;
        Ok(self)
    }

    /// Leaves one blank (unformatted) row as visual separation between
    /// sections.
    pub fn blank(&mut self) -> &mut Self {
        self.row += 1;
        self
    }

    /// Writes a three-column section header (e.g. "Known inputs" /
    /// "Value" / "Notes").
    pub fn section(
        &mut self,
        label: &str,
        value: &str,
        note: &str,
    ) -> Result<&mut Self, XlsxError> {
        let header = Theme::header();
        self.ws
            .write_string_with_format(self.row, theme::LABEL_COL, label, &header)?;
        self.ws.merge_range(
            self.row,
            theme::VALUE_COL,
            self.row,
            theme::NOTE_COL - 1,
            value,
            &header,
        )?;
        self.ws
            .write_string_with_format(self.row, theme::NOTE_COL, note, &header)?;
        self.row += 1;
        Ok(self)
    }

    /// Writes one labeled scalar input (pale blue) with a trailing note.
    /// Returns the A1 address of the value cell, for use in later formulas.
    pub fn input_scalar(
        &mut self,
        label: &str,
        value: f64,
        note: &str,
    ) -> Result<String, XlsxError> {
        self.write_label_and_note(label, note)?;
        self.ws
            .write_number_with_format(self.row, theme::VALUE_COL, value, &Theme::input())?;
        let addr = theme::cell_addr(self.row, theme::VALUE_COL);
        self.row += 1;
        Ok(addr)
    }

    /// Writes a labeled column vector of scalar inputs, one value per row
    /// starting at the value column. Returns the A1 range covering every
    /// value, for use as a `CVX.PARAMETER`/`CVX.VARIABLE` range argument.
    pub fn input_vector(
        &mut self,
        label: &str,
        values: &[f64],
        note: &str,
    ) -> Result<String, XlsxError> {
        let start_row = self.row;
        let input_fmt = Theme::input();
        for (i, value) in values.iter().enumerate() {
            if i == 0 {
                self.write_label_and_note(label, note)?;
            } else {
                self.row += 1;
            }
            self.ws
                .write_number_with_format(self.row, theme::VALUE_COL, *value, &input_fmt)?;
        }
        let addr = theme::range_addr(start_row, theme::VALUE_COL, self.row, theme::VALUE_COL);
        self.row += 1;
        Ok(addr)
    }

    /// Writes a labeled rectangular grid of scalar inputs (a small matrix),
    /// one spreadsheet row per matrix row, starting at the value column.
    /// Returns the A1 range covering every value.
    pub fn input_matrix(
        &mut self,
        label: &str,
        rows: &[Vec<f64>],
        note: &str,
    ) -> Result<String, XlsxError> {
        let start_row = self.row;
        let input_fmt = Theme::input();
        for (i, matrix_row) in rows.iter().enumerate() {
            if i == 0 {
                self.write_label_and_note(label, note)?;
            } else {
                self.row += 1;
            }
            for (j, value) in matrix_row.iter().enumerate() {
                self.ws.write_number_with_format(
                    self.row,
                    theme::VALUE_COL + j as u16,
                    *value,
                    &input_fmt,
                )?;
            }
        }
        let last_col =
            theme::VALUE_COL + rows.first().map_or(0, |r| r.len() as u16).saturating_sub(1);
        let addr = theme::range_addr(start_row, theme::VALUE_COL, self.row, last_col);
        self.row += 1;
        Ok(addr)
    }

    /// Writes one step of the formula walkthrough: a label, the `CVX.*`
    /// formula itself (formula-styled), and a one-line explanation. Returns
    /// the A1 address of the formula cell so later steps can reference the
    /// handle it returns.
    pub fn formula_row(
        &mut self,
        label: &str,
        formula: &str,
        note: &str,
    ) -> Result<String, XlsxError> {
        self.write_label(label)?;
        self.ws.write_formula_with_format(
            self.row,
            theme::VALUE_COL,
            formula,
            &Theme::formula(),
        )?;
        self.ws
            .write_string_with_format(self.row, theme::NOTE_COL, note, &Theme::narrative())?;
        let addr = theme::cell_addr(self.row, theme::VALUE_COL);
        self.row += 1;
        Ok(addr)
    }

    /// Writes one row of the final result/answer block.
    pub fn result_row(
        &mut self,
        label: &str,
        formula: &str,
        note: &str,
    ) -> Result<String, XlsxError> {
        self.write_label(label)?;
        self.ws
            .write_formula_with_format(self.row, theme::VALUE_COL, formula, &Theme::result())?;
        self.ws
            .write_string_with_format(self.row, theme::NOTE_COL, note, &Theme::narrative())?;
        let addr = theme::cell_addr(self.row, theme::VALUE_COL);
        self.row += 1;
        Ok(addr)
    }

    /// Writes a two-column section header: a label header and a single
    /// header merged across the remaining columns. Used by `00-overview.xlsx`'s
    /// table of contents, which has no separate "notes" zone.
    pub fn two_col_header(&mut self, label: &str, rest: &str) -> Result<&mut Self, XlsxError> {
        let header = Theme::header();
        self.ws
            .write_string_with_format(self.row, theme::LABEL_COL, label, &header)?;
        self.ws.merge_range(
            self.row,
            theme::VALUE_COL,
            self.row,
            theme::NOTE_COL,
            rest,
            &header,
        )?;
        self.row += 1;
        Ok(self)
    }

    /// Writes one table-of-contents row for `00-overview.xlsx`: a file name
    /// and its one-line scenario description.
    pub fn toc_row(&mut self, file_name: &str, scenario: &str) -> Result<&mut Self, XlsxError> {
        let label_fmt = Theme::label();
        self.ws
            .write_string_with_format(self.row, theme::LABEL_COL, file_name, &label_fmt)?;
        self.ws.merge_range(
            self.row,
            theme::VALUE_COL,
            self.row,
            theme::NOTE_COL,
            scenario,
            &Theme::narrative(),
        )?;
        self.row += 1;
        Ok(self)
    }

    fn write_label(&mut self, label: &str) -> Result<(), XlsxError> {
        self.ws
            .write_string_with_format(self.row, theme::LABEL_COL, label, &Theme::label())?;
        Ok(())
    }

    fn write_label_and_note(&mut self, label: &str, note: &str) -> Result<(), XlsxError> {
        self.write_label(label)?;
        self.ws
            .write_string_with_format(self.row, theme::NOTE_COL, note, &Theme::narrative())?;
        Ok(())
    }
}
