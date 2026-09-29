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
   - Build the XLAM from source files in `xlam/`.
   - Version the XLAM to match the Rust XLL.
