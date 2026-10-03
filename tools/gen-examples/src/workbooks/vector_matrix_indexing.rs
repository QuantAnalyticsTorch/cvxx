//! `06-vector-matrix-indexing.xlsx`: selecting individual line items or
//! sub-ranges out of a multi-period budget matrix.

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::error::GenError;
use crate::scenario::Sheet;

pub fn build(path: &Path) -> Result<(), GenError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Indexing")?;

    let mut sheet = Sheet::new(worksheet)?;
    sheet.title("Multi-Period Budget: Vector and Matrix Indexing")?;
    sheet.narrative(&[
        "A small business plans its spending for three categories across four \
         quarters. Once that plan is stored as a single cvxx parameter \
         matrix, CVX.INDEX lets a model pull out a single line item, an \
         entire category's row, an entire quarter's column, or any other \
         rectangular sub-section, without re-entering the numbers.",
        "No variable or solving is needed for this workbook: CVX.INDEX works \
         directly on a parameter (or any expression), which is why it lives \
         alongside CVX.EXPRESSION's other functional builders in \
         docs/expressions.md.",
    ])?;
    sheet.freeze_here()?;
    sheet.blank();

    sheet.section("Quarterly budget ($000s)", "Q1 / Q2 / Q3 / Q4", "Notes")?;
    let budget = sheet.input_matrix(
        "Marketing / Payroll / Rent",
        &[
            vec![10.0, 12.0, 9.0, 15.0],
            vec![40.0, 40.0, 42.0, 42.0],
            vec![8.0, 8.0, 8.0, 8.0],
        ],
        "Row 1 = Marketing, row 2 = Payroll, row 3 = Rent; columns are Q1..Q4.",
    )?;

    sheet.blank();
    sheet.section("Step", "Formula", "What it does")?;
    let param = sheet.formula_row(
        "1. Store the budget matrix as a parameter",
        &format!("=CVX.PARAMETER({budget},\"budget\")"),
        "A single 3x4 parameter holding every category's spend for every quarter.",
    )?;
    let one_entry = sheet.formula_row(
        "2. Select a single entry (Rent, Q4)",
        &format!("=CVX.INDEX({param},3,4,1,1,\"rent_q4\")"),
        "Row 3 (Rent), column 4 (Q4): rows/cols both default to 1 for a single cell.",
    )?;
    let payroll_row = sheet.formula_row(
        "3. Select an entire row (Payroll, every quarter)",
        &format!("=CVX.INDEX({param},2,1,1,4,\"payroll_by_quarter\")"),
        "Row 2, starting at column 1, spanning all 4 columns: a 1x4 sub-block.",
    )?;
    let q3_column = sheet.formula_row(
        "4. Select an entire column (every category, Q3)",
        &format!("=CVX.INDEX({param},1,3,3,1,\"q3_by_category\")"),
        "Column 3, starting at row 1, spanning all 3 rows: a 3x1 sub-block.",
    )?;
    let sub_block = sheet.formula_row(
        "5. Select a general sub-block (Marketing & Payroll, Q1 & Q2)",
        &format!("=CVX.INDEX({param},1,1,2,2,\"marketing_payroll_h1\")"),
        "Rows 1-2, columns 1-2: a 2x2 sub-block.",
    )?;
    let payroll_total = sheet.formula_row(
        "6. Total payroll for the year (INDEX + SUM)",
        &format!("=CVX.SUM({payroll_row},\"payroll_total_year\")"),
        "Reduces the 1x4 payroll-by-quarter sub-block from step 3 down to one number.",
    )?;

    sheet.blank();
    sheet.section("Answer", "Value", "What it means")?;
    sheet.result_row(
        "Describe the single-entry selection",
        &format!("=CVX.DESCRIBE({one_entry})"),
        "Shows how CVX.DESCRIBE renders a CVX.INDEX result (operand[row, col]).",
    )?;
    sheet.result_row(
        "Describe the full-row selection",
        &format!("=CVX.DESCRIBE({payroll_row})"),
        "Shows how CVX.DESCRIBE renders a row selection (operand[row, col1:col2]).",
    )?;
    sheet.result_row(
        "Shape of the Q3-by-category selection",
        &format!("=CVX.SHAPE({q3_column})"),
        "Confirms the column selection from step 4 is a 3x1 sub-block.",
    )?;
    sheet.result_row(
        "Shape of the H1 sub-block selection",
        &format!("=CVX.SHAPE({sub_block})"),
        "Confirms the general sub-block from step 5 is a 2x2 block.",
    )?;
    sheet.result_row(
        "Describe the payroll total expression",
        &format!("=CVX.DESCRIBE({payroll_total})"),
        "Shows the combined CVX.INDEX + CVX.SUM expression tree from step 6.",
    )?;

    workbook.save(path)?;
    Ok(())
}
