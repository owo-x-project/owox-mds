---
applyTo: ".mds/source/**,.mds/test/**,**/*.md"
mds-managed: true
---

# mds Project Rules

Markdown is the source of truth. Generated code must not be edited directly.

## Workflow

- Use `mds new <name.lang.md> impl` to create new source markdown files in `.mds/source/` (never create them manually)
- Use `mds new <name.lang.md> test` to create matching test markdown in `.mds/test/` when behavior needs executable verification and add `Covers` for the source module id
- Use `mds new overview.md overview` for hierarchy overviews, and use language root module markdown such as `mds new lib.rs.md root-module`, `mds new sub/mod.rs.md root-module`, or `mds new index.ts.md root-module` for package or directory API docs
- Run `mds package sync` after package metadata changes
- Run `mds lint` before any generation-sensitive changes
- Run `mds build --dry-run` before writing generated outputs
- Run `mds lint --fix --check` to validate markdown quality

## mds Markdown Format

Implementation files: `.mds/source/name.{lang}.md` → generates `src/name.{lang}`

### Generation Rules

- One `.{lang}.md` file = one generated source file
- All code blocks are concatenated (separated by blank lines) to produce the output
- Normal import/use/require statements belong in code blocks when the implementation needs dependencies
- Each code block must contain exactly one logical unit by default
- Doc comments and docstrings belong in surrounding markdown text, not inside code blocks
- `Purpose` documents every source md; `Contract` documents impl-state behavior
- `API` summarizes the public surface in prose
- Source md without `Source` code is spec state; adding generated code makes it impl state
- New docs should not add Imports / Exports / Types tables

### Tableless Source Pattern

- Source docs use `Purpose`, `Contract`, `API`, `Source`, and `Cases`
- Test docs use `Purpose`, `Covers`, `Cases`, and `Test`

### Constraints

- One source md per feature
- Keep executable test intent in `.mds/test/` with `Covers`
- Generated output naming follows package output rules and the active language descriptor
- Code fence language must match file extension
- Default `mds lint` expects top-level implementations to be split per code fence; projects may relax selected checks in `[check]`
- Project-specific rules override mds rules when they conflict
