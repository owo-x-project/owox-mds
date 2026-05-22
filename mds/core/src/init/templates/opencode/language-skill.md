---
name: mds-{{LANG_ID}}
description: Author tableless mds {{LANG_ID}} markdown
mds-managed: true
---

# mds {{LANG_ID}} Authoring

Use this when editing `.mds/source/**/*.{{LANG_SUFFIXES}}.md` for descriptor `{{LANG_ID}}`.

Descriptor import style: `{{IMPORT_STYLE}}`

## Best Practices

- Write dependency imports and public entrypoints directly in `{{SOURCE}}` code fences using normal language syntax.
- Use prose sections such as `{{PURPOSE}}`, `{{CONTRACT}}`, `API`, and `{{CASES}}` to explain behavior instead of Imports / Exports / Types tables.
- Keep source md in spec state without generated code until implementation is ready; adding `{{SOURCE}}` code makes it impl state.
- Keep one feature per implementation md and one top-level logical unit per code fence.
- Keep doc comments and docstrings in Markdown prose outside generated code fences.

## Code Example

Write dependencies in the code fence itself when the implementation needs them.

```{{FENCE_LANG}}
{{GENERATED_IMPORT}}
```

Describe the public API in prose and keep the actual public definitions in the same code fence as the implementation.

Run `mds lint`, `mds build --dry-run`, `mds build`, and `mds test` after edits.
