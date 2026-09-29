# Skill: cvxx-optimization-modeling

## Purpose

Capture domain knowledge for convex optimization in `cvxx`.

## Guidelines

1. **Problem formulation**
   - Accept objective, variables, constraints, and parameters from Excel ranges.
   - Validate that the problem is convex or disciplined convex as appropriate.
   - Reject ambiguous formulations with clear error messages.

2. **Variable types**
   - Support continuous variables first.
   - Plan for integer / binary variables as later enhancements.
   - Represent variable bounds explicitly.

3. **Constraint representation**
   - Support linear equality and inequality constraints.
   - Support second-order cone and semidefinite constraints if `cvxrust` allows.
   - Use a normalized internal representation.

4. **Solver invocation**
   - Delegate numerical solving to `cvxrust`.
   - Handle solver status: optimal, infeasible, unbounded, numerical failure.
   - Return dual values or sensitivity information only when requested.

5. **Excel mapping**
   - Map Excel ranges to matrices/vectors clearly.
   - Document orientation (row-major vs. column-major).
   - Provide helper functions for common problem templates.
