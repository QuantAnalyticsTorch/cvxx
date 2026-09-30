# Architecture Decisions

This document records significant architectural decisions for `cvxx`. Each
entry follows the architect note format; see `docs/architecture/` for none
yet split out as standalone notes.

---
date: 2026-09-30
topic: Adopt clarabel as the standing solver framework
status: accepted
---

## Observation

`ISSUE-0010` / `SPEC-0010` needed to replace `cvxrust::solve`'s permanent
`"not implemented"` stub with a real solver. The initial draft of SPEC-0010
proposed a hand-rolled two-phase simplex implementation living entirely
inside the `cvxrust` placeholder crate, with no new external dependency.

## Recommendation

Use [`clarabel`](https://crates.io/crates/clarabel), a pure-Rust conic
solver, as the numerical engine behind `cvxrust::solve`, rather than
hand-rolling and maintaining a simplex implementation. `clarabel` natively
supports free (unrestricted-sign) variables and both linear and quadratic
programs, so the same `Problem` → conic-program translation layer
(`P`/`q`/`A`/`b`/cones) built for this issue's linear-programming scope is
the intended foundation for quadratic-objective support later, rather than
a second, unrelated solver integration.

`clarabel` is a dependency of `cvxrust` only; `cvxx` continues to depend
solely on `cvxrust`'s existing public API and must never import `clarabel`
directly, preserving the layered boundary in `copilot-instructions.md`
("Optimization layer: `cvxrust` problem formulation and solvers").

## Rationale

- Avoids maintaining bespoke simplex/anti-cycling logic (Bland's rule,
  degenerate pivoting, free-variable splitting) that a real solver already
  handles correctly and efficiently.
- Establishes one solver framework for the whole roadmap (LP now, QP next)
  instead of adopting a different crate per problem class.
- Pure Rust, no C/BLAS/LAPACK toolchain requirement for the cone types this
  project currently needs (`ZeroConeT`, `NonnegativeConeT`), keeping the
  Windows XLL build simple.

## Affected Components

- `specifications/0010-convex-solver-backend.md` (updated to specify
  `clarabel` instead of a hand-rolled simplex).
- `cvxrust/Cargo.toml` (new `clarabel` dependency).
- `cvxrust/src/lib.rs` (`solve` implementation, `Variable` identity fix).
- `.github/copilot-instructions.md`, `.github/agents/cvxx-developer.agent.md`,
  `.github/skills/cvxx-optimization-modeling/SKILL.md`, `README.md` (updated
  to document `clarabel` as the standing solver framework).
- Future specifications extending problem-class support (e.g. quadratic
  objectives) should extend this `clarabel` translation layer rather than
  introduce another solver crate.
