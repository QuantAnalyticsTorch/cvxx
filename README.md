# cvxx

`cvxx` is an Excel add-in that exposes convex optimization capabilities to Excel worksheets through a Rust implementation built on top of `cvxrust`. It consists of:

- A Rust dynamic-link library (DLL/XLL) that registers optimization functions with Excel via [`xladd`](https://github.com/MathiasPius/xladd) and the `XLOPER12` API.
- An Excel macro-enabled add-in (`cvxx.xlam`) that provides a ribbon tab with documentation links, examples, and helper utilities.
- Documentation, example workbooks, and narrative notebooks stored under `docs/`.

## Goals

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
├── src/                     # Rust source code (to be added)
│   ├── excel/               # XLL registration and XLOPER12 adapters
│   ├── data/                # Excel-to-Rust data parsing and validation
│   ├── core/                # Handle registry, identifiers, object lifetimes
│   └── analytics/           # cvxrust problem construction and solvers
├── tests/                   # Rust and integration tests (to be added)
├── README.md                # This file
└── LICENSE
```

## Workflow

1. Business requirements are captured as Markdown issues in `issues/`.
2. Each issue is refined into one or more technical specifications in `specifications/`.
3. Specifications are implemented by the developer agent as Rust code, tests, and documentation.
4. The architect agent continuously reviews the overall architecture and consistency.

## Getting Started

> This section will be expanded once the initial build infrastructure is in place.

## License

This project is licensed under the GNU General Public License v3.0. See [LICENSE](LICENSE) for details.

