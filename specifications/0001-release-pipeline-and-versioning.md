---
id: SPEC-0001
title: Release pipeline and versioning for cvxx binaries
issue: ISSUE-0001
status: implemented
created: 2026-09-29
---

## Objective

Define and implement the versioning strategy, local build process, GitHub Actions validation, offline documentation build, and GitHub Release publishing pipeline for the `cvxx` Excel add-in release archive.

## Non-Objective

- No code signing.
- No automated XLAM build in CI.
- No 32-bit Excel support.
- No nightly/unstable Rust features.
- No MSI/EXE installer; the release artifact is a plain `.zip` archive.
  `install.ps1`/`install.bat` are optional convenience scripts bundled
  inside that archive (unblocking files, registering a Trusted Location),
  not a standalone installer; they never run automatically and never
  perform the Excel add-in registration itself.
- No client-side analytics, telemetry, or network calls from the rendered documentation site.

## Interface

### Version source of truth

- Canonical version lives in `Cargo.toml` under `package.version`.
- A `version.txt` file is generated at repository root during the build for non-Rust tooling to read.
- The XLAM ribbon label should display the same version (to be updated manually when the XLAM is regenerated).

### Public release artifacts

A single `cvxx-{version}.zip` archive is attached to each GitHub Release, containing:

| Path in archive | Purpose |
|---|---|
| `cvxx.xll` | 64-bit Excel XLL produced by `cargo build --release` |
| `cvxx.xlam` | Manually built ribbon add-in, committed under `assets/` |
| `docs/html/` | Static HTML docs rendered from `docs/*.md`, with a shared stylesheet and a small bundled JS file for navigation/search; no external assets or network calls |
| `docs/examples/` | Example workbooks/notebooks, copied as-is from `docs/examples/` |
| `INSTALL.md` | Install/trust instructions for unsigned add-ins, copied from the repository |
| `install.ps1` | Optional one-click helper that unblocks every extracted file (removes Mark of the Web) and can register the folder as an Excel Trusted Location; never touches add-in registration itself |
| `install.bat` | Double-clickable wrapper around `install.ps1` for users uncomfortable running PowerShell scripts directly |
| `SHA256SUMS.txt` | Checksums for every file in the archive |

### Git tags

- Format: `v{major}.{minor}.{patch}` with optional pre-release suffix, e.g. `v0.5.0-beta.1`.
- Annotated tags must match `Cargo.toml` `package.version` exactly.

### Documentation build

- `docs/*.md` remains the single authored source and continues to render as-is on GitHub; no change to its format is required.
- A static-site generator (`mdbook`, installed as a pinned CI tool, no vendored binary committed) renders `docs/*.md` into `docs/html/` via a `docs/book.toml` / `docs/SUMMARY.md` that lists the existing pages.
- The generated site must work when opened directly from disk (`file://`): no `fetch`/XHR of local assets, no CDN-hosted CSS/JS/fonts, search index inlined into a bundled script.
- The `cvxx.xlam` ribbon's help links point at relative paths into `docs/html/` so the same archive layout works regardless of install location.

## Data Model

N/A. This specification covers process and tooling, not runtime data structures.

## Error Handling

- CI must fail if `Cargo.toml` version does not match the pushed git tag.
- CI must fail if `cargo fmt`, `cargo clippy`, or `cargo test` reports errors.
- CI must warn (but not fail) if `version.txt` is out of sync.
- CI must fail if the `mdbook` build reports broken internal links or fails to render any page under `docs/`.
- Archive assembly must fail (not silently skip) if `docs/html/` or `docs/examples/` is missing or empty at packaging time.

## Test Approach

- Manual: run `cargo build --release` on a clean Windows machine and confirm `target/release/cvxx.xll` loads in 64-bit Excel.
- CI: validate formatting, linting, tests, and tag/version alignment on every push and pull request.
- CI: build `docs/html/` and verify the output directory is non-empty and contains an `index.html`.
- Manual: open `docs/html/index.html` directly from disk (no local web server) and confirm navigation, styling, and search all work without network access.
- Manual: create a pre-release tag and verify the GitHub Release draft contains a single `.zip` archive with the expected internal layout.

## Dependencies

- GitHub repository already exists.
- Rust toolchain (`stable`, `x86_64-pc-windows-msvc`) installed locally.
- Excel 2016 or later 64-bit for manual XLAM and XLL validation.
- `mdbook` (or equivalent static-site generator) available in CI for the documentation build.

## Implementation Notes

1. Add a `build.rs` or a small script that writes `version.txt` from `Cargo.toml`.
2. Create `.github/workflows/ci.yml` with the validation jobs, including the `mdbook` build of `docs/html/`.
3. Add `docs/book.toml` and `docs/SUMMARY.md` so `mdbook build` renders the existing `docs/*.md` pages without restructuring them.
4. Create `.github/workflows/release.yml` that triggers on version tags, creates a GitHub Release draft, assembles the `cvxx-{version}.zip` archive (XLL, XLAM, `docs/html/`, `docs/examples/`, `INSTALL.md`, checksums), and attaches it. Since the XLL/XLAM are built/placed locally, the workflow may be limited to drafting the release and validating archive contents, or it may build the XLL and rendered docs in CI but still allow a maintainer to override with a locally built artifact. For this project, keep CI validation-only and let the maintainer upload the locally assembled archive.
5. Update `README.md` / add `INSTALL.md` with install instructions for unsigned add-ins and for extracting the release archive.
6. Place `cvxx.xlam` under `assets/` once it is created.
7. Point the `cvxx.xlam` ribbon help buttons at relative `docs/html/...` paths so they resolve both from the release archive and a local checkout.
