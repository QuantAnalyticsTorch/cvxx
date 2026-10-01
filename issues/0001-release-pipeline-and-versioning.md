---
id: ISSUE-0001
title: Release pipeline and versioning for cvxx binaries
priority: must
status: draft
created: 2026-09-29
---

## Background

`cvxx` is an Excel add-in composed of a Rust XLL (`cvxx.xll`) and an Excel macro-enabled add-in (`cvxx.xlam`). Once the project reaches version `1.0.0`, the binaries must be published on GitHub so users can download and install them. Before that, the project needs a clear versioning strategy and a lightweight release process that matches the team's constraints.

User documentation (currently authored as per-function Markdown pages under `docs/`) must also reach end users. The chosen approach is a combination: `docs/*.md` remains the single authored source, published as-is on GitHub for browsing/contribution, and additionally rendered to a static HTML site (with CSS and minimal JS for navigation/search) that ships offline alongside the add-in so the `cvxx.xlam` ribbon's help links work without network access.

## Goal

Establish a versioning and release pipeline that:

1. Keeps the XLL, XLAM, and bundled documentation versions in lockstep.
2. Produces reproducible local builds of the XLL on 64-bit Windows.
3. Publishes GitHub Releases with a downloadable release archive containing `cvxx.xll`, `cvxx.xlam`, rendered offline docs, and example workbooks, from version `1.0.0` onward.
4. Documents the install steps for unsigned add-ins.

## Acceptance Criteria

- [ ] A single source of truth for the project version exists and is documented.
- [ ] The Rust XLL builds locally for 64-bit Excel with a documented one-line command.
- [ ] A GitHub Actions workflow validates every PR and tag (`cargo test`, `cargo clippy`, `cargo fmt`, and tag/version alignment).
- [ ] `docs/*.md` is rendered into a static, styled HTML site (CSS + optional minimal JS for navigation/search, no network calls required) under `docs/html/`, generated as part of the release build rather than hand-written.
- [ ] GitHub Releases from `v1.0.0` onward host a single `.zip` archive containing `cvxx.xll`, `cvxx.xlam`, `docs/html/`, `docs/examples/`, and a checksum file; no MSI/EXE installer or code-signed installer is required.
- [ ] README or docs explain how to unblock and trust unsigned Excel add-ins, and how to extract and use the release archive.
- [ ] No release depends on code signing or secret-holding infrastructure.

## Notes

- The XLAM will be created manually in Excel and committed as a binary asset; it is not rebuilt in CI.
- Target architecture: `x86_64-pc-windows-msvc` only.
- Pre-1.0 releases should use pre-release tags and be marked as pre-release on GitHub.
- The Markdown-to-HTML doc build tooling (e.g. `mdbook` or similar) is an implementation detail to be settled in the corresponding specification; it must not require network access at doc-view time and should stay within the Rust toolchain where practical.
