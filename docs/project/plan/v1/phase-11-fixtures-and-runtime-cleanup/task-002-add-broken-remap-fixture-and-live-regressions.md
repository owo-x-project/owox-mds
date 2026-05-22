# Task 002: Add Broken Remap Fixture And Live Regressions

## 目的

success-path fixture と分離した broken/remap TypeScript fixture を追加し、diagnostic remap と CLI/LSP/VS Code の live regression を確認できる状態にする。

## 前提条件

- remap 契約が core / LSP / VS Code でそろっている

## 作業内容

- broken/remap 専用 TypeScript fixture package を追加する
- remap 成功 / 失敗 / no-op を見分ける regression case を追加する
- Rust test、CLI 実行、VS Code diagnostic mirror 確認へ fixture を接続する

## 完了条件

- success-path fixture と broken/remap fixture の責務が分離される
- diagnostic remap 変更時の代表 regression が再実行できる

## 検証方法

- core / LSP test に broken/remap fixture を追加する
- diagnostic mirror と generated remap の手動確認手順を整える

## 依存関係

- `../phase-06-core-policy-hardening/task-001-preserve-diagnostic-remap-with-source-map.md`
- `../phase-08-cli-coverage-and-lsp-advanced-surface/task-002-prioritize-structured-navigation-and-references.md`
- `../phase-10-vscode-diagnostics-and-coverage/task-001-remap-diagnostics-and-add-extension-coverage.md`

## 成果物

- `examples/`
- `mds/core/tests/`
- `mds/lsp/tests/`
- extension regression assets