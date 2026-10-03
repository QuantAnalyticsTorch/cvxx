# cvxx

`cvxx` brings disciplined convex optimization straight into Excel. Define variables, expressions, and constraints with simple `CVX.*` worksheet formulas, solve the resulting problem, and read the results back into your spreadsheet — no external solver, no copy-pasting results from another tool.

## Getting Started

1. **Download the latest release.** Once `cvxx` reaches `v1.0.0`, each [GitHub Release](../../releases) publishes a single `cvxx-{version}.zip` archive containing everything you need: `cvxx.xll`, `cvxx.xlam`, offline documentation, and example workbooks.
2. **Extract the zip** to a folder of your choice.
3. **Unblock the files** (Windows marks downloaded files as untrusted): in PowerShell, `Get-ChildItem -Recurse | Unblock-File`.
4. **Load the add-ins in Excel**: `File` → `Options` → `Add-ins` → `Manage: Excel Add-ins` → `Go…` → `Browse…` and select both `cvxx.xll` and `cvxx.xlam` from the extracted folder.
5. **Open an example workbook** from the `docs/examples/` folder, or click the `cvxx` ribbon tab's **Examples** button to open one directly from Excel.
6. **Browse the documentation** via the ribbon's **Help** button (opens the bundled offline docs), or read it online under [`docs/`](docs/).

See [INSTALL.md](INSTALL.md) for the full install/trust walkthrough, including how to unblock unsigned add-ins and verify the archive's `SHA256SUMS.txt`.

> Pre-`v1.0.0`, there is no published release yet — see [Building From Source](#building-from-source) below.

## What You Get

- `CVX.VARIABLE`, `CVX.PARAMETER`, `CVX.EXPRESSION`, and `CVX.CONSTRAINT` to build up a problem from worksheet cells and ranges.
- `CVX.PROBLEM` / `CVX.SOLVE` to assemble and solve it, backed by [`clarabel`](https://crates.io/crates/clarabel), a pure-Rust conic (LP/QP) solver.
- `CVX.STATUS`, `CVX.OBJECTIVE_VALUE`, `CVX.VALUE`, and `CVX.DESCRIBE` to read back solve status, objective value, and variable values.
- A `cvxx` ribbon tab with example workbooks, offline help, and (optionally) support/feedback links.

See [docs/](docs/) for the full function reference, or the ribbon's **Help** button for the same content offline.

## Building From Source

### Versioning

The canonical version lives in `Cargo.toml`'s `package.version`. A `build.rs`
step regenerates `version.txt` at the repository root from that value on
every `cargo build`, so non-Rust tooling (the packaging script below) can
read the version without invoking `cargo`. Release tags use
`v{major}.{minor}.{patch}` (optionally `-prerelease`, e.g. `v0.5.0-beta.1`)
and must match `Cargo.toml` exactly — CI enforces this on every tagged push.

### Build the XLL

```powershell
cargo build --release   # produces target/release/cvxx.dll
```

An Excel XLL is just a DLL with the exports Excel's C API expects, renamed
with a `.xll` extension — `scripts/package-release.ps1` (below) copies
`cvxx.dll` to `cvxx.xll` automatically when assembling the release archive;
there is no separate manual rename step.

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, and `cargo test --workspace` must all pass; the same three
commands run in [`.github/workflows/ci.yml`](.github/workflows/ci.yml) on
every push and pull request, on a `windows-latest` GitHub-hosted runner
(target: `x86_64-pc-windows-msvc` only).

### Build `cvxx.xlam`

Follow [.github/skills/cvxx-excel-ribbon-xlam/SKILL.md](.github/skills/cvxx-excel-ribbon-xlam/SKILL.md)
and commit the result under `assets/cvxx.xlam`. The XLAM is built manually
in Excel, never in CI.

### Render the offline docs

```powershell
mdbook build docs   # produces docs/html/ from docs/*.md (docs/book.toml, docs/SUMMARY.md)
```

`docs/html/` is a generated, git-ignored build artifact (same as `target/`),
rebuilt by CI (`.github/workflows/ci.yml`) and by the packaging step below.
It renders `docs/*.md` into a styled, searchable static site that opens
directly from disk — no local web server or network access required.

### Assemble a release archive

Once you have built `target/release/cvxx.dll`, `assets/cvxx.xlam`, and
`docs/html/`, assemble the `cvxx-{version}.zip` release archive (with a
`SHA256SUMS.txt` checksum file) with:

```powershell
.\scripts\package-release.ps1
```

This fails loudly if any required input (XLL, XLAM, `docs/html/`,
`docs/examples/`, `INSTALL.md`) is missing or empty, and writes
`dist\cvxx-{version}.zip`. Pushing a `v{version}` tag runs
[`.github/workflows/release.yml`](.github/workflows/release.yml), which
validates the tag against `Cargo.toml`, re-runs the full CI validation
(including the docs build), and drafts a GitHub Release for the version —
attach the locally assembled archive to that draft and publish it. No step
of the pipeline builds the XLAM or signs any binary.

## Project Goals

- Make disciplined convex optimization available directly from Excel formulas.
- Keep all numerical work in safe, performant Rust code.
- Provide a polished Excel user experience (ribbon, help, examples).
- Maintain a clear separation between business requirements, technical specifications, and implementation.

## Architecture at a Glance

Excel cannot receive Rust objects directly, so the add-in uses a handle-based object model. Complex objects (variables, expressions, problems, results) live in an in-memory Rust registry and are referenced from Excel by opaque string handles such as `"cvx:var:7f3a..."`. Functions either operate on those handles or return new ones.

```
┌──────────────────────────────────────────────────────────────┐
│  Excel (xlsm / xlsx / xlam ribbon)                           │
├──────────────────────────────────────────────────────────────┤
│  cvxx.xlam  ──►  VBA / JS helpers, ribbon callbacks          │
├──────────────────────────────────────────────────────────────┤
│  cvxx.xll  ──►  xladd / XLOPER12  ──►  string handles        │
├──────────────────────────────────────────────────────────────┤
│  Rust crate                                                  │
│  ├─ excel/  ──►  function registration, XLOPER12 adapters    │
│  ├─ data/   ──►  Excel range parsing & shape normalization   │
│  ├─ core/   ──►  handle registry, identifiers, lifetimes     │
│  └─ analytics/  ──►  cvxrust problem building & solving       │
└──────────────────────────────────────────────────────────────┘
```

## Repository Layout

```
cvxx/
├── .github/
│   ├── agents/              # AI agent definitions
│   ├── skills/              # Shared skill instructions
│   ├── workflows/           # CI (ci.yml) and release-draft (release.yml) GitHub Actions
│   └── copilot-instructions.md
├── issues/                  # Business requirements (created by the business analyst agent)
├── specifications/          # Technical specifications (created by the technical analyst agent)
├── docs/                    # User and developer documentation, example notebooks
│   ├── book.toml            # mdbook config (renders docs/*.md into docs/html/)
│   ├── SUMMARY.md           # mdbook table of contents
│   └── html/                # Generated, git-ignored offline docs site (not committed)
├── scripts/                 # Release/versioning tooling (PowerShell)
│   ├── package-release.ps1       # Assembles dist/cvxx-{version}.zip
│   ├── check-version-sync.ps1    # Warns if version.txt is stale vs. Cargo.toml
│   ├── check-tag-matches-version.ps1  # Fails if a release tag != Cargo.toml version
│   └── check-docs-links.ps1      # Fails on broken docs/*.md links or unrendered pages
├── xlam/                    # Reviewable ribbon XML / VBA source and build tooling for cvxx.xlam
├── assets/                  # Committed binary artifacts (cvxx.xlam)
├── src/                     # Rust source code
│   ├── excel/               # XLL registration and XLOPER12 adapters
│   ├── data/                # Excel-to-Rust data parsing and validation
│   ├── core/                # Handle registry, identifiers, object lifetimes
│   └── analytics/           # cvxrust problem construction and solvers
├── tests/                   # Rust and integration tests (to be added)
├── build.rs                 # Regenerates version.txt from Cargo.toml on every build
├── version.txt              # Generated, committed copy of the current version
├── INSTALL.md               # Install/trust instructions for unsigned add-ins
├── README.md                # This file
└── LICENSE
```

## Development Workflow

1. Business requirements are captured as Markdown issues in `issues/`.
2. Each issue is refined into one or more technical specifications in `specifications/`.
3. Specifications are implemented by the developer agent as Rust code, tests, and documentation.
4. The architect agent continuously reviews the overall architecture and consistency.

## License

This project is licensed under the GNU General Public License v3.0. See [LICENSE](LICENSE) for details.

