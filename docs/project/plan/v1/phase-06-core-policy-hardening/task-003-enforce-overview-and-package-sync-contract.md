# Task 003: Enforce Overview And Package Sync Contract

## 目的

`overview.md` managed region と `package sync --check` / write mode の契約を v1 spec どおり固定する。

## 前提条件

- output / special file / root module rule が config/schema 起点で決定できる

## 作業内容

- `overview.md` managed region の必須要件を実装と validation へ反映する
- `package sync --check` と write mode の差分を明確化する
- package summary と metadata snapshot の整合 contract をそろえる

## 完了条件

- `overview.md` 欠落や managed region 欠落が package error / sync error として観測できる
- `package sync` の check / write contract が説明できる

## 検証方法

- `package sync --check` と write mode の test を追加する
- representative fixture で `overview.md` と manifest の整合を確認する

## 依存関係

- `../phase-04-core-language-and-file-resolution/task-002-replace-output-special-file-and-root-module-rules.md`
- `../../../specs/mds-core/SPEC-core-overview-and-package-sync.md`

## 成果物

- `mds/core/src/package_sync.rs`
- `mds/core/src/markdown.rs`
- `mds/core/tests/`
- `docs/project/validation.md`