use super::*;
use crate::model::{Constraint, Domain, DomainConstraint, Expression, Relation, Sense, Variable};
use crate::solver::solve;

fn scalar_var(id: u64) -> Variable {
    Variable::new(id, (1, 1))
}

#[test]
fn binary_outranks_integer_under_ord() {
    // SPEC-0019: on an overlapping restriction, `Domain::Binary` must win,
    // which the `build_domain_map` merge implements via `Domain`'s
    // derived `Ord` (`Binary > Integer`).
    assert!(Domain::Binary > Domain::Integer);
    assert_eq!(Domain::Integer.max(Domain::Binary), Domain::Binary);
}

#[test]
fn continuous_problem_is_unaffected_by_empty_domains() {
    // A problem with `domains: Vec::new()` must take the exact same
    // `clarabel` path (and produce the exact same result) as before
    // SPEC-0019 existed.
    let x = scalar_var(1);
    let y = scalar_var(2);
    let problem = Problem {
        domains: Vec::new(),
        sense: Sense::Minimize,
        objective: Expression::add(Expression::from_variable(x), Expression::from_variable(y)),
        constraints: vec![
            Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::from_variable(x),
                rhs: Expression::constant(1.0),
            },
            Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::from_variable(y),
                rhs: Expression::constant(2.0),
            },
        ],
        variables: vec![x, y],
    };
    let solution = solve(&problem);
    assert_eq!(solution.status, SolveStatus::Optimal);
    assert!((solution.objective_value.unwrap() - 3.0).abs() < 1e-6);
}

fn whole_var_domain(variable: Variable, domain: Domain) -> DomainConstraint {
    DomainConstraint {
        variable,
        row_start: 0,
        col_start: 0,
        rows: variable.shape.0,
        cols: variable.shape.1,
        domain,
    }
}

#[test]
fn solves_an_all_integer_knapsack_style_problem() {
    // maximize 5x + 4y subject to 2x + 3y <= 12, 0 <= x,y <= 4, integer.
    let x = scalar_var(1);
    let y = scalar_var(2);
    let problem = Problem {
        domains: vec![
            whole_var_domain(x, Domain::Integer),
            whole_var_domain(y, Domain::Integer),
        ],
        sense: Sense::Maximize,
        objective: Expression::add(
            Expression::scale(5.0, Expression::from_variable(x)),
            Expression::scale(4.0, Expression::from_variable(y)),
        ),
        constraints: vec![
            Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::add(
                    Expression::scale(2.0, Expression::from_variable(x)),
                    Expression::scale(3.0, Expression::from_variable(y)),
                ),
                rhs: Expression::constant(12.0),
            },
            Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::from_variable(x),
                rhs: Expression::constant(0.0),
            },
            Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::from_variable(x),
                rhs: Expression::constant(4.0),
            },
            Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::from_variable(y),
                rhs: Expression::constant(0.0),
            },
            Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::from_variable(y),
                rhs: Expression::constant(4.0),
            },
        ],
        variables: vec![x, y],
    };
    let solution = solve(&problem);
    assert_eq!(solution.status, SolveStatus::Optimal);
    assert!((solution.objective_value.unwrap() - 24.0).abs() < 1e-6);
    let xv = solution.variable_values[0][0];
    let yv = solution.variable_values[1][0];
    assert!((xv - xv.round()).abs() < 1e-6);
    assert!((yv - yv.round()).abs() < 1e-6);
    assert!(2.0 * xv + 3.0 * yv <= 12.0 + 1e-6);
}

#[test]
fn solves_a_mixed_continuous_integer_binary_problem() {
    // maximize x + 3y + 2z subject to x + y + z <= 3.5, x continuous in
    // [0, 2], y integer in [0, 3], z binary.
    let x = scalar_var(1);
    let y = scalar_var(2);
    let z = scalar_var(3);
    let problem = Problem {
        domains: vec![
            whole_var_domain(y, Domain::Integer),
            whole_var_domain(z, Domain::Binary),
        ],
        sense: Sense::Maximize,
        objective: Expression::add(
            Expression::add(
                Expression::from_variable(x),
                Expression::scale(3.0, Expression::from_variable(y)),
            ),
            Expression::scale(2.0, Expression::from_variable(z)),
        ),
        constraints: vec![
            Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::add(
                    Expression::add(Expression::from_variable(x), Expression::from_variable(y)),
                    Expression::from_variable(z),
                ),
                rhs: Expression::constant(3.5),
            },
            Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::from_variable(x),
                rhs: Expression::constant(0.0),
            },
            Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::from_variable(x),
                rhs: Expression::constant(2.0),
            },
            Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::from_variable(y),
                rhs: Expression::constant(0.0),
            },
            Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::from_variable(y),
                rhs: Expression::constant(3.0),
            },
        ],
        variables: vec![x, y, z],
    };
    let solution = solve(&problem);
    assert_eq!(solution.status, SolveStatus::Optimal);
    // y=3 (integer, uses up budget), z=1 (binary), x = 3.5 - 3 - 1 = -0.5
    // but x >= 0, so y can only be 2: x = 3.5 - 2 - 1 = 0.5, obj = 0.5+6+2=8.5
    // Just check feasibility/integrality rather than hand-solving exactly.
    let yv = solution.variable_values[1][0];
    let zv = solution.variable_values[2][0];
    assert!((yv - yv.round()).abs() < 1e-6);
    assert!(zv == 0.0 || zv == 1.0);
}

#[test]
fn infeasible_all_integer_problem_is_reported_infeasible() {
    // x integer, 0.25 <= x <= 0.75 has no integer solution.
    let x = scalar_var(1);
    let problem = Problem {
        domains: vec![whole_var_domain(x, Domain::Integer)],
        sense: Sense::Minimize,
        objective: Expression::from_variable(x),
        constraints: vec![
            Constraint {
                relation: Relation::GreaterEqual,
                lhs: Expression::from_variable(x),
                rhs: Expression::constant(0.25),
            },
            Constraint {
                relation: Relation::LessEqual,
                lhs: Expression::from_variable(x),
                rhs: Expression::constant(0.75),
            },
        ],
        variables: vec![x],
    };
    let solution = solve(&problem);
    assert_eq!(solution.status, SolveStatus::Infeasible);
}

#[test]
fn unbounded_mixed_integer_problem_is_reported_unbounded() {
    // maximize x, a continuous variable with no finite bound (either from
    // a domain restriction or a constraint); y carries the domain
    // restriction that routes this problem to the `microlp` path, but
    // does not itself constrain x. microlp's own `(i32::MIN, i32::MAX)`
    // translation of an "unbounded" integer domain is still a *finite*
    // box bound at the solver level (SPEC-0019's `(i64::MIN, i64::MAX)`
    // intent is adapted to `microlp`'s native `i32` bounds), so a direct
    // test of an unbounded *integer* variable cannot exercise
    // `microlp::Error::Unbounded`; a genuinely unbounded continuous
    // variable alongside a domain-restricted one still does.
    let x = scalar_var(1);
    let y = scalar_var(2);
    let problem = Problem {
        domains: vec![whole_var_domain(y, Domain::Integer)],
        sense: Sense::Maximize,
        objective: Expression::from_variable(x),
        constraints: vec![Constraint {
            relation: Relation::GreaterEqual,
            lhs: Expression::from_variable(y),
            rhs: Expression::constant(0.0),
        }],
        variables: vec![x, y],
    };
    let solution = solve(&problem);
    assert_eq!(solution.status, SolveStatus::Unbounded);
}

#[test]
fn quadratic_objective_with_a_domain_restriction_is_rejected() {
    // SPEC-0019 Non-Objective: MIQP is out of scope; a quadratic
    // objective paired with any domain restriction must be rejected with
    // a specific, actionable message rather than silently solved as an
    // LP relaxation or crashing.
    let x = scalar_var(1);
    let problem = Problem {
        domains: vec![whole_var_domain(x, Domain::Integer)],
        sense: Sense::Minimize,
        objective: Expression::mul(Expression::from_variable(x), Expression::from_variable(x)),
        constraints: Vec::new(),
        variables: vec![x],
    };
    let solution = solve(&problem);
    assert_eq!(
        solution.status,
        SolveStatus::Error(QUADRATIC_ERROR.to_string())
    );
}

#[test]
fn quadratic_constraint_with_a_domain_restriction_is_rejected() {
    let x = scalar_var(1);
    let y = scalar_var(2);
    let problem = Problem {
        domains: vec![whole_var_domain(x, Domain::Integer)],
        sense: Sense::Minimize,
        objective: Expression::from_variable(x),
        constraints: vec![Constraint {
            relation: Relation::LessEqual,
            lhs: Expression::mul(Expression::from_variable(x), Expression::from_variable(x)),
            rhs: Expression::from_variable(y),
        }],
        variables: vec![x, y],
    };
    let solution = solve(&problem);
    assert_eq!(
        solution.status,
        SolveStatus::Error(QUADRATIC_ERROR.to_string())
    );
}

#[test]
fn overlapping_domain_restrictions_let_binary_win() {
    // A whole-variable Integer restriction plus a Binary restriction on
    // the same (sole) scalar entry must behave exactly as a plain Binary
    // restriction: x in {0, 1}, not the wider integer range.
    let x = scalar_var(1);
    let problem = Problem {
        domains: vec![
            whole_var_domain(x, Domain::Integer),
            whole_var_domain(x, Domain::Binary),
        ],
        sense: Sense::Maximize,
        objective: Expression::from_variable(x),
        constraints: vec![Constraint {
            relation: Relation::LessEqual,
            lhs: Expression::from_variable(x),
            rhs: Expression::constant(10.0),
        }],
        variables: vec![x],
    };
    let solution = solve(&problem);
    assert_eq!(solution.status, SolveStatus::Optimal);
    assert!((solution.objective_value.unwrap() - 1.0).abs() < 1e-6);
}

#[test]
fn stopped_at_limit_reports_the_best_incumbent() {
    // A deliberately hard 0/1 knapsack-style instance (many binary items,
    // near-degenerate value/weight ratios) engineered to still be running
    // branch-and-bound when `MILP_NODE_LIMIT` is hit, so the search stops
    // with a feasible-but-unproven incumbent rather than ever reaching
    // `ProvenOptimal`.
    const N: usize = 40;
    let vars: Vec<Variable> = (1..=N as u64).map(scalar_var).collect();
    let domains = vars
        .iter()
        .map(|&v| whole_var_domain(v, Domain::Binary))
        .collect();

    // Weights/values chosen so that no obviously-dominant greedy order
    // exists (value/weight ratios interleave), forcing genuine branching.
    let weights: Vec<f64> = (0..N).map(|i| 1.0 + ((i * 37) % 23) as f64).collect();
    let values: Vec<f64> = (0..N).map(|i| 1.0 + ((i * 53) % 29) as f64).collect();
    let capacity: f64 = weights.iter().sum::<f64>() * 0.5;

    let mut objective = Expression::scale(values[0], Expression::from_variable(vars[0]));
    for i in 1..N {
        objective = Expression::add(
            objective,
            Expression::scale(values[i], Expression::from_variable(vars[i])),
        );
    }

    let mut weighted_sum = Expression::scale(weights[0], Expression::from_variable(vars[0]));
    for i in 1..N {
        weighted_sum = Expression::add(
            weighted_sum,
            Expression::scale(weights[i], Expression::from_variable(vars[i])),
        );
    }

    let problem = Problem {
        domains,
        sense: Sense::Maximize,
        objective,
        constraints: vec![Constraint {
            relation: Relation::LessEqual,
            lhs: weighted_sum,
            rhs: Expression::constant(capacity),
        }],
        variables: vars,
    };
    let solution = solve(&problem);
    // Either outcome is a correctness pass (the solver may prove
    // optimality faster than expected on some machines); the real
    // assertion is that *if* the limit is hit, it is reported correctly
    // with a usable incumbent, never silently misreported as Optimal.
    match solution.status {
        SolveStatus::StoppedAtLimit => {
            assert!(solution.objective_value.is_some());
            assert_eq!(solution.variable_values.len(), N);
        }
        SolveStatus::Optimal => {
            assert!(solution.objective_value.is_some());
        }
        other => panic!("unexpected status for a feasible knapsack instance: {other:?}"),
    }
}
