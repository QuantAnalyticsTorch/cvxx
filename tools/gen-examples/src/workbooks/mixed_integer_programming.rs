//! `08-mixed-integer-programming.xlsx`: a project-selection knapsack,
//! choosing which proposals to fund (yes/no decisions) within a fixed
//! budget. The flagship mixed-integer example from SPEC-0019/ISSUE-0019.

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::error::GenError;
use crate::scenario::Sheet;

pub fn build(path: &Path) -> Result<(), GenError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Project Selection")?;

    let mut sheet = Sheet::new(worksheet)?;
    sheet.title("Project Selection: Mixed-Integer Programming")?;
    sheet.narrative(&[
        "A manager has four candidate projects, each with an upfront cost and \
         an expected value, and a fixed budget. Each project is either fully \
         funded or not at all — there is no such thing as funding 40% of a \
         project — so the decision for each one is a yes/no (binary) choice, \
         not a continuous fraction.",
        "cvxx models a yes/no decision as a variable restricted to the \
         binary domain {0, 1} with CVX.BINARY (see CVX.INTEGER for whole-number \
         decisions like truck counts, which allow any integer rather than just \
         0 and 1). Passing a CVX.BINARY/CVX.INTEGER handle into CVX.CONSTRAINTS \
         or CVX.PROBLEM alongside ordinary constraints routes the problem to \
         cvxx's mixed-integer solver path instead of its continuous one.",
    ])?;
    sheet.freeze_here()?;
    sheet.blank();

    sheet.section("Known value", "Value(s)", "Notes")?;
    let costs = sheet.input_vector(
        "Project cost ($k)",
        &[40.0, 50.0, 30.0, 70.0],
        "Upfront cost of projects 1 through 4, in thousands of dollars.",
    )?;
    let values = sheet.input_vector(
        "Project expected value ($k)",
        &[70.0, 65.0, 50.0, 110.0],
        "Expected value delivered by projects 1 through 4, in thousands of dollars.",
    )?;
    let budget = sheet.input_scalar(
        "Total budget ($k)",
        100.0,
        "The most the manager can spend across every funded project.",
    )?;

    sheet.blank();
    sheet.section("Step", "Formula", "What it does")?;
    let selected = sheet.formula_row(
        "1. Create a 4x1 variable for the funding decisions",
        "=CVX.VARIABLE(4,1,\"selected\")",
        "selected(i) will be 1 if project i is funded, 0 otherwise.",
    )?;
    let cost_param = sheet.formula_row(
        "2. Store the project costs as a parameter",
        &format!("=CVX.PARAMETER({costs},\"project_cost\")"),
        "A 4x1 parameter vector.",
    )?;
    let value_param = sheet.formula_row(
        "3. Store the project values as a parameter",
        &format!("=CVX.PARAMETER({values},\"project_value\")"),
        "A 4x1 parameter vector.",
    )?;
    let budget_param = sheet.formula_row(
        "4. Store the budget as a parameter",
        &format!("=CVX.PARAMETER({budget},\"budget\")"),
        "The spending limit across every funded project.",
    )?;
    let total_cost = sheet.formula_row(
        "5. Total spend across funded projects",
        "=CVX.EXPRESSION(\"sum(project_cost * selected)\",\"total_cost\")",
        "Entrywise-multiplies each project's cost by its funding decision, then sums them.",
    )?;
    let total_value = sheet.formula_row(
        "6. Total value across funded projects",
        "=CVX.EXPRESSION(\"sum(project_value * selected)\",\"total_value\")",
        "Entrywise-multiplies each project's value by its funding decision, then sums them.",
    )?;
    let binary_domain = sheet.formula_row(
        "7. Restrict every funding decision to yes/no",
        &format!("=CVX.BINARY({selected},\"selected_is_binary\")"),
        "Each entry of selected must be exactly 0 or 1, not a fraction in between.",
    )?;
    let within_budget = sheet.formula_row(
        "8. Budget constraint",
        "=CVX.CONSTRAINT(\"total_cost <= budget\",\"within_budget\")",
        "The total cost of funded projects cannot exceed the budget.",
    )?;
    let constraint_set = sheet.formula_row(
        "9. Combine the binary domain and the constraint into one set",
        &format!("=CVX.CONSTRAINTS({binary_domain}:{within_budget},\"selection_constraints\")"),
        "CVX.CONSTRAINTS accepts both ordinary constraint handles and domain \
         handles from CVX.BINARY/CVX.INTEGER in the same range.",
    )?;
    let objective = sheet.formula_row(
        "10. Frame total value as a maximization objective",
        &format!("=CVX.MAXIMIZE({total_value},\"maximize_value\")"),
        "The goal is the highest-value set of projects the budget allows.",
    )?;
    let problem = sheet.formula_row(
        "11. Assemble the problem",
        &format!("=CVX.PROBLEM({objective},{constraint_set},\"selection_problem\")"),
        "Combining a CVX.BINARY-referencing constraint set routes this problem \
         to cvxx's mixed-integer solver instead of its continuous one.",
    )?;
    let result = sheet.formula_row(
        "12. Solve it",
        &format!("=CVX.SOLVE({problem},\"selection_result\")"),
        "Runs the mixed-integer solver and returns a handle to the result.",
    )?;

    sheet.blank();
    sheet.section("Answer", "Value(s)", "What it means")?;
    sheet.result_row(
        "Solve status",
        &format!("=CVX.STATUS({result})"),
        "\"optimal\" means the highest-value funding plan within budget was \
         proven optimal (see 04-problems-and-solving.xlsx for cvxx's other \
         possible statuses, and docs/problems.md for \"stopped_at_limit\", \
         which can occur on harder mixed-integer problems).",
    )?;
    sheet.result_row(
        "Maximized total value ($k)",
        &format!("=CVX.OBJECTIVE_VALUE({result})"),
        "The best achievable total project value within the budget.",
    )?;
    sheet.result_row(
        "Funding decisions (projects 1-4)",
        &format!("=CVX.VALUE({result},{selected})"),
        "Spills across four cells: 1 means fund the project, 0 means skip it.",
    )?;
    sheet.result_row(
        "Describe the binary domain",
        &format!("=CVX.DESCRIBE({binary_domain})"),
        "Confirms the domain handle from step 7 restricts the whole selected variable.",
    )?;
    sheet.result_row(
        "Describe the total-cost expression",
        &format!("=CVX.DESCRIBE({total_cost})"),
        "Shows the expression built in step 5 that the budget constraint's string formula refers to by name.",
    )?;
    sheet.result_row(
        "Binary domain handle type",
        &format!("=CVX.TYPE({binary_domain})"),
        "Confirms the handle from step 7 is a domain, not a constraint.",
    )?;
    sheet.result_row(
        "Describe the budget constraint",
        &format!("=CVX.DESCRIBE({within_budget})"),
        "Confirms the ordinary constraint from step 8 is part of the set solved above.",
    )?;
    sheet.result_row(
        "Describe the budget parameter",
        &format!("=CVX.DESCRIBE({budget_param})"),
        "The named parameter the budget constraint's string formula refers to.",
    )?;
    sheet.result_row(
        "Total-value expression type",
        &format!("=CVX.TYPE({total_value})"),
        "Confirms the handle from step 6 is an expression.",
    )?;
    sheet.result_row(
        "Project-cost parameter shape",
        &format!("=CVX.SHAPE({cost_param})"),
        "Confirms the project-cost parameter is a 4x1 column vector.",
    )?;
    sheet.result_row(
        "Project-value parameter shape",
        &format!("=CVX.SHAPE({value_param})"),
        "Confirms the project-value parameter is a 4x1 column vector.",
    )?;

    workbook.save(path)?;
    Ok(())
}
