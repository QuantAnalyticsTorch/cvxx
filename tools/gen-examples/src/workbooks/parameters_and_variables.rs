//! `01-parameters-and-variables.xlsx`: a household's known monthly expenses
//! (parameter) alongside an unknown savings target (variable).

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::error::GenError;
use crate::scenario::Sheet;

pub fn build(path: &Path) -> Result<(), GenError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Parameters & Variables")?;

    let mut sheet = Sheet::new(worksheet)?;
    sheet.title("Household Budget: Parameters and Variables")?;
    sheet.narrative(&[
        "Every month a household has certain costs it cannot avoid, and one \
         number it hasn't decided on yet: how much to set aside in savings. \
         cvxx calls the known costs a parameter, and the number still to be \
         decided a variable.",
        "This workbook stores five known monthly expenses as a single cvxx \
         parameter, creates a variable for the undecided monthly savings \
         target, and then inspects both handles with cvxx's description \
         functions.",
    ])?;
    sheet.freeze_here()?;
    sheet.blank();

    sheet.section("Expense", "Amount ($)", "Notes")?;
    let expenses = sheet.input_vector(
        "Monthly expenses",
        &[1450.0, 480.0, 210.0, 260.0, 140.0],
        "Rent, groceries, utilities, transport, and insurance, in that order.",
    )?;

    sheet.blank();
    sheet.section("Step", "Formula", "What it does")?;
    let param = sheet.formula_row(
        "1. Store the expenses as a parameter",
        &format!("=CVX.PARAMETER({expenses},\"expenses\")"),
        "Caches the five known monthly amounts under the name \"expenses\".",
    )?;
    let savings = sheet.formula_row(
        "2. Create a variable for the savings target",
        "=CVX.VARIABLE(1,1,\"savings_target\")",
        "The monthly dollar amount to set aside is not known yet, so it's a variable.",
    )?;

    sheet.blank();
    sheet.section("Answer", "Value", "What it means")?;
    sheet.result_row(
        "Total known monthly expenses",
        &format!("=SUM({expenses})"),
        "The fixed monthly cost the savings target has to be planned around.",
    )?;
    sheet.result_row(
        "Describe the expenses parameter",
        &format!("=CVX.DESCRIBE({param})"),
        "A human-readable summary of the parameter's name and contents.",
    )?;
    sheet.result_row(
        "Expenses parameter shape",
        &format!("=CVX.SHAPE({param})"),
        "Confirms the parameter is a 5x1 column of values.",
    )?;
    sheet.result_row(
        "Savings variable type",
        &format!("=CVX.TYPE({savings})"),
        "Confirms the handle is a variable, not a parameter.",
    )?;

    workbook.save(path)?;
    Ok(())
}
