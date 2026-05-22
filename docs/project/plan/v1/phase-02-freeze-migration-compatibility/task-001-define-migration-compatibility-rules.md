# Task 001: Define Migration Compatibility Rules

## 目的

旧 built-in registry から config/schema runtime へ移る移行期間の fallback 挙動と削除順序を固定する。

## 前提条件

- capability schema の最小 surface が固定されている

## 作業内容

- 互換期間の fallback 挙動を定義する
- built-in descriptor / tool / package manager registry の完全削除段階と停止条件を整理する
- proposal と plan に同じ移行前提を反映する

## 固定する migration compatibility rules

### fallback の適用条件

- legacy fallback は、config/schema runtime だけでは既存 v1 flow を維持できない間に限り許可する
- 対象は既に同梱済みの built-in descriptor、tool profile、package manager registry の読み取り互換だけとする。capability schema と package config で固定した public contract、および対象 package 自身の metadata 読み取りで同等の runtime data がそろった package / fixture では fallback を前提にしない
- phase 02 は compatibility window だけを固定する。quality slot / capture の precedence / fallback / order は phase 05 で正式化する
- package manager registry fallback の runtime input owner は phase 03 `task-001-establish-core-schema-loader-boundary.md` と `task-002-add-descriptor-source-and-pack-resolution.md` とし、package manager metadata reader / schema loader / descriptor source 境界で後続実装を受ける

### 禁止事項

- legacy fallback を理由に public schema/config surface を増やさない
- 新言語、新ツール、新 package manager、新 diagnostic capture rule、新 quality slot semantic を built-in 追加で受けない
- examples、fixtures、docs、後続 task で旧 built-in registry を新しい必須経路として説明しない

### 停止条件と削除順序

- 対象 package / fixture の representative flow が capability schema と package config で固定した contract、および対象 package 自身の metadata 読み取りだけで説明でき、representative validation が legacy built-in descriptor / tool profile / package manager registry 前提なしで通った時点で、その対象の fallback を停止する
- package manager metadata reader / schema loader / descriptor source の責務境界は phase 03 `task-001-establish-core-schema-loader-boundary.md` と `task-002-add-descriptor-source-and-pack-resolution.md` で固定する。phase 02 の停止条件はその境界決定待ちで変えない
- fallback 停止後、旧 built-in 依存の再導入を禁止する
- built-in descriptor corpus / tool registry / package manager registry の実削除、および examples 整合は phase 11 `task-003-remove-obsolete-builtins-and-align-examples.md` で行う。phase 02-10 では削除ではなく縮退と停止条件確認を進める

## 完了条件

- fallback と削除順序が明文化されている
- built-in descriptor corpus の完全削除判断と停止条件が明文化されている
- fallback 適用条件、禁止事項、停止条件、削除 phase が proposal と一致する
- 後続 task が互換期間の有無を都度判断しなくてよい

## 検証方法

- proposal と plan を読み、移行段階と fallback の記述が矛盾しないことを確認する

## 依存関係

- `../phase-01-freeze-capability-schema-surface/task-001-freeze-capability-schema-surface.md`
- `../../../proposals/active/proposal-capability-schema-runtime.md`

## 成果物

- `docs/project/proposals/active/proposal-capability-schema-runtime.md`
- `docs/project/plan/v1/`

## 参照

- `../phase-03-core-schema-loader-boundary/task-001-establish-core-schema-loader-boundary.md`: package manager metadata reader / schema loader 境界の固定
- `../phase-03-core-schema-loader-boundary/task-002-add-descriptor-source-and-pack-resolution.md`: descriptor source / pack resolution 境界の固定
- `../phase-05-core-quality-and-doctor-runtime/index.md`: runtime precedence / order の正式化先
- `../phase-11-fixtures-and-runtime-cleanup/task-003-remove-obsolete-builtins-and-align-examples.md`: obsolete built-in と package manager registry fallback cleanup の実施先
