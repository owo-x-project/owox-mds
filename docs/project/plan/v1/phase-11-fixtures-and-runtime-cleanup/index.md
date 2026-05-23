# Phase 11: Fixtures And Runtime Cleanup

## 目的

success-path fixture、broken/remap fixture、obsolete built-in descriptor corpus cleanup を並行で進め、最終 validation の確認資産をそろえる。

## 前提条件

- core、CLI、LSP、VS Code の主要 v1 差分が解消されている

## 完了条件

- `minimal-ts` が success-path fixture として安定する
- broken/remap fixture が live regression 資産として使える
- built-in descriptor / tool / package manager registry が runtime 参照から完全削除され、obsolete corpus directory と docs の v1 必須経路説明が残らず、examples の整合が新 runtime 前提へ移っている

## 検証方法

- `examples/` と regression assets を使う representative command を確認する

## task 一覧

- `task-001-align-minimal-ts-fixture.md`: success-path fixture を完成させる
- `task-002-add-broken-remap-fixture-and-live-regressions.md`: broken/remap fixture と live regression を追加する
- `task-003-remove-obsolete-builtins-and-align-examples.md`: obsolete built-in descriptor corpus の完全削除と examples 整合を進める

## 依存関係

- `../phase-10-vscode-diagnostics-and-coverage/task-001-remap-diagnostics-and-add-extension-coverage.md`
- `../../../specs/examples/SPEC-examples-minimal-ts-fixture.md`
- `../../../specs/examples/SPEC-examples-v1-regression-fixtures.md`

## 参照

- `task-001-align-minimal-ts-fixture.md`: success-path fixture の完成
- `task-002-add-broken-remap-fixture-and-live-regressions.md`: broken/remap fixture の追加
- `task-003-remove-obsolete-builtins-and-align-examples.md`: obsolete built-in descriptor corpus cleanup と examples 整合
