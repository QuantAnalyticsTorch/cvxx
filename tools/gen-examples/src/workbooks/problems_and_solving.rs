//! `04-problems-and-solving.xlsx`: the classic diet problem, minimizing
//! weekly grocery cost subject to nutrition requirements.

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::error::GenError;
use crate::scenario::Sheet;

pub fn build(path: &Path) -> Result<(), GenError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Diet Problem")?;

    let mut sheet = Sheet::new(worksheet)?;
    sheet.title("Diet Problem: Minimizing Weekly Grocery Cost")?;
    sheet.narrative(&[
        "A household wants to buy bread and milk to meet its weekly protein \
         and calorie needs as cheaply as possible. Buying more of either food \
         costs more, but each also contributes nutrients, so the cheapest mix \
         is not simply \"buy none of either\".",
        "This workbook builds the cost objective and nutrition constraints as \
         cvxx expressions and constraints, assembles them into a problem, \
         solves it, and reads back how many units of each food to buy.",
    ])?;
    sheet.freeze_here()?;
    sheet.blank();

    sheet.section("Known value", "Amount", "Notes")?;
    sheet.input_scalar("Cost per unit of bread ($)", 2.5, "")?;
    sheet.input_scalar("Cost per unit of milk ($)", 1.8, "")?;
    sheet.input_scalar("Protein per unit of bread (g)", 6.0, "")?;
    sheet.input_scalar("Protein per unit of milk (g)", 8.0, "")?;
    sheet.input_scalar("Calories per unit of bread", 120.0, "")?;
    sheet.input_scalar("Calories per unit of milk", 150.0, "")?;
    sheet.input_scalar("Minimum weekly protein (g)", 24.0, "")?;
    sheet.input_scalar(
        "Minimum weekly calories",
        600.0,
        "The coefficients above are written directly into the formulas below \
         as literals, matching cvxx's string-expression grammar.",
    )?;

    sheet.blank();
    sheet.section("Step", "Formula", "What it does")?;
    let bread = sheet.formula_row(
        "1. Create a variable for units of bread to buy",
        "=CVX.VARIABLE(1,1,\"qty_bread\")",
        "Units of bread to buy this week.",
    )?;
    let milk = sheet.formula_row(
        "2. Create a variable for units of milk to buy",
        "=CVX.VARIABLE(1,1,\"qty_milk\")",
        "Units of milk to buy this week.",
    )?;
    let cost = sheet.formula_row(
        "3. Build the weekly cost expression",
        "=CVX.EXPRESSION(\"2.5 * qty_bread + 1.8 * qty_milk\",\"weekly_cost\")",
        "Total dollars spent as a function of how much bread and milk are bought.",
    )?;
    let protein = sheet.formula_row(
        "4. Protein requirement constraint",
        "=CVX.CONSTRAINT(\"6 * qty_bread + 8 * qty_milk >= 24\",\"protein_requirement\")",
        "Total protein from both foods must meet the weekly minimum.",
    )?;
    let calories = sheet.formula_row(
        "5. Calorie requirement constraint",
        "=CVX.CONSTRAINT(\"120 * qty_bread + 150 * qty_milk >= 600\",\"calorie_requirement\")",
        "Total calories from both foods must meet the weekly minimum.",
    )?;
    let bread_nonneg = sheet.formula_row(
        "6. Non-negativity for bread",
        "=CVX.CONSTRAINT(\"qty_bread >= 0\",\"bread_nonneg\")",
        "Cannot buy a negative amount of bread.",
    )?;
    let milk_nonneg = sheet.formula_row(
        "7. Non-negativity for milk",
        "=CVX.CONSTRAINT(\"qty_milk >= 0\",\"milk_nonneg\")",
        "Cannot buy a negative amount of milk.",
    )?;
    let constraint_set = sheet.formula_row(
        "8. Combine every constraint into one set",
        &format!("=CVX.CONSTRAINTS({protein}:{milk_nonneg},\"diet_constraints\")"),
        "Collects the two nutrition requirements and both non-negativity constraints.",
    )?;
    let objective = sheet.formula_row(
        "9. Frame cost as a minimization objective",
        &format!("=CVX.MINIMIZE({cost},\"minimize_cost\")"),
        "The goal is to spend as little as possible.",
    )?;
    let problem = sheet.formula_row(
        "10. Assemble the problem",
        &format!("=CVX.PROBLEM({objective},{constraint_set},\"diet_problem\")"),
        "Combines the objective and the constraint set into one solvable problem.",
    )?;
    let result = sheet.formula_row(
        "11. Solve it",
        &format!("=CVX.SOLVE({problem},\"diet_result\")"),
        "Runs the solver and returns a handle to the result.",
    )?;

    sheet.blank();
    sheet.section("Answer", "Value", "What it means")?;
    sheet.result_row(
        "Solve status",
        &format!("=CVX.STATUS({result})"),
        "\"optimal\" means a cheapest feasible mix of bread and milk was found.",
    )?;
    sheet.result_row(
        "Cheapest weekly grocery cost that still meets every nutrition requirement",
        &format!("=CVX.OBJECTIVE_VALUE({result})"),
        "The minimized value of the weekly_cost expression from step 3.",
    )?;
    sheet.result_row(
        "Units of bread to buy",
        &format!("=CVX.VALUE({result},{bread})"),
        "The solved value of the qty_bread variable from step 1.",
    )?;
    sheet.result_row(
        "Units of milk to buy",
        &format!("=CVX.VALUE({result},{milk})"),
        "The solved value of the qty_milk variable from step 2.",
    )?;
    sheet.result_row(
        "Describe the calorie requirement constraint",
        &format!("=CVX.DESCRIBE({calories})"),
        "Confirms the constraint from step 5 is part of the constraint set solved above.",
    )?;
    sheet.result_row(
        "Describe the bread non-negativity constraint",
        &format!("=CVX.DESCRIBE({bread_nonneg})"),
        "Confirms the constraint from step 6 is part of the constraint set solved above.",
    )?;

    workbook.save(path)?;
    Ok(())
}
