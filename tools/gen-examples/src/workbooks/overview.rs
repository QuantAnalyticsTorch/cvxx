//! `00-overview.xlsx`: a short, brand-forward landing sheet with no
//! `CVX.*` formulas of its own, listing every other generated workbook.

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::error::GenError;
use crate::scenario::Sheet;

use super::WORKBOOKS;

pub fn build(path: &Path) -> Result<(), GenError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("cvxx")?;

    let mut sheet = Sheet::new(worksheet)?;
    sheet.title("cvxx: Convex Optimization in Excel")?;
    sheet.narrative(&[
        "cvxx brings convex optimization to Excel: build parameters, variables, \
         expressions, constraints, and problems with CVX.* worksheet functions, \
         then solve them and read back the answer without leaving the \
         spreadsheet.",
        "Each workbook below is a self-contained, narrated example covering one \
         area of cvxx. Open cvxx.xlam's Examples gallery to jump straight to \
         any of them, or open the files directly from docs/examples/.",
        "These workbooks contain live CVX.* formulas: open them with cvxx.xlam \
         and cvxx.xll loaded so Excel recalculates every formula on open. If a \
         result still shows 0, press Ctrl+Alt+F9 to force a full recalculation.",
    ])?;
    sheet.freeze_here()?;
    sheet.blank();
    sheet.two_col_header("Workbook", "What it shows")?;
    for (file_name, scenario) in WORKBOOKS {
        sheet.toc_row(file_name, scenario)?;
    }

    workbook.save(path)?;
    Ok(())
}
