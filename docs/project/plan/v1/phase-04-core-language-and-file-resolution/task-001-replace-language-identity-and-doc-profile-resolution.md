# Task 001: Replace Language Identity And Doc Profile Resolution

## 目的

file suffix、fence label、package config / schema から language identity と doc profile を決定する runtime へ移行する。

## 前提条件

- core の config/schema loader 境界が固定されている

## 作業内容

- language identity 解決を file suffix と fence label 起点へ寄せる
- prose-only を含む doc profile 判定を config/schema 起点へ寄せる
- CLI / LSP が再利用する language 判定 API を built-in 依存から切り離す

## 完了条件

- language identity と doc profile 判定が config/schema 由来で決定できる
- 将来の language 追加に built-in 追加が必須でない責務境界が明確である

## 検証方法

- `mds/core` の config / markdown / model 系 test を実行する
- representative fixture で parse と build が成立することを確認する

## 依存関係

- `index.md`
- `../phase-03-core-schema-loader-boundary/task-001-establish-core-schema-loader-boundary.md`

## 成果物

- `mds/core/src/model.rs`
- `mds/core/src/markdown.rs`
- `mds/core/src/descriptor.rs`
- `mds/core/tests/`