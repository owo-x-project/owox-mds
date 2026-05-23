---
name: mds
description: Generate code from markdown sources, validate document structure, and synchronize markdown sources with implementation
mds-managed: true
---

## What this skill does

Work with the mds (Markdown Source) system where Markdown is the single source of truth for code generation.

## When to use

- Adding or modifying features defined in markdown
- Code generation output needs updating
- Validating markdown-to-code synchronization

## Commands

```sh
mds new <name.lang.md> impl # Create implementation markdown from the current tableless template
mds new <name.lang.md> test # Create matching test markdown with Covers
mds new overview.md overview # Create hierarchy overview markdown
mds new index.ts.md root-module # Create language root module markdown with API prose
mds lint                    # Validate markdown structure
mds build --dry-run         # Preview generation
mds build                   # Generate code
mds lint --fix --check      # Fix formatting
mds test                    # Run generated tests
```

Always use `mds new` to create new files: `mds new greet.ts.md impl`, `mds new greet.ts.md test`, `mds new overview.md overview`

## mds Format Quick Reference

Source files: `.mds/source/name.{lang}.md` → generates `src/name.{lang}`
Test docs: `.mds/test/name.md` → generates language-specific test outputs and must declare `Covers`

- One file = one generated source file
- All code blocks are concatenated (separated by blank lines) to produce output
- Normal import/use/require statements belong in code blocks when the implementation needs dependencies
- Each code block must contain exactly one logical unit by default
- Doc comments and docstrings belong in surrounding markdown text, not inside code blocks
- `Purpose` documents every source md; `Contract` documents impl-state behavior
- `API` summarizes the public surface in prose
- Source md without `Source` code is spec state; adding generated code makes it impl state
- Test docs center on `Covers`, `Cases`, and `Test`

Rules: one md per feature, code fence language = file extension, top-level implementations split per fence by default
