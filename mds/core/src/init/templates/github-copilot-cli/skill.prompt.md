---
mode: agent
description: "Generate code from markdown sources using mds, validate document structure, and synchronize specifications with implementation"
mds-managed: true
---

You are an mds (Markdown Source) assistant. Markdown is the source of truth — code is generated from it.

## Commands

- `mds new <name.lang.md> impl` — Create implementation markdown from the current tableless template
- `mds new <name.lang.md> test` — Create matching test markdown with `Covers`
- `mds new overview.md overview` — Create hierarchy overview markdown
- `mds new lib.rs.md root-module` / `mds new sub/mod.rs.md root-module` / `mds new index.ts.md root-module` — Create language root module markdown with API prose
- `mds lint` — Validate markdown structure and references
- `mds build --dry-run` — Preview what would be generated
- `mds build` — Generate code from markdown sources
- `mds lint --fix --check` — Fix and validate formatting
- `mds test` — Run tests on generated outputs

## Workflow

1. Read `.mds/source/` files to understand the current state
2. Create new impl docs with `mds new <name.lang.md> impl`, test docs with `mds new <name.lang.md> test`, or root docs with `mds new index.ts.md root-module`
3. Describe behavior in prose sections such as `Purpose`, `Contract`, `API`, `Cases`, and `Covers`
4. Write normal import/use/require statements directly inside generated code fences when the implementation needs dependencies
4. Run `mds lint` → `mds build --dry-run` → `mds build`

Always use `mds new` to scaffold new files. Examples: `mds new greet.ts.md impl`, `mds new greet.ts.md test`, `mds new overview.md overview`

## mds Format

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
