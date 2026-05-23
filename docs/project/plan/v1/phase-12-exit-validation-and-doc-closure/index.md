# Phase 12: Exit Validation And Doc Closure

## 目的

v1 完了判定に必要な横断 validation と docs 参照整合を閉じる。

## 前提条件

- fixture、runtime cleanup、extension coverage がそろっている

## 完了条件

- v1 完了に必要な代表確認が一通り実施される
- plan / validation / examples / descriptor authoring docs / 関連 spec の参照が現実装と一致する

## 検証方法

- Rust workspace warning-free build/test、VS Code compile / diagnostics regression、package cwd での examples command、diagnostic remap、docs 参照整合を確認する

## task 一覧

- `task-001-run-v1-exit-validation-and-close-docs.md`: 最終 validation と docs closure を行う
- `task-002-align-repo-docs-for-descriptor-authoring.md`: descriptor authoring / pack / source の repo docs を整備する

## 依存関係

- `../phase-11-fixtures-and-runtime-cleanup/task-001-align-minimal-ts-fixture.md`
- `../phase-11-fixtures-and-runtime-cleanup/task-002-add-broken-remap-fixture-and-live-regressions.md`
- `../phase-11-fixtures-and-runtime-cleanup/task-003-remove-obsolete-builtins-and-align-examples.md`
- `../phase-07-cli-and-lsp-adoption-foundation/task-005-add-descriptor-authoring-cli.md`
- `../../../specs/shared/SPEC-descriptor-pack-resolution.md`
- `../../../specs/mds-cli/SPEC-cli-descriptor-authoring-workflows.md`
- `../../../validation.md`

## 参照

- `task-001-run-v1-exit-validation-and-close-docs.md`: 最終 validation と docs closure
- `task-002-align-repo-docs-for-descriptor-authoring.md`: descriptor authoring docs 整備
