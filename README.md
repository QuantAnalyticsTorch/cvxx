# cvxx

`cvxx` brings disciplined convex optimization straight into Excel. Define variables, expressions, and constraints with simple `CVX.*` worksheet formulas, solve the resulting problem, and read the results back into your spreadsheet — no external solver, no copy-pasting results from another tool.

## Getting Started

1. **Download the latest release.** Once `cvxx` reaches `v1.0.0`, each [GitHub Release](../../releases) publishes a single `cvxx-{version}.zip` archive containing everything you need: `cvxx.xll`, `cvxx.xlam`, offline documentation, and example workbooks.
2. **Extract the zip** to a folder of your choice.
3. **Unblock the files** (Windows marks downloaded files as untrusted): in PowerShell, `Get-ChildItem -Recurse | Unblock-File`.
4. **Load the add-ins in Excel**: `File` → `Options` → `Add-ins` → `Manage: Excel Add-ins` → `Go…` → `Browse…` and select both `cvxx.xll` and `cvxx.xlam` from the extracted folder.
5. **Open an example workbook** from the `docs/examples/` folder, or click the `cvxx` ribbon tab's **Examples** button to open one directly from Excel.
6. **Browse the documentation** via the ribbon's **Help** button (opens the bundled offline docs), or read it online under [`docs/`](docs/).

> Pre-`v1.0.0`, there is no published release yet — see [Building From Source](#building-from-source) below.

## What You Get

- `CVX.VARIABLE`, `CVX.PARAMETER`, `CVX.EXPRESSION`, and `CVX.CONSTRAINT` to build up a problem from worksheet cells and ranges.
- `CVX.PROBLEM` / `CVX.SOLVE` to assemble and solve it, backed by [`clarabel`](https://crates.io/crates/clarabel), a pure-Rust conic (LP/QP) solver.
- `CVX.STATUS`, `CVX.OBJECTIVE_VALUE`, `CVX.VALUE`, and `CVX.DESCRIBE` to read back solve status, objective value, and variable values.
- A `cvxx` ribbon tab with example workbooks, offline help, and (optionally) support/feedback links.

See [docs/](docs/) for the full function reference, or the ribbon's **Help** button for the same content offline.

## Building From Source

> This section will be expanded once the release pipeline (`ISSUE-0001`) is complete. For now, build the XLL locally with `cargo build --release` (produces `target/release/cvxx.xll`) and build `cvxx.xlam` following [.github/skills/cvxx-excel-ribbon-xlam/SKILL.md](.github/skills/cvxx-excel-ribbon-xlam/SKILL.md).

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
│   └── copilot-instructions.md
├── issues/                  # Business requirements (created by the business analyst agent)
├── specifications/          # Technical specifications (created by the technical analyst agent)
├── docs/                    # User and developer documentation, example notebooks
├── xlam/                    # Reviewable ribbon XML / VBA source and build tooling for cvxx.xlam
├── assets/                  # Committed binary artifacts (cvxx.xlam)
├── src/                     # Rust source code
│   ├── excel/               # XLL registration and XLOPER12 adapters
│   ├── data/                # Excel-to-Rust data parsing and validation
│   ├── core/                # Handle registry, identifiers, object lifetimes
│   └── analytics/           # cvxrust problem construction and solvers
├── tests/                   # Rust and integration tests (to be added)
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

