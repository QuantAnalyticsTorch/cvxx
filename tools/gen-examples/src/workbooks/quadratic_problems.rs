//! `05-quadratic-problems.xlsx`: portfolio risk minimization, choosing asset
//! weights that minimize variance subject to a target return. The flagship
//! "portfolio risk" quadratic example from SPEC-0016.

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::error::GenError;
use crate::scenario::Sheet;

pub fn build(path: &Path) -> Result<(), GenError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Portfolio Risk")?;

    let mut sheet = Sheet::new(worksheet)?;
    sheet.title("Portfolio Risk Minimization: Quadratic Problems")?;
    sheet.narrative(&[
        "An investor splits money between two assets and wants the \
         allocation that is as safe as possible (lowest variance) while still \
         hitting a target average return, and while investing the full \
         amount with no short selling.",
        "Portfolio variance is a quadratic function of the allocation weights \
         (weights.T @ sigma @ weights), which is why this scenario needs \
         cvxx's quadratic problem support rather than the purely affine \
         problems in 04-problems-and-solving.xlsx. The @ and .T operators \
         (matrix multiplication and transpose) are also demonstrated here via \
         their functional builders.",
    ])?;
    sheet.freeze_here()?;
    sheet.blank();

    sheet.section("Known value", "Value(s)", "Notes")?;
    let sigma = sheet.input_matrix(
        "Covariance matrix (sigma)",
        &[vec![0.04, 0.01], vec![0.01, 0.09]],
        "Annualized return variance/covariance between asset 1 and asset 2.",
    )?;
    let returns = sheet.input_vector(
        "Expected annual return",
        &[0.08, 0.12],
        "Asset 1 expects an 8% return, asset 2 a 12% return.",
    )?;
    let target_return = sheet.input_scalar(
        "Target portfolio return",
        0.09,
        "The investor wants at least a 9% expected return.",
    )?;

    sheet.blank();
    sheet.section("Step", "Formula", "What it does")?;
    let weights = sheet.formula_row(
        "1. Create a 2x1 variable for the asset weights",
        "=CVX.VARIABLE(2,1,\"weights\")",
        "weights(1) is the fraction in asset 1, weights(2) the fraction in asset 2.",
    )?;
    let sigma_param = sheet.formula_row(
        "2. Store the covariance matrix as a parameter",
        &format!("=CVX.PARAMETER({sigma},\"sigma\")"),
        "A 2x2 parameter matrix.",
    )?;
    let returns_param = sheet.formula_row(
        "3. Store the expected returns as a parameter",
        &format!("=CVX.PARAMETER({returns},\"expected_returns\")"),
        "A 2x1 parameter vector.",
    )?;
    let min_return_param = sheet.formula_row(
        "4. Store the target return as a parameter",
        &format!("=CVX.PARAMETER({target_return},\"min_return\")"),
        "The minimum acceptable expected annual portfolio return.",
    )?;
    let variance = sheet.formula_row(
        "5. Portfolio variance (@ / .T string grammar)",
        "=CVX.EXPRESSION(\"weights.T @ sigma @ weights\",\"portfolio_variance\")",
        "weights.T @ sigma @ weights is the quadratic form for portfolio variance.",
    )?;
    let variance_alt = sheet.formula_row(
        "5b. The same variance, built from functional MATMUL/TRANSPOSE builders",
        &format!("=CVX.MATMUL(CVX.TRANSPOSE({weights}),CVX.MATMUL({sigma_param},{weights}))"),
        "An unnamed equivalent of step 5, shown purely to demonstrate the functional \
         CVX.MATMUL/CVX.TRANSPOSE builders alongside the @/.T string grammar.",
    )?;
    let weighted_return = sheet.formula_row(
        "6. Portfolio return (MUL + sum(...) string grammar)",
        "=CVX.EXPRESSION(\"sum(expected_returns * weights)\",\"portfolio_return\")",
        "Entrywise-multiplies each asset's weight by its expected return, then sums them.",
    )?;
    let target = sheet.formula_row(
        "7. Target-return constraint",
        "=CVX.CONSTRAINT(\"portfolio_return >= min_return\",\"target_return\")",
        "The expected portfolio return must meet the investor's target.",
    )?;
    let fully_invested = sheet.formula_row(
        "8. Fully-invested constraint",
        "=CVX.CONSTRAINT(\"sum(weights) == 1\",\"fully_invested\")",
        "The two weights must add up to the whole investment (100%).",
    )?;
    let w1_nonneg = sheet.formula_row(
        "9. No short selling asset 1",
        "=CVX.CONSTRAINT(\"index(weights, 1, 1) >= 0\",\"w1_nonneg\")",
        "Selects the first entry of weights and requires it to be non-negative.",
    )?;
    let w2_nonneg = sheet.formula_row(
        "10. No short selling asset 2",
        "=CVX.CONSTRAINT(\"index(weights, 2, 1) >= 0\",\"w2_nonneg\")",
        "Selects the second entry of weights and requires it to be non-negative.",
    )?;
    let constraint_set = sheet.formula_row(
        "11. Combine every constraint into one set",
        &format!("=CVX.CONSTRAINTS({target}:{w2_nonneg},\"portfolio_constraints\")"),
        "Collects the target-return, fully-invested, and no-short-selling constraints.",
    )?;
    let objective = sheet.formula_row(
        "12. Frame variance as a minimization objective",
        &format!("=CVX.MINIMIZE({variance},\"minimize_risk\")"),
        "The goal is the lowest-variance allocation meeting every constraint.",
    )?;
    let problem = sheet.formula_row(
        "13. Assemble the problem",
        &format!("=CVX.PROBLEM({objective},{constraint_set},\"portfolio_problem\")"),
        "Combines the objective and the constraint set into one solvable problem.",
    )?;
    let result = sheet.formula_row(
        "14. Solve it",
        &format!("=CVX.SOLVE({problem},\"portfolio_result\")"),
        "Runs the quadratic solver and returns a handle to the result.",
    )?;

    sheet.blank();
    sheet.section("Answer", "Value(s)", "What it means")?;
    sheet.result_row(
        "Solve status",
        &format!("=CVX.STATUS({result})"),
        "\"optimal\" means a lowest-variance allocation meeting every constraint was found.",
    )?;
    sheet.result_row(
        "Minimized portfolio variance",
        &format!("=CVX.OBJECTIVE_VALUE({result})"),
        "The minimized value of the portfolio_variance expression from step 5.",
    )?;
    sheet.result_row(
        "Allocation weights (asset 1, asset 2)",
        &format!("=CVX.VALUE({result},{weights})"),
        "Spills across two cells: the fraction to invest in each asset.",
    )?;
    sheet.result_row(
        "Describe the alternate variance expression",
        &format!("=CVX.DESCRIBE({variance_alt})"),
        "Confirms the functional MATMUL/TRANSPOSE builders from step 5b produce the \
         same (left) @ (right) structure as the @/.T string grammar.",
    )?;
    sheet.result_row(
        "Weighted-returns expression type",
        &format!("=CVX.TYPE({weighted_return})"),
        "Confirms the handle from step 6 is an expression.",
    )?;
    sheet.result_row(
        "Returns parameter shape",
        &format!("=CVX.SHAPE({returns_param})"),
        "Confirms the expected-returns parameter is a 2x1 column vector.",
    )?;
    sheet.result_row(
        "Describe the minimum-return parameter",
        &format!("=CVX.DESCRIBE({min_return_param})"),
        "The named parameter the target-return constraint's string formula refers to.",
    )?;
    sheet.result_row(
        "Describe the fully-invested constraint",
        &format!("=CVX.DESCRIBE({fully_invested})"),
        "Confirms the constraint from step 8 is part of the constraint set solved above.",
    )?;
    sheet.result_row(
        "Describe the no-short-selling constraint for asset 1",
        &format!("=CVX.DESCRIBE({w1_nonneg})"),
        "Confirms the constraint from step 9 is part of the constraint set solved above.",
    )?;

    workbook.save(path)?;
    Ok(())
}
