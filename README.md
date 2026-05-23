# owox-mds

`owox-mds` は Markdown を実装正本として扱う `mds` の monorepo。

## 含むもの

- `mds/core`: 解析、生成、quality、descriptor を担う core library
- `mds/cli`: `mds` CLI
- `mds/lsp`: `mds-lsp` language server
- `editors/vscode`: VS Code extension
- `examples`: TypeScript / Python / Rust の最小サンプル

## descriptor 方針

v1 の `mds` 本体は language / tool / package-manager 固有 descriptor content を所有しない。package-local descriptor、workspace shared descriptor、lock 済み descriptor source から registry を組み立てる。

descriptor の作成、検証、override、source pack 化は `docs/project/specs/shared/GUIDE-descriptor-authoring.md` を入口にする。代表 command は `mds init descriptor` と `mds descriptor check/explain/schema/sources`。

## project docs

- project 正本: `docs/project/`
- 最初に読む資料: `docs/project/index.md`
- descriptor authoring guide: `docs/project/specs/shared/GUIDE-descriptor-authoring.md`
- AI 用 project 定義: `.agents/project.md`
