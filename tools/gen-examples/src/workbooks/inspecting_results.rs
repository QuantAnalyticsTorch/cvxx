//! `07-inspecting-results.xlsx`: re-examining a solved diet problem, reading
//! back status, cost, quantities, and handle descriptions in depth.

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::error::GenError;
use crate::scenario::Sheet;

pub fn build(path: &Path) -> Result<(), GenError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Inspecting Results")?;

    let mut sheet = Sheet::new(worksheet)?;
    sheet.title("Inspecting Results: Reading Back a Solved Problem")?;
    sheet.narrative(&[
        "Once a cvxx problem has been solved, the result handle it returns \
         carries the solve status, the objective value, and every decision \
         variable's solved value(s) \u{2014} but only CVX.STATUS/CVX.OBJECTIVE_VALUE/ \
         CVX.VALUE know how to read them back.",
        "This workbook re-builds the same small diet problem introduced in \
         04-problems-and-solving.xlsx, then spends most of its space on the \
         six inspection functions (CVX.VALUE, CVX.STATUS, \
         CVX.OBJECTIVE_VALUE, CVX.DESCRIBE, CVX.SHAPE, CVX.TYPE), applied to \
         every kind of handle involved: the variable, the problem, the \
         objective, and the result itself.",
    ])?;
    sheet.freeze_here()?;
    sheet.blank();

    sheet.section("Known value", "Amount", "Notes")?;
    sheet.input_scalar("Cost per unit of bread ($)", 2.5, "")?;
    sheet.input_scalar("Cost per unit of milk ($)", 1.8, "")?;
    sheet.input_scalar(
        "Minimum weekly protein (g) and calories",
        24.0,
        "Same recap scenario as 04-problems-and-solving.xlsx: 6g/8g protein and \
         120/150 calories per unit of bread/milk, minimum 600 weekly calories.",
    )?;

    sheet.blank();
    sheet.section(
        "Recap: rebuild and solve the diet problem",
        "Formula",
        "What it does",
    )?;
    let bread = sheet.formula_row(
        "1. Variable: units of bread",
        "=CVX.VARIABLE(1,1,\"qty_bread\")",
        "Units of bread to buy this week.",
    )?;
    let milk = sheet.formula_row(
        "2. Variable: units of milk",
        "=CVX.VARIABLE(1,1,\"qty_milk\")",
        "Units of milk to buy this week.",
    )?;
    let cost = sheet.formula_row(
        "3. Expression: weekly cost",
        "=CVX.EXPRESSION(\"2.5 * qty_bread + 1.8 * qty_milk\",\"weekly_cost\")",
        "Total dollars spent as a function of bread and milk quantities.",
    )?;
    let protein = sheet.formula_row(
        "4. Constraint: protein requirement",
        "=CVX.CONSTRAINT(\"6 * qty_bread + 8 * qty_milk >= 24\",\"protein_requirement\")",
        "Total protein from both foods must meet the weekly minimum.",
    )?;
    let calories = sheet.formula_row(
        "5. Constraint: calorie requirement",
        "=CVX.CONSTRAINT(\"120 * qty_bread + 150 * qty_milk >= 600\",\"calorie_requirement\")",
        "Total calories from both foods must meet the weekly minimum.",
    )?;
    let bread_nonneg = sheet.formula_row(
        "6. Constraint: bread non-negativity",
        "=CVX.CONSTRAINT(\"qty_bread >= 0\",\"bread_nonneg\")",
        "Cannot buy a negative amount of bread.",
    )?;
    let milk_nonneg = sheet.formula_row(
        "7. Constraint: milk non-negativity",
        "=CVX.CONSTRAINT(\"qty_milk >= 0\",\"milk_nonneg\")",
        "Cannot buy a negative amount of milk.",
    )?;
    let constraint_set = sheet.formula_row(
        "8. Combine every constraint",
        &format!("=CVX.CONSTRAINTS({protein}:{milk_nonneg},\"diet_constraints\")"),
        "Collects the two nutrition requirements and both non-negativity constraints.",
    )?;
    let objective = sheet.formula_row(
        "9. Objective: minimize cost",
        &format!("=CVX.MINIMIZE({cost},\"minimize_cost\")"),
        "The goal is to spend as little as possible.",
    )?;
    let problem = sheet.formula_row(
        "10. Problem: assemble objective + constraints",
        &format!("=CVX.PROBLEM({objective},{constraint_set},\"diet_problem\")"),
        "Combines the objective and the constraint set into one solvable problem.",
    )?;
    let result = sheet.formula_row(
        "11. Solve",
        &format!("=CVX.SOLVE({problem},\"diet_result\")"),
        "Runs the solver and returns a handle to the result.",
    )?;

    sheet.blank();
    sheet.section("Reading back the result", "Formula", "What it does")?;
    sheet.formula_row(
        "12. CVX.STATUS on the result",
        &format!("=CVX.STATUS({result})"),
        "\"optimal\", \"infeasible\", or \"unbounded\".",
    )?;
    sheet.formula_row(
        "13. CVX.OBJECTIVE_VALUE on the result",
        &format!("=CVX.OBJECTIVE_VALUE({result})"),
        "The minimized weekly cost; #VALUE! if the result were not optimal.",
    )?;
    sheet.formula_row(
        "14. CVX.VALUE for the bread variable",
        &format!("=CVX.VALUE({result},{bread})"),
        "CVX.VALUE always needs both the result and the variable explicitly.",
    )?;
    sheet.formula_row(
        "15. CVX.VALUE for the milk variable",
        &format!("=CVX.VALUE({result},{milk})"),
        "Same result handle, a different variable from the same solved problem.",
    )?;

    sheet.blank();
    sheet.section("Describing every kind of handle", "Formula", "What it does")?;
    sheet.formula_row(
        "16. CVX.TYPE on the variable",
        &format!("=CVX.TYPE({bread})"),
        "\"variable\".",
    )?;
    sheet.formula_row(
        "17. CVX.TYPE on the objective",
        &format!("=CVX.TYPE({objective})"),
        "\"objective\".",
    )?;
    sheet.formula_row(
        "18. CVX.TYPE on the problem",
        &format!("=CVX.TYPE({problem})"),
        "\"problem\".",
    )?;
    sheet.formula_row(
        "19. CVX.TYPE on the result",
        &format!("=CVX.TYPE({result})"),
        "\"result\".",
    )?;
    sheet.formula_row(
        "20. CVX.SHAPE on the bread variable",
        &format!("=CVX.SHAPE({bread})"),
        "\"1x1\"; CVX.SHAPE only applies to parameters, variables, and expressions.",
    )?;
    sheet.formula_row(
        "21. CVX.DESCRIBE on the problem",
        &format!("=CVX.DESCRIBE({problem})"),
        "A diagnostic summary naming the objective and every constraint it bundles.",
    )?;

    sheet.blank();
    sheet.section("Answer", "Value", "What it means")?;
    sheet.result_row(
        "Solve status",
        &format!("=CVX.STATUS({result})"),
        "\"optimal\" means a cheapest feasible mix of bread and milk was found.",
    )?;
    sheet.result_row(
        "Cheapest weekly grocery cost",
        &format!("=CVX.OBJECTIVE_VALUE({result})"),
        "The minimized value of the weekly_cost expression.",
    )?;
    sheet.result_row(
        "Units of bread to buy",
        &format!("=CVX.VALUE({result},{bread})"),
        "The solved value of the qty_bread variable.",
    )?;
    sheet.result_row(
        "Units of milk to buy",
        &format!("=CVX.VALUE({result},{milk})"),
        "The solved value of the qty_milk variable.",
    )?;
    sheet.result_row(
        "Describe the result handle itself",
        &format!("=CVX.DESCRIBE({result})"),
        "Confirms CVX.DESCRIBE works on every registry object kind, including results.",
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
