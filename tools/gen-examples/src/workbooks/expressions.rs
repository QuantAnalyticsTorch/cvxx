//! `02-expressions.xlsx`: a markup-pricing calculator that builds a profit
//! expression from price, cost, and quantity.

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::error::GenError;
use crate::scenario::Sheet;

pub fn build(path: &Path) -> Result<(), GenError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Expressions")?;

    let mut sheet = Sheet::new(worksheet)?;
    sheet.title("Markup Pricing Calculator: Building Expressions")?;
    sheet.narrative(&[
        "A boutique wants to see, as a formula it can later optimize, how its \
         selling price turns into profit once unit cost, expected quantity \
         sold, a planned marketing discount, and a one-time signing bonus are \
         all taken into account.",
        "cvxx expressions are lazy: each CVX.* call below returns a handle \
         representing a formula, not yet a number. The price itself is still \
         a variable (it hasn't been decided), so nothing here evaluates to a \
         dollar amount until a problem built from it is solved.",
    ])?;
    sheet.freeze_here()?;
    sheet.blank();

    sheet.section("Known value", "Amount", "Notes")?;
    let unit_cost = sheet.input_scalar(
        "Unit cost ($)",
        18.0,
        "What it costs the boutique to make one unit.",
    )?;
    let quantity = sheet.input_scalar(
        "Expected quantity sold",
        350.0,
        "Units expected to sell this season.",
    )?;
    let discount = sheet.input_scalar(
        "Marketing discount ($)",
        250.0,
        "A planned one-time promotional discount.",
    )?;
    let bonus = sheet.input_scalar(
        "Signing bonus ($)",
        500.0,
        "A one-time bonus from a new wholesale partner.",
    )?;

    sheet.blank();
    sheet.section("Step", "Formula", "What it does")?;
    let price = sheet.formula_row(
        "1. Create a variable for the selling price",
        "=CVX.VARIABLE(1,1,\"price\")",
        "The per-unit selling price has not been decided yet.",
    )?;
    let cost_param = sheet.formula_row(
        "2. Store the unit cost as a parameter",
        &format!("=CVX.PARAMETER({unit_cost},\"unit_cost\")"),
        "The cost per unit is known in advance.",
    )?;
    let qty_param = sheet.formula_row(
        "3. Store the expected quantity as a parameter",
        &format!("=CVX.PARAMETER({quantity},\"quantity\")"),
        "The expected sales volume is known in advance.",
    )?;
    let discount_param = sheet.formula_row(
        "4. Store the marketing discount as a parameter",
        &format!("=CVX.PARAMETER({discount},\"discount_amount\")"),
        "The planned discount is a known, fixed amount.",
    )?;
    let bonus_param = sheet.formula_row(
        "5. Store the signing bonus as a parameter",
        &format!("=CVX.PARAMETER({bonus},\"signing_bonus\")"),
        "The one-time bonus is a known, fixed amount.",
    )?;
    let margin_per_unit = sheet.formula_row(
        "6. Margin per unit (string grammar)",
        "=CVX.EXPRESSION(\"price - unit_cost\",\"margin_per_unit\")",
        "Selling price minus unit cost, written as a string expression.",
    )?;
    let gross_margin = sheet.formula_row(
        "7. Gross margin (functional MUL builder)",
        &format!("=CVX.MUL({margin_per_unit},{qty_param},\"gross_margin\")"),
        "Margin per unit times expected quantity, built from handles instead of a string.",
    )?;
    let total_cost = sheet.formula_row(
        "8. Total cost (functional MUL builder)",
        &format!("=CVX.MUL({cost_param},{qty_param},\"total_cost\")"),
        "Unit cost times expected quantity.",
    )?;
    let margin_ratio = sheet.formula_row(
        "9. Margin ratio (functional DIV builder)",
        &format!("=CVX.DIV({gross_margin},{total_cost},\"margin_ratio\")"),
        "Gross margin divided by total cost.",
    )?;
    let discount_as_cost = sheet.formula_row(
        "10. Discount expressed as a negative cash flow (NEG builder)",
        &format!("=CVX.NEG({discount_param},\"discount_as_cost\")"),
        "Flips the sign of the discount so it can be added like any other cash flow.",
    )?;
    let net_margin = sheet.formula_row(
        "11. Net margin after the discount (functional ADD builder)",
        &format!("=CVX.ADD({gross_margin},{discount_as_cost},\"net_margin\")"),
        "Gross margin plus the (now negative) discount.",
    )?;
    let after_bonus = sheet.formula_row(
        "12. Net margin plus the signing bonus (functional ADD builder)",
        &format!("=CVX.ADD({net_margin},{bonus_param},\"contribution\")"),
        "Adds the one-time wholesale signing bonus to the net margin.",
    )?;
    let after_tax = sheet.formula_row(
        "13. After-tax contribution (SCALE builder)",
        &format!("=CVX.SCALE({after_bonus},0.9,\"contribution_after_tax\")"),
        "Scales the total contribution by 0.9 to approximate a 10% tax.",
    )?;
    let maximize_contribution = sheet.formula_row(
        "14. Frame it as a maximization objective",
        &format!("=CVX.MAXIMIZE({after_tax},\"maximize_contribution\")"),
        "Objectives are built the same way expressions are; solving them is covered \
         in 04-problems-and-solving.xlsx and 05-quadratic-problems.xlsx.",
    )?;

    sheet.blank();
    sheet.section("Answer", "Value", "What it means")?;
    sheet.result_row(
        "Describe the after-tax contribution expression",
        &format!("=CVX.DESCRIBE({after_tax})"),
        "Shows the full expression tree built from the 14 steps above.",
    )?;
    sheet.result_row(
        "Describe the maximization objective",
        &format!("=CVX.DESCRIBE({maximize_contribution})"),
        "Confirms the objective wraps the same expression for maximization.",
    )?;
    sheet.result_row(
        "Confirm the price handle is a variable",
        &format!("=CVX.TYPE({price})"),
        "The price itself started life as a CVX.VARIABLE in step 1.",
    )?;
    sheet.result_row(
        "Describe the margin ratio expression",
        &format!("=CVX.DESCRIBE({margin_ratio})"),
        "Confirms the functional DIV builder from step 9 produces a single division node.",
    )?;

    workbook.save(path)?;
    Ok(())
}
