# cvxx

`cvxx` brings disciplined convex optimization straight into Excel. Define
parameters, variables, expressions, and constraints with `CVX.*` worksheet
formulas, solve the resulting problem, and read the results back into your
spreadsheet.

This offline copy of the documentation is bundled with every
[release archive](https://github.com/QuantAnalyticsTorch/cvxx/releases) and
opens directly from disk (no network access is required) via the `cvxx`
ribbon's **Help** button. The same Markdown source also renders on
[GitHub](https://github.com/QuantAnalyticsTorch/cvxx/tree/main/docs).

## Function reference

- [Parameters](parameters.md) — `CVX.PARAMETER`
- [Variables](variables.md) — `CVX.VARIABLE`
- [Expressions](expressions.md) — `CVX.EXPRESSION` and the functional builders
- [Constraints](constraints.md) — `CVX.CONSTRAINT`, `CVX.CONSTRAINTS`, and the
  relational functional builders
- [Problems & Solving](problems.md) — `CVX.MINIMIZE`/`CVX.MAXIMIZE`,
  `CVX.PROBLEM`, `CVX.SOLVE`
- [Inspecting Results](inspection.md) — `CVX.VALUE`, `CVX.STATUS`,
  `CVX.OBJECTIVE_VALUE`, `CVX.DESCRIBE`, `CVX.SHAPE`, `CVX.TYPE`

## Architecture

- [Architecture Decisions](architecture.md) — the project's layered design
  and significant architecture decisions.

## Examples

Example workbooks live under `docs/examples/` alongside this documentation
(also reachable from the ribbon's **Examples** button).
