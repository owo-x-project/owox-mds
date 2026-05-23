# Phase 03: Core Schema Loader Boundary

## 目的

`mds-core` が config/schema runtime と descriptor source / pack を読み込む loader 境界を固定し、後続の rule 置換 task を並行着手できる状態にする。

## 前提条件

- schema surface と migration compatibility が固定されている

## 完了条件

- core が capability schema と descriptor source / pack を読む入口と責務境界を説明できる
- language/file rule 置換 task が同じ loader 前提で並行着手できる

## 検証方法

- core config / package 系 test と API 参照面を確認する

## task 一覧

- `task-001-establish-core-schema-loader-boundary.md`: core の config/schema loader 境界を固定する
- `task-002-add-descriptor-source-and-pack-resolution.md`: descriptor source / pack resolution 境界を追加する

## 依存関係

- `../phase-02-freeze-migration-compatibility/task-001-define-migration-compatibility-rules.md`
- `../../../specs/shared/SPEC-descriptor-pack-resolution.md`
- `../../../specs/mds-core/SPEC-core-config-and-authoring-policy.md`

## 参照

- `task-001-establish-core-schema-loader-boundary.md`: loader 境界の固定
- `task-002-add-descriptor-source-and-pack-resolution.md`: descriptor source / pack resolution の追加
