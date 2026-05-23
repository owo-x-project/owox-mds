# mds — Markdown Source (VS Code Extension)

Language support for [mds](https://github.com/owo-x-project/owox-mds) implementation files (`.{lang}.md` — e.g. `.ts.md`, `.py.md`, `.go.md`).

## Features

- **Syntax highlighting** for mds Markdown files with embedded code blocks
- **Language Server** integration via `mds-lsp` for real-time diagnostics
- **Markdown preview** commands for mds implementation Markdown files
- **Snippets** for tableless authoring-v2 source/test documents and config files
- **Code block detection** with embedded language support through stable virtual documents
- **Generated-file and shadow-document bridge** for hover, definition, and diagnostics remapped back into Markdown when exact remap exists; remap failures stay hidden instead of guessed onto Markdown
- **Authoring assistance** for tableless source/test docs, H5 shared definitions, and quick fixes
- **Any language** — automatically detects `.{ext}.md` files under `.mds/source/` and `.mds/test/`

## Authoring Model

- Source docs live under `.mds/source/**/*.md` and typically use `Purpose`, `Contract`, `API`, `Source`, and `Cases`.
- Test docs live under `.mds/test/**/*.md` and typically use `Purpose`, `Covers`, `Cases`, and `Test`.
- Write imports and exports in prose plus normal code blocks. Legacy metadata tables remain readable, but snippets and default completions now prefer tableless authoring-v2.
- When an external language server answers on generated code, the extension uses the generated-file bridge to map results back to the Markdown source.

## Requirements

- Marketplace builds include the matching `mds-lsp` binary for the current platform
- Optional: install `mds-lsp` separately only when using a custom `mds.lsp.path` or local development build

## Configuration

| Setting | Default | Description |
| --- | --- | --- |
| `mds.lsp.enabled` | `true` | Enable/disable the mds language server |
| `mds.lsp.path` | `""` | Path to the mds-lsp binary. If empty, uses the bundled server first, then PATH |
| `mds.lsp.logLevel` | `"info"` | Log level for the language server |
| `mds.lsp.additionalLanguages` | `[]` | Additional language extensions (e.g., `.go.md`) |

## Regression Coverage

- `npm run compile` compiles the extension.
- `npm run test:diagnostics-regression` runs fixture-based no-harness Node regressions against the same stateful diagnostic mirror path used by the extension listener: diagnostics event snapshot -> cache update -> Markdown collection payload. It covers generated success, embedded merge success, remap-failure suppression, and stale shadow URI purge after block moves.
- Broader phase 10 bridge UX coverage lives in [regression/fixtures.md](./regression/fixtures.md) and [regression/bridge-ux-checklist.md](./regression/bridge-ux-checklist.md). Those assets document the canonical `examples/minimal-ts` workspace fixture and the manual scenarios for status bar, hover, definition, references, rename, formatting, edit-backed code actions, and diagnostic mirror behavior.

## License

MIT — see [LICENSE](../../LICENSE).
