---
mds-managed: true
---

# mds Project Rules

Markdown is the source of truth. Generated code must not be edited directly.

## Dev Environment

```sh
mds new <name.lang.md> impl # Create implementation markdown from the current tableless template
mds new <name.lang.md> test # Create matching test markdown with Covers
mds new overview.md overview # Create hierarchy overview markdown
mds new index.ts.md root-module # Create language root module markdown with API prose
mds lint                    # Validate markdown structure
mds build --dry-run         # Preview generation output
mds build                   # Generate code from markdown
mds lint --fix --check      # Fix and validate formatting
mds test                    # Run tests on generated outputs
```

Always use `mds new <path> <kind>` to create new `.mds/source/` and `.mds/test/` files. Prefer `mds new <name.lang.md> impl`, `mds new <name.lang.md> test`, and `mds new overview.md overview`. Never create managed scaffolding manually.

## mds Markdown Format

Source files live in `.mds/source/` as `name.{lang}.md` (e.g., `helper.ts.md` → generates `src/helper.ts`). Test docs live in `.mds/test/` as Markdown files with `Covers` and `Test` sections.

### Generation Rules

- One `.{lang}.md` file = one generated source file
- All code blocks are concatenated (separated by blank lines) to produce the output
- Normal import/use/require statements belong in code blocks when the implementation needs dependencies
- Each code block must contain exactly one logical unit (type, function, class, impl, etc.) by default
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

## Testing

- Run `mds lint` to validate structure before committing
- Run `mds test` to run all generated tests
- Fix any errors before creating PRs
