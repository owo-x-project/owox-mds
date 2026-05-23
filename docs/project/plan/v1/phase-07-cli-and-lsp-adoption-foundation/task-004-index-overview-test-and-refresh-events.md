# Task 004: Index Overview Test And Refresh Events

## 目的

`mds-lsp` が impl md だけでなく test md と `overview.md` も含めて workspace を index し、通常の保存・変更で stale にならない状態にする。

## 前提条件

- core が source/test/overview 契約を正しく返せる

## 作業内容

- workspace index に test doc、`overview.md`、generated/source map 対応を含める
- 保存、config 変更、watched file change で必要な再 index を行う
- `overview.md` special file の扱いを diagnostics / symbol / navigation 側と整合させる

## 完了条件

- impl/test/overview を含む index が生成される
- 編集 / 保存 / file 追加削除で index が古いまま残らない

## 検証方法

- workspace index と reindex event の test を追加する
- representative package で save / file add / file remove 相当の更新を確認する

## 依存関係

- `index.md`
- `../phase-06-core-policy-hardening/task-003-enforce-overview-and-package-sync-contract.md`

## 成果物

- `mds/lsp/src/server.rs`
- `mds/lsp/src/state.rs`
- `mds/lsp/tests/`