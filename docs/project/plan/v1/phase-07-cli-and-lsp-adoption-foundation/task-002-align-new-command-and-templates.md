# Task 002: Align New Command And Templates

## 目的

`mds new` を `mds new <path> <kind> [options]` 契約へそろえ、impl/test/overview/root module template を v1 authoring policy に従って起票できる状態にする。

## 前提条件

- core が doc kind、doc profile、link policy を解釈できる

## 作業内容

- `new` の引数構造を `path` と `kind` 中心へ見直す
- kind 未指定時の対話 fallback と usage / selection 動作を整える
- impl/test/overview/root module template を v1 policy に合わせて更新する

## 完了条件

- `new` が kind ごとに適切な template を canonical root に起票できる
- unmanaged file 上書きが `--force` なしで発生しない

## 検証方法

- args / new 系 test を追加する
- temp package で impl / test / overview / root module 起票を確認する

## 依存関係

- `index.md`
- `../phase-06-core-policy-hardening/task-002-enforce-link-policy-normalization.md`
- `../phase-06-core-policy-hardening/task-003-enforce-overview-and-package-sync-contract.md`

## 成果物

- `mds/cli/src/args.rs`
- `mds/core/src/new.rs`
- `mds/core/src/init/mod.rs`
- `mds/cli/tests/args_test.rs`
- `mds/core/tests/`