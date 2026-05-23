# Task 002: Move Doctor Policy To Config Runtime

## 目的

doctor の required / optional tool と version floor 判定を config/schema 起点へ移す。

## 前提条件

- language / file rule 解決が config/schema runtime へ移行している

## 作業内容

- doctor が参照する required / optional / version floor policy を config 起点へ寄せる
- package ごとの tool policy 差分を hardcode ではなく runtime 入力で返せるようにする
- CLI doctor 表示が新 policy と矛盾しないことを確認する

## 完了条件

- doctor policy が hardcode 依存ではなく package policy で説明できる
- required / optional / version floor の判定根拠が config/schema 由来である

## 検証方法

- `mds/core` の doctor 系 test を実行する
- representative package で doctor 出力を確認する

## 依存関係

- `../phase-04-core-language-and-file-resolution/task-001-replace-language-identity-and-doc-profile-resolution.md`
- `../phase-04-core-language-and-file-resolution/task-002-replace-output-special-file-and-root-module-rules.md`
- `../../../specs/mds-cli/SPEC-cli-doctor-and-update.md`

## 成果物

- `mds/core/src/doctor.rs`
- `mds/core/src/config.rs`
- `mds/core/tests/`