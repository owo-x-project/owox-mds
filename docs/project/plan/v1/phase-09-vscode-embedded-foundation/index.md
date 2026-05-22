# Phase 09: VS Code Embedded Foundation

## 目的

VS Code extension の active context 表示と embedded provider bridge を並行で進め、diagnostic mirror phase の前提をそろえる。

## 前提条件

- LSP の language discovery、structured navigation、bridge command surface が利用できる

## 完了条件

- status bar で active language / doc kind を安定表示できる
- provider がある言語で references / rename / code action / formatting を再利用できる

## 検証方法

- extension compile と bridge scenario 確認を行う

## task 一覧

- `task-001-add-status-bar-and-active-context.md`: active context 表示を追加する
- `task-002-expand-embedded-provider-delegation.md`: embedded provider bridge を拡張する

## 依存関係

- `../phase-08-cli-coverage-and-lsp-advanced-surface/task-002-prioritize-structured-navigation-and-references.md`
- `../phase-08-cli-coverage-and-lsp-advanced-surface/task-003-expand-authoring-and-bridge-command-surface.md`
- `../../../specs/vscode-extension/SPEC-vscode-embedded-editor-experience.md`

## 参照

- `task-001-add-status-bar-and-active-context.md`: active context UX の追加
- `task-002-expand-embedded-provider-delegation.md`: provider bridge の拡張