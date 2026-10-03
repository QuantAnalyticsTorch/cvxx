# Copilot Instructions for cvxx

This document defines the core architecture and Rust coding standards for the `cvxx` project. All contributors and agents should follow these guidelines.

## Project Mission

Build an Excel add-in that exposes `cvxrust` convex optimization to Excel via a Rust XLL built with `xladd`, plus an `xlam` ribbon add-in for documentation and examples.

## Architectural Principles

1. **Layered boundaries**
   - Excel UI layer: `xlam` ribbon, VBA/JS helpers, HTML help.
   - Excel C API layer: Rust XLL using `xladd` / `XLOPER12`.
   - Optimization layer: `cvxrust` problem formulation and solvers, numerically backed by `clarabel` (the standing conic solver framework for this project; see `docs/architecture.md`).
   - Never let UI code call solver internals directly; always go through the XLL function surface.
   - `clarabel` is an implementation detail of `cvxrust`; `cvxx` never depends on it directly.

2. **Memory safety**
   - All code touching the Excel C API must be Rust `unsafe` only in thin, reviewed wrappers.
   - Prefer owned data; avoid lifetime gymnastics across the FFI boundary.

3. **Error propagation**
   - Errors from Rust must be translated into Excel-visible values (`#VALUE!`, `#NUM!`, custom error strings).
   - Log detailed diagnostics to a file or debug channel, not to the worksheet.

4. **Testability**
   - Pure Rust optimization logic must be unit-testable without Excel.
   - XLL behavior must be covered by integration tests with sample workbooks.

5. **Documentation**
   - Every public XLL function needs: purpose, argument list, return value, error conditions, and an example.
   - User-facing docs live in `docs/`; API docs use `rustdoc`.

## Rust Coding Standards

- Edition: 2021 (upgrade to 2024 when stable and beneficial).
- `cargo clippy` and `cargo fmt` must pass.
- Avoid `unwrap()` and `expect()` in production paths; use `Result` and `thiserror`/`anyhow`.
- Use `tracing` for structured logging.
- Public APIs must be typed; avoid raw `XLOPER12` leaking outside the `excel` adapter module.
- Keep modules small and single-responsibility:
  - `excel/` — XLL registration and `XLOPER12` adapters; the only module that speaks Excel.
  - `data/` — Excel range parsing, shape validation, and normalization into Rust types.
  - `core/` — handle registry, opaque identifiers, object lifetime management, and shared error types.
  - `analytics/` — `cvxrust` problem construction, solver invocation, and result extraction.
- Excel cannot receive Rust objects. Complex state is stored in a thread-safe handle registry and surfaced to Excel as opaque string handles (e.g., `cvx:var:<uuid>`).
- Convex problems are solved by translating them into a conic program and delegating to `clarabel` inside `cvxrust`. Extending problem-class support (e.g., quadratic objectives) means widening this translation layer, not introducing another solver crate.

## Excel Integration Conventions

- Use `xladd` for function registration and `XLOPER12` manipulation.
- Function names exposed to Excel should be prefixed with `CVX.` (e.g., `CVX.SOLVE`).
- Support both array formulas and scalar call patterns where sensible.
- Complex objects are referenced from Excel via opaque string handles produced by the `core` registry.
- Functions that operate on handles validate them and return `#VALUE!` for unknown or expired handles.
- Thread-safe calculation is required; respect Excel's multi-threaded recalculation rules.

## XLAM Ribbon Conventions

- Ribbon XML must be packaged inside `cvxx.xlam`.
- Help content is served as static HTML from `docs/html/`.
- Example notebooks are placed in `docs/examples/` and linked from the ribbon.

## Documentation Standards

- Markdown for prose; XML-based notebooks where interactive examples are required.
- Every specification in `specifications/` must trace back to an issue in `issues/`.
- Every specification must identify affected user-facing documentation and
  examples, or explain why it has no user-visible documentation impact.
- Implementations must update the affected `docs/` pages and examples in the
  same change as the behavior they describe.
- The Markdown under `docs/` is the canonical user documentation;
  `docs/html/` is generated with mdBook and must not be hand-edited.
- CI builds the book, validates links, and checks that every exported
  `CVX.*` function is mentioned in the documentation. This coverage check
  complements, but does not replace, review of accuracy, arguments, results,
  errors, and examples.
- All code changes must include tests and a `docs/` update if user-visible.

## Agent Responsibilities

| Agent | Output | Scope |
|---|---|---|
| `cvxx-business-analyst` | `issues/*.md` | Requirements only; no code |
| `cvxx-technical-analyst` | `specifications/*.md` | Specs only; no code |
| `cvxx-developer` | Rust code, tests, docs | Implements specs |
| `cvxx-architect` | Architecture reviews | Cross-cutting consistency |

When in doubt, prefer explicit interfaces and small, well-named modules.
