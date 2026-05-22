# Task 001: Establish Core Schema Loader Boundary

## 目的

`mds-core` が capability schema を読み、CLI / LSP / VS Code が再利用できる runtime 入力を返す loader 境界を固定する。

## 前提条件

- schema surface と migration compatibility が固定されている

## 作業内容

- config loader と schema loader の責務境界を実装へ反映する
- core が後続 task で再利用する language / policy input を返せる API をそろえる
- built-in descriptor 依存の直結箇所を後続 task で置換しやすい形へ寄せる

## 完了条件

- config/schema loader の責務境界が code と spec で説明できる
- language/file rule 置換 task が同じ loader を前提に進められる

## 検証方法

- `mds/core` の config / package / descriptor 系 test を確認する
- CLI / LSP が再利用する core API の入力が hardcode 前提でないことを確認する

## 依存関係

- `../phase-02-freeze-migration-compatibility/task-001-define-migration-compatibility-rules.md`
- `../../../specs/mds-core/SPEC-core-config-and-authoring-policy.md`

## 成果物

- `mds/core/src/config.rs`
- `mds/core/src/model.rs`
- `mds/core/src/descriptor.rs`
- `mds/core/tests/`