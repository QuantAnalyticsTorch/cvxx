# Skill: cvxx-documentation-and-notebooks

## Purpose

Standardize project documentation and interactive example notebooks.

## Guidelines

1. **Prose documentation**
   - Use Markdown for README, architecture notes, issues, and specs.
   - Keep one idea per paragraph.
   - Use code blocks with language tags.

2. **API documentation**
   - Use `rustdoc` comments (`///`) for public items.
   - Include examples in doc comments where practical.

3. **Example notebooks**
   - Use the XML-based notebook format required by this workspace.
   - Each notebook should have:
     - A markdown introduction cell.
     - A setup cell (imports / prerequisites).
     - One or more example cells.
     - A markdown summary cell.
   - Do not expose secrets or absolute paths in notebooks.

4. **User docs**
   - Place user guides in `docs/user/`.
   - Place developer guides in `docs/dev/`.
   - Keep the help index at `docs/html/index.html`.

5. **Maintenance**
   - Update docs when XLL function signatures change.
   - Verify that ribbon links still resolve.
