# Phase 06: Core Policy Hardening

## 目的

diagnostic remap、link policy、overview/package sync 契約を固め、CLI / LSP / examples が同じ core policy を共有できる状態にする。

## 前提条件

- quality / doctor runtime が config/schema 起点へ移行している

## 完了条件

- remap、link policy、overview/package sync 契約が v1 spec どおり観測できる
- 後続の CLI / LSP / fixture task が同じ package policy を再利用できる

## 検証方法

- core test と representative fixture で remap / lint fix / package sync を確認する

## task 一覧

- `task-001-preserve-diagnostic-remap-with-source-map.md`: source map 前提の remap を維持する
- `task-002-enforce-link-policy-normalization.md`: link policy 3 mode と lint fix 正規化を固める
- `task-003-enforce-overview-and-package-sync-contract.md`: `overview.md` と package sync 契約を固める

## 依存関係

- `../phase-05-core-quality-and-doctor-runtime/task-001-replace-quality-slot-and-capture-resolution.md`
- `../phase-05-core-quality-and-doctor-runtime/task-002-move-doctor-policy-to-config-runtime.md`
- `../../../specs/shared/SPEC-authoring-markdown-format.md`
- `../../../specs/mds-core/SPEC-core-overview-and-package-sync.md`

## 参照

- `task-001-preserve-diagnostic-remap-with-source-map.md`: remap 契約の維持
- `task-002-enforce-link-policy-normalization.md`: link policy 正規化
- `task-003-enforce-overview-and-package-sync-contract.md`: overview / package sync 契約