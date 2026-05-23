---
id: ADR-001-remove-built-in-descriptor-corpus
status: 採用
related:
  - ../../proposals/active/proposal-capability-schema-runtime.md
  - ../../plan/v1/index.md
  - ../../plan/v1/phase-02-freeze-migration-compatibility/task-001-define-migration-compatibility-rules.md
  - ../../plan/v1/phase-11-fixtures-and-runtime-cleanup/task-003-remove-obsolete-builtins-and-align-examples.md
subproject: mds-core
---

# built-in descriptor corpus removal

## 背景

v1 では config/schema runtime への移行により、runtime の正本を package-local capability schema、package manager manifest、package config、package metadata に限定する必要がある。historical built-in descriptor corpus はこの runtime model と競合し、fixtures や tests が obsolete corpus を正本として参照すると v1 の検証条件を曖昧にする。

## 判断

`mds-core` から runtime built-in descriptor corpus / registry を削除する。v1 の公開 runtime contract は package-local capability schema、package manager manifest、package config、package metadata、明示された descriptor source / lock に限定し、core resolver は built-in descriptor data を fallback として使わない。

`mds init` の v1 clean bootstrap だけは例外として、npm / TypeScript の最小 descriptor seed を package-local `.mds/descriptors/**` に書き出してよい。この seed は init 導線の authoring 初期値であり、resolver runtime の built-in corpus ではない。代表実行経路は書き出された package-local descriptor を読むことで成立しなければならない。

## 代替案

- built-in corpus を残し、必要な言語と tool を都度追加する。
- built-in corpus を fallback として残し、package-local schema 不在時だけ使う。
- package-local `.mds/descriptors/...` と historical built-in descriptor corpus を同一概念として継続する。

## 結果

- phase 02 で停止条件を固定し、phase 11 で obsolete runtime built-in descriptor corpus を削除済み。
- `mds-core` の loader、quality、LSP は built-in corpus fallback を前提にしない。
- `mds init` が保持する seed descriptor は package-local descriptor を生成するためだけに使い、init 後の resolver / LSP / VS Code は package-local または descriptor source 由来の registry を正とする。
- 今後の文書では `descriptor` と書く場合、package-local capability schema か historical built-in corpus かを明示する。

## 関連資料

- `../../plan/v1/index.md`
- `../../plan/v1/phase-02-freeze-migration-compatibility/task-001-define-migration-compatibility-rules.md`
- `../../plan/v1/phase-11-fixtures-and-runtime-cleanup/task-003-remove-obsolete-builtins-and-align-examples.md`
- `../../proposals/active/proposal-capability-schema-runtime.md`
