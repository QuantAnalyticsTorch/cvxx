# Skill: cvxx-excel-ribbon-xlam

## Purpose

Define how to build and maintain the Excel macro-enabled add-in (`cvxx.xlam`) that supplies the ribbon UI.

## Guidelines

1. **XLAM structure**
   - Store ribbon XML in `customUI/customUI.xml`.
   - Store VBA callbacks in a dedicated module.
   - Keep the XLAM lightweight; heavy logic belongs in the Rust XLL.

2. **Ribbon design**
   - Provide a single top-level tab named `cvxx`.
   - Group controls logically: Solve, Examples, Help, About.
   - Use large buttons for primary actions and split buttons for example galleries.

3. **Documentation integration**
   - Link ribbon buttons to static HTML files in `docs/html/`.
   - Open HTML help in the user's default browser.
   - Maintain a help index page.

4. **Example notebooks**
   - Place interactive examples in `docs/examples/`.
   - Link each example from the ribbon.
   - Ensure examples use the current `CVX.*` function names.

5. **Distribution**
   - `cvxx.xlam` is created and maintained manually in Excel (ribbon XML injected via a tool such as the Custom UI Editor, VBA callbacks written directly in the VBE); it is never generated from source files by a build step.
   - After each manual edit, export the ribbon XML and every VBA module as plain text into `xlam/source/` (e.g. `xlam/source/customUI.xml`, `xlam/source/*.bas`) purely so changes are reviewable in diffs; these exports are not consumed by any build and must be re-imported manually if the XLAM is rebuilt from scratch.
   - Commit the binary `cvxx.xlam` itself under `assets/`.
   - Version the XLAM to match the Rust XLL.
