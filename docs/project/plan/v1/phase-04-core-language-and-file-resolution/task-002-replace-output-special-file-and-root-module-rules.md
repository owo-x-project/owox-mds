# Task 002: Replace Output Special File And Root Module Rules

## 目的

output rule、special file rule、root module rule を config/schema runtime から解決する形へ移行する。

## 前提条件

- core の config/schema loader 境界が固定されている

## 作業内容

- output rule を config/schema から読む経路へ置き換える
- `overview.md` を含む special file rule を built-in 前提から外す
- root module rule を doc profile と整合する形へ寄せる

## 完了条件

- output / special file / root module rule が config/schema 由来で決定できる
- package sync、generation、LSP index が同じ file rule を再利用できる

## 検証方法

- `mds/core` の generation / package / markdown 系 test を実行する
- representative fixture で source / test / build / package sync を確認する

## 依存関係

- `index.md`
- `../phase-03-core-schema-loader-boundary/task-001-establish-core-schema-loader-boundary.md`

## 成果物

- `mds/core/src/generation.rs`
- `mds/core/src/markdown.rs`
- `mds/core/src/package_sync.rs`
- `mds/core/tests/`