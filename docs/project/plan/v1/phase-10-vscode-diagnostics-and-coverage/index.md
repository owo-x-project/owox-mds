# Phase 10: VS Code Diagnostics And Coverage

## 目的

generated / embedded diagnostics の mirror と extension 側 coverage を固定し、fixture / cleanup phase の前提をそろえる。

## 前提条件

- active context と embedded provider bridge の主要機能がそろっている

## 完了条件

- generated / embedded diagnostics が Markdown 正本へ戻る
- extension の主要 bridge UX を継続確認できる coverage がある

## 検証方法

- extension compile、diagnostic mirror 確認、fixture を使う bridge scenario を行う

## task 一覧

- `task-001-remap-diagnostics-and-add-extension-coverage.md`: diagnostic mirror と extension coverage を固定する

## 依存関係

- `../phase-09-vscode-embedded-foundation/task-001-add-status-bar-and-active-context.md`
- `../phase-09-vscode-embedded-foundation/task-002-expand-embedded-provider-delegation.md`

## 参照

- `task-001-remap-diagnostics-and-add-extension-coverage.md`: diagnostics / coverage の固定