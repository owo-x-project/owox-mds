# Phase 01: Freeze Capability Schema Surface

## 目的

capability schema が担う最小 surface を固定し、後続 phase が built-in 依存を増やさず進められる状態にする。

## 前提条件

- proposal と現行 spec の差分が把握されている

## 完了条件

- language identity、output rule、special file rule、quality slot、capture rule の最小 surface が正本へ反映されている
- 後続 phase が参照する schema 項目一覧がぶれない

## 検証方法

- proposal、shared spec、core spec の責務境界に矛盾がないことを確認する

## task 一覧

- `task-001-freeze-capability-schema-surface.md`: capability schema の最小項目を固定する

## 依存関係

- `../index.md`
- `../../../proposals/active/proposal-capability-schema-runtime.md`
- `../../../specs/shared/SPEC-language-extension-contract.md`
- `../../../specs/mds-core/SPEC-core-config-and-authoring-policy.md`

## 参照

- `task-001-freeze-capability-schema-surface.md`: schema surface の固定