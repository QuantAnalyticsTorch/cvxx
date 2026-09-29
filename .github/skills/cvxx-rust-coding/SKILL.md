# Skill: cvxx-rust-coding

## Purpose

Define idiomatic, safe Rust practices for the `cvxx` project.

## Guidelines

1. **Project setup**
   - Use `cargo` workspaces if the project grows beyond one crate.
   - Keep `Cargo.toml` dependencies minimal and pinned to compatible versions.

2. **Unsafe code**
   - Isolate `unsafe` blocks in thin adapter modules.
   - Document the safety invariant for every `unsafe` block.
   - Avoid `unsafe` in general business logic.

3. **Error handling**
   - Define a project-wide error enum with `thiserror`.
   - Use `Result<T, CvxxError>` in fallible functions.
   - Map errors to Excel error values at the XLL boundary.

4. **Types and APIs**
   - Prefer strong types over raw `XLOPER12` in business logic.
   - Use `From`/`TryFrom` for conversions.
   - Keep public APIs small and stable.

5. **Testing**
   - Write unit tests for pure functions.
   - Use `rstest` or table-driven tests for multiple cases.
   - Keep tests deterministic and fast.

6. **Formatting and linting**
   - Run `cargo fmt` and `cargo clippy -- -D warnings` before committing.
