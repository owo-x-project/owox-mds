---
name: mds
description: Generate code from markdown sources, validate document structure, and synchronize markdown sources with implementation
mds-managed: true
---

## What this skill does

This skill helps you work with the mds (Markdown Source) system. mds treats Markdown files as the single source of truth and generates code from them.

## When to use

- When adding or modifying features defined in markdown
- When code generation output needs updating
- When validating that markdown sources and generated code are in sync

## Commands

```sh
mds new <name.lang.md> impl # Create implementation markdown from the current tableless template
mds new <name.lang.md> test # Create matching test markdown with Covers
mds new overview.md overview # Create hierarchy overview markdown
mds new index.ts.md root-module # Create language root module markdown with API prose
mds lint                    # Validate markdown structure and references
mds build --dry-run         # Preview what would be generated
mds build                   # Generate code from markdown sources
mds lint --fix --check      # Fix and validate formatting
mds test                    # Run tests on generated outputs
```

## Workflow

1. Read existing `.mds/source/` files to understand the current state
2. Create new impl docs with `mds new <name.lang.md> impl`, test docs with `mds new <name.lang.md> test`, or root docs with `mds new index.ts.md root-module`
3. Describe behavior in prose sections such as `Purpose`, `Contract`, `API`, `Cases`, and `Covers`
4. Write normal import/use/require statements directly inside generated code fences when the implementation needs dependencies
4. Run `mds lint` to validate structure
5. Run `mds build --dry-run` to preview generation
6. Run `mds build` to generate code
7. Run `mds test` to verify correctness

## Creating New Files

Always use `mds new` to create implementation markdown files:

```sh
mds new greet.ts.md impl                 # New TypeScript feature
mds new utils/helper.py.md impl          # New Python feature in subdirectory
mds new parser.rs.md impl                # New Rust feature
mds new greet.ts.md test                 # New test doc for a feature
mds new overview.md overview             # New source overview markdown
mds new sub/mod.rs.md root-module        # New Rust directory root module doc
mds new greet.ts.md impl --package ./my-pkg
```

## Format References

- See `.mds/reference/overview.md` for source overview format
- See `.mds/reference/root-module.md` for package or directory root API doc format
- See `.mds/reference/impl.md` for source implementation md format
- See `.mds/reference/test.md` for test md format

## mds Markdown Format

Implementation files: `.mds/source/name.{lang}.md` → generates `src/name.{lang}`

### Generation Rules

- One `.{lang}.md` file = one generated source file
- All code blocks in the file are concatenated (separated by blank lines) to produce the output
- Normal import/use/require statements belong in code blocks when the implementation needs dependencies
- Each code block must contain exactly one logical unit (type, function, class, impl, etc.) by default
- Doc comments and docstrings belong in surrounding markdown text, not inside code blocks

### Sections

Document source md as both specification and implementation source:

- `## {{PURPOSE}}` — Feature description
- `## {{CONTRACT}}` — Behavior guarantees
- `## API` — Public surface and module role in prose
- `## {{SOURCE}}` — Implementation code blocks with normal language imports/exports
- `## {{CASES}}` — Example behaviors
- Test docs center on `## Covers`, `## Cases`, and `## Test`
- Source md without generated `{{SOURCE}}` code is spec state; adding generated code makes it impl state

### Tableless Pattern

- Source docs usually use `Purpose`, `Contract`, `API`, `Source`, and `Cases`
- Test docs use `Purpose`, `Covers`, `Cases`, and `Test`

### Constraints

- One `.{lang}.md` per feature (one file = one generated source)
- Code fence language must match file extension
- Default `mds lint` expects top-level implementations to be split per code fence; projects may relax selected checks in `[check]`
- Multiple code blocks per section → concatenated with blank lines
