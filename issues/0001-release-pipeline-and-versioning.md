---
id: ISSUE-0001
title: Release pipeline and versioning for cvxx binaries
priority: must
status: draft
created: 2026-09-29
---

## Background

`cvxx` is an Excel add-in composed of a Rust XLL (`cvxx.xll`) and an Excel macro-enabled add-in (`cvxx.xlam`). Once the project reaches version `1.0.0`, the binaries must be published on GitHub so users can download and install them. Before that, the project needs a clear versioning strategy and a lightweight release process that matches the team's constraints.

## Goal

Establish a versioning and release pipeline that:

1. Keeps the XLL and XLAM versions in lockstep.
2. Produces reproducible local builds of the XLL on 64-bit Windows.
3. Publishes GitHub Releases with downloadable `cvxx.xll` and `cvxx.xlam` artifacts from version `1.0.0` onward.
4. Documents the install steps for unsigned add-ins.

## Acceptance Criteria

- [ ] A single source of truth for the project version exists and is documented.
- [ ] The Rust XLL builds locally for 64-bit Excel with a documented one-line command.
- [ ] A GitHub Actions workflow validates every PR and tag (`cargo test`, `cargo clippy`, `cargo fmt`, and tag/version alignment).
- [ ] GitHub Releases from `v1.0.0` onward host `cvxx.xll`, `cvxx.xlam`, and a checksum file.
- [ ] README or docs explain how to unblock and trust unsigned Excel add-ins.
- [ ] No release depends on code signing or secret-holding infrastructure.

## Notes

- The XLAM will be created manually in Excel and committed as a binary asset; it is not rebuilt in CI.
- Target architecture: `x86_64-pc-windows-msvc` only.
- Pre-1.0 releases should use pre-release tags and be marked as pre-release on GitHub.
