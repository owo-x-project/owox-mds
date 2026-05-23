# Phase 08: CLI Coverage And LSP Advanced Surface

## 目的

CLI contract の回帰固定と LSP の advanced navigation / bridge surface 拡張を並行で進め、VS Code phase の前提をそろえる。

## 前提条件

- CLI init/new と LSP language discovery / workspace index が新 runtime と整合している

## 完了条件

- CLI の主要 command contract が test で固定される
- LSP の structured navigation と bridge command surface が VS Code へ渡せる水準に達する

## 検証方法

- CLI integration test、LSP navigation / authoring / command surface test を確認する

## task 一覧

- `task-001-add-cli-integration-coverage.md`: CLI 主要 contract を test で固定する
- `task-002-prioritize-structured-navigation-and-references.md`: structured-first navigation を固める
- `task-003-expand-authoring-and-bridge-command-surface.md`: authoring / bridge command surface を拡張する

## 依存関係

- `../phase-07-cli-and-lsp-adoption-foundation/task-001-align-init-wizard-screen-flow.md`
- `../phase-07-cli-and-lsp-adoption-foundation/task-002-align-new-command-and-templates.md`
- `../phase-07-cli-and-lsp-adoption-foundation/task-003-adopt-lsp-language-discovery.md`
- `../phase-07-cli-and-lsp-adoption-foundation/task-004-index-overview-test-and-refresh-events.md`

## 参照

- `task-001-add-cli-integration-coverage.md`: CLI coverage の固定
- `task-002-prioritize-structured-navigation-and-references.md`: structured navigation の固定
- `task-003-expand-authoring-and-bridge-command-surface.md`: bridge surface の拡張