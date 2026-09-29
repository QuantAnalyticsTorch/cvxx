---
id: SPEC-0001
title: Release pipeline and versioning for cvxx binaries
issue: ISSUE-0001
status: draft
created: 2026-09-29
---

## Objective

Define and implement the versioning strategy, local build process, GitHub Actions validation, and GitHub Release publishing pipeline for the `cvxx` Excel add-in binaries.

## Non-Objective

- No code signing.
- No automated XLAM build in CI.
- No 32-bit Excel support.
- No nightly/unstable Rust features.

## Interface

### Version source of truth

- Canonical version lives in `Cargo.toml` under `package.version`.
- A `version.txt` file is generated at repository root during the build for non-Rust tooling to read.
- The XLAM ribbon label should display the same version (to be updated manually when the XLAM is regenerated).

### Public release artifacts

| File | Purpose |
|---|---|
| `cvxx.xll` | 64-bit Excel XLL produced by `cargo build --release` |
| `cvxx.xlam` | Manually built ribbon add-in, committed under `assets/` |
| `SHA256SUMS.txt` | Checksums for both artifacts |

### Git tags

- Format: `v{major}.{minor}.{patch}` with optional pre-release suffix, e.g. `v0.5.0-beta.1`.
- Annotated tags must match `Cargo.toml` `package.version` exactly.

## Data Model

N/A. This specification covers process and tooling, not runtime data structures.

## Error Handling

- CI must fail if `Cargo.toml` version does not match the pushed git tag.
- CI must fail if `cargo fmt`, `cargo clippy`, or `cargo test` reports errors.
- CI must warn (but not fail) if `version.txt` is out of sync.

## Test Approach

- Manual: run `cargo build --release` on a clean Windows machine and confirm `target/release/cvxx.xll` loads in 64-bit Excel.
- CI: validate formatting, linting, tests, and tag/version alignment on every push and pull request.
- Manual: create a pre-release tag and verify the GitHub Release draft contains the expected artifacts.

## Dependencies

- GitHub repository already exists.
- Rust toolchain (`stable`, `x86_64-pc-windows-msvc`) installed locally.
- Excel 2016 or later 64-bit for manual XLAM and XLL validation.

## Implementation Notes

1. Add a `build.rs` or a small script that writes `version.txt` from `Cargo.toml`.
2. Create `.github/workflows/ci.yml` with the validation jobs.
3. Create `.github/workflows/release.yml` that triggers on version tags, creates a GitHub Release draft, and attaches artifacts. Since binaries are built locally, the workflow may be limited to drafting the release and checking artifact presence, or it may build the XLL in CI but still allow a maintainer to override with a signed/local artifact. For this project, keep CI validation-only and let the maintainer upload the locally built artifacts.
4. Update `README.md` with install instructions for unsigned add-ins.
5. Place `cvxx.xlam` under `assets/` once it is created.
