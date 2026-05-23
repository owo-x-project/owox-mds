# Platform Team Guide

## 役割

`mds` 本体と editor tooling を継続保守する。

## 担当範囲

- `mds/core`
- `mds/cli`
- `mds/lsp`
- `editors/vscode`
- `examples`
- `docs/project`

## 固有ルール

- 仕様変更・追加は必ず Spec を更新・作成してから実装を行う
- 仕様変更時は examples も更新し、使用感をレビューする

## 固有知識

- source map 整合は LSP diagnostics と generated file remap の前提
- CLI、LSP、VS Code extension の UX は `mds-core` の出力規約に揃える

## 参照

- `../architecture.md`
- `../validation.md`
- `../tech-stack.md`
