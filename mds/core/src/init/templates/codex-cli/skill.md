---
mds-managed: true
---

# mds Skill

## {{PURPOSE}}

Work with the mds (Markdown Source) system where Markdown is the single source of truth for code generation.

## Commands

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

## Workflow

1. Read existing `.mds/source/` files to understand the current state
2. Create new impl docs with `mds new <name.lang.md> impl`, test docs with `mds new <name.lang.md> test`, or root docs with `mds new index.ts.md root-module`
3. Describe behavior in prose sections such as `Purpose`, `Contract`, `API`, `Cases`, and `Covers`
4. Write normal import/use/require statements directly inside generated code fences when the implementation needs dependencies
4. Run `mds lint` → `mds build --dry-run` → `mds build`

Always use `mds new` to scaffold new files. Example: `mds new greet.ts.md impl`, `mds new greet.ts.md test`, `mds new overview.md overview`, `mds new index.ts.md root-module`

## Format References

- See `.mds/reference/overview.md` for source overview format
- See `.mds/reference/root-module.md` for package or directory root API doc format
- See `.mds/reference/impl.md` for source implementation md format
- See `.mds/reference/test.md` for test md format

## mds Markdown Format

Source files: `.mds/source/name.{lang}.md` → generates `src/name.{lang}`
Test docs: `.mds/test/name.md` → generates language-specific test outputs and must declare `Covers`

### Structure

- One `.{lang}.md` file = one generated source file
- All code blocks in the file are concatenated (separated by blank lines) to produce the output
- Normal import/use/require statements belong in code blocks when the implementation needs dependencies
- Each code block must contain exactly one logical unit (type, function, class, impl, etc.) by default
- Doc comments and docstrings belong in surrounding markdown text, not inside code blocks
- `Purpose` documents every source md; `Contract` documents impl-state behavior
- `API` summarizes the public surface in prose
- Source md without `Source` code is spec state; adding generated code makes it impl state
- Test docs center on `Covers`, `Cases`, and `Test`

### Rules

- One `.{lang}.md` per feature
- Code fence language must match file extension
- Default `mds lint` expects top-level implementations to be split per code fence; projects may relax selected checks in `[check]`
