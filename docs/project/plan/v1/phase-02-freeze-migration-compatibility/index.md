# Phase 02: Freeze Migration Compatibility

## 目的

旧 built-in registry から config/schema runtime へ移る間の fallback と削除順を固定し、後続実装の戻り先を明確にする。

## 前提条件

- capability schema の最小 surface が固定されている

## 完了条件

- fallback 方針と削除順序が plan / proposal で説明できる
- fallback 適用条件、禁止事項、停止条件、削除 phase が phase 02 docs から辿れる
- 後続 phase が互換期間の前提を再定義しなくてよい

## 固定ルール要約

- fallback は config/schema runtime だけでは既存 v1 flow を維持できない間だけ許可する
- legacy fallback を理由に新要件、新 built-in、runtime precedence 変更を持ち込まない
- capability schema と package config で固定した public contract、および対象 package 自身の metadata 読み取りだけで representative validation が通った対象から fallback を止める
- package manager registry fallback の runtime input owner は phase 03 `task-001-establish-core-schema-loader-boundary.md` と `task-002-add-descriptor-source-and-pack-resolution.md` で固定し、quality slot / capture の precedence / fallback / order は phase 05 で正式化し、built-in descriptor corpus / tool registry / package manager registry の完全削除と examples 整合は phase 11 で行う

## 検証方法

- proposal と plan が同じ移行段階を示していることを確認する

## task 一覧

- `task-001-define-migration-compatibility-rules.md`: 移行互換ルールを固定する

## 依存関係

- `../phase-01-freeze-capability-schema-surface/task-001-freeze-capability-schema-surface.md`
- `../../../proposals/active/proposal-capability-schema-runtime.md`

## 参照

- `task-001-define-migration-compatibility-rules.md`: fallback と削除順の固定
- `../phase-03-core-schema-loader-boundary/task-001-establish-core-schema-loader-boundary.md`: package manager metadata reader / schema loader 境界の固定
- `../phase-05-core-quality-and-doctor-runtime/index.md`: runtime precedence / order の正式化
- `../phase-11-fixtures-and-runtime-cleanup/task-003-remove-obsolete-builtins-and-align-examples.md`: obsolete built-in と package manager registry fallback cleanup の実施
