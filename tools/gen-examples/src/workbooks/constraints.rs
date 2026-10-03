//! `03-constraints.xlsx`: a factory's production limits expressed as a
//! checklist of capacity and demand constraints.

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::error::GenError;
use crate::scenario::Sheet;

pub fn build(path: &Path) -> Result<(), GenError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Constraints")?;

    let mut sheet = Sheet::new(worksheet)?;
    sheet.title("Factory Production Limits: Building Constraints")?;
    sheet.narrative(&[
        "A small factory makes two products on shared machines. It cannot run \
         the machines more than a fixed number of hours a week, must make at \
         least a minimum batch of product 1 to satisfy a standing order, and \
         has an exact production quota for product 2 from a signed contract.",
        "This workbook expresses each of those business rules as a cvxx \
         constraint, then combines them into a single constraint set ready to \
         hand to a problem (see 04-problems-and-solving.xlsx for how a \
         constraint set is actually solved against).",
    ])?;
    sheet.freeze_here()?;
    sheet.blank();

    sheet.section("Known value", "Amount", "Notes")?;
    let machine_hours = sheet.input_scalar(
        "Weekly machine-hours available",
        480.0,
        "Total shared machine capacity per week, in hours.",
    )?;
    let min_product1 = sheet.input_scalar(
        "Minimum units of product 1",
        50.0,
        "Standing order that must always be satisfied.",
    )?;
    let product2_quota = sheet.input_scalar(
        "Exact units of product 2",
        120.0,
        "Fixed quota from a signed supply contract.",
    )?;

    sheet.blank();
    sheet.section("Step", "Formula", "What it does")?;
    let product1 = sheet.formula_row(
        "1. Create a variable for product 1's weekly output",
        "=CVX.VARIABLE(1,1,\"product1\")",
        "Units of product 1 to produce this week.",
    )?;
    let product2 = sheet.formula_row(
        "2. Create a variable for product 2's weekly output",
        "=CVX.VARIABLE(1,1,\"product2\")",
        "Units of product 2 to produce this week.",
    )?;
    let capacity_param = sheet.formula_row(
        "3. Store the machine-hour limit as a parameter",
        &format!("=CVX.PARAMETER({machine_hours},\"machine_hours_limit\")"),
        "One hour of machine time is needed per unit of either product.",
    )?;
    let quota_param = sheet.formula_row(
        "4. Store the product 2 quota as a parameter",
        &format!("=CVX.PARAMETER({product2_quota},\"product2_quota\")"),
        "The supply contract's fixed quota for product 2.",
    )?;
    let capacity = sheet.formula_row(
        "5. Capacity constraint (string grammar)",
        "=CVX.CONSTRAINT(\"product1 + product2 <= machine_hours_limit\",\"capacity\")",
        "Total output cannot exceed the available machine-hours.",
    )?;
    let min_demand = sheet.formula_row(
        "6. Minimum order constraint (functional GREATER_THAN builder)",
        &format!("=CVX.GREATER_THAN({product1},{min_product1},\"min_product1\")"),
        "Product 1 output must satisfy the standing order.",
    )?;
    let exact_quota = sheet.formula_row(
        "7. Exact quota constraint (functional EQUAL builder)",
        &format!("=CVX.EQUAL({product2},{quota_param},\"exact_quota\")"),
        "Product 2 output must match the contract exactly.",
    )?;
    let nonneg1 = sheet.formula_row(
        "8. Non-negativity for product 1 (functional LESS_THAN builder)",
        &format!("=CVX.LESS_THAN(0,{product1},\"product1_nonneg\")"),
        "Output cannot be negative.",
    )?;
    let constraint_set = sheet.formula_row(
        "9. Combine every constraint into one set",
        &format!(
            "=CVX.CONSTRAINTS({},\"production_constraints\")",
            contiguous_range(&capacity, &nonneg1)
        ),
        "Collects the capacity, minimum-order, exact-quota, and non-negativity \
         constraints into one handle, ready to pass to CVX.PROBLEM.",
    )?;

    sheet.blank();
    sheet.section("Answer", "Value", "What it means")?;
    sheet.result_row(
        "Describe the constraint set",
        &format!("=CVX.DESCRIBE({constraint_set})"),
        "Lists every constraint that was combined into the set.",
    )?;
    sheet.result_row(
        "Constraint set type",
        &format!("=CVX.TYPE({constraint_set})"),
        "Confirms the handle is a constraint_set, not a single constraint.",
    )?;
    sheet.result_row(
        "Describe the minimum-order constraint",
        &format!("=CVX.DESCRIBE({min_demand})"),
        "Shows the individual constraint built with CVX.GREATER_THAN in step 5.",
    )?;
    sheet.result_row(
        "Describe the exact-quota constraint",
        &format!("=CVX.DESCRIBE({exact_quota})"),
        "Shows the individual constraint built with CVX.EQUAL in step 7.",
    )?;
    sheet.result_row(
        "Describe the machine-hour parameter",
        &format!("=CVX.DESCRIBE({capacity_param})"),
        "The named parameter the capacity constraint's string formula refers to.",
    )?;

    workbook.save(path)?;
    Ok(())
}

/// The four constraint-producing step cells (capacity, min_demand,
/// exact_quota, nonneg1) are written on four consecutive rows in the value
/// column, so their combined range is simply `first:last`.
fn contiguous_range(first: &str, last: &str) -> String {
    format!("{first}:{last}")
}
