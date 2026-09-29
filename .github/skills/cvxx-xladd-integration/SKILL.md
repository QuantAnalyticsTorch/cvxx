# Skill: cvxx-xladd-integration

## Purpose

Describe how to integrate with Excel using `xladd` and the `XLOPER12` C API.

## Context

- Excel calls XLL functions via the C API.
- `xladd` provides Rust bindings and helpers for registration and `XLOPER12` manipulation.
- Thread safety matters because Excel can recalculate in parallel.

## Guidelines

1. **Function registration**
   - Register each function with a unique Excel name, typically prefixed `CVX.`.
   - Declare argument types and return types accurately.
   - Provide help text for the function wizard.
   - Observe Excel's 255-character limit for the function help string and for each argument description. Keep descriptions concise; put detailed documentation in `docs/` and `rustdoc`, not in the registration strings.
   - Validate the length of every help string and argument description at build or test time to avoid silent truncation in Excel.

2. **Argument handling**
   - Convert `XLOPER12` inputs to Rust types as early as possible.
   - Validate shapes (scalars, vectors, matrices) before passing to solver code.
   - Return `#VALUE!` for malformed inputs.

3. **Return values**
   - Return `XLOPER12` from the adapter layer only.
   - Map solver status to Excel values:
     - Optimal → result array
     - Infeasible → custom error string or `#NUM!`
     - Solver failure → `#VALUE!` with logged details

4. **Thread safety**
   - Do not mutate global state during calculation.
   - Use read-only shared data or thread-local storage if needed.

5. **Debugging**
   - Use `OutputDebugString` or `tracing` with a file appender.
   - Never block Excel with message boxes during calculation.
