---
id: SPEC-core-overview-and-package-sync
status: 承認済み
related:
  - ../shared/SPEC-model-package-layout.md
  - ../shared/SPEC-authoring-markdown-format.md
  - ../../requirements/v1/REQ-quality-safe-package-bounded-generation.md
  - ../../requirements/v1/REQ-product-markdown-source-of-truth.md
subproject: mds-core
---

# mds-core Overview And Package Sync

## 概要

`mds-core` における `.mds/source/overview.md` special file と `mds package sync` の契約を定義する。

## 関連要求

- `REQ-quality-safe-package-bounded-generation`
- `REQ-product-markdown-source-of-truth`

## 入力

- `.mds/source/overview.md`
- package manager metadata
- package sync config
- `mds package sync` command mode

## 出力

- synchronized source overview
- package metadata diff
- package sync diagnostics

## 挙動

- `mds-core` は `.mds/source/overview.md` を package 単位の必須 special file として扱う。
- source overview は top-level visible section `## Purpose` `## Architecture` `## Rules` を必須で持つ。
- source overview は `## Architecture` 以降、`## Rules` 直前までの package overview 領域に `### Package Summary` `### Dependencies` `### Dev Dependencies` の fixed visible heading managed section を必須で持つ。この領域には `## Exposes` などの追加 top-level section を含めてもよいが、`## Rules` 以降の narrative section は managed section 探索対象外とする。
- top-level narrative section は authoring label preset / label override の対象にできる。
- managed section heading は overview special file 契約の fixed anchor とし、label preset / label override / section title independence の対象外とする。
- `mds package sync` は package metadata を読み、heading managed section だけを同期する。
- `--check` では heading managed section 差分を報告し、non-zero result を返す。通常実行では同じ section だけを更新する。
- manual prose と heading managed section 外の記述は保持する。
- package sync hook を使う場合、package sync 後に hook command を案内または実行対象として扱える。

## 状態遷移 / 不変条件

- source overview は package ごとに 1 つ存在する。
- source overview の top-level narrative section は `## Purpose` `## Architecture` `## Rules` である。
- heading managed section 更新は package metadata に対して決定的である。
- heading managed section 更新は `Architecture` / `Rules` semantic section の label override を尊重する。
- manual prose は package sync で壊さない。
- test overview は任意の補助文書であり、package sync の必須対象ではない。source/test overview path は package layout special file として扱い、descriptor `[[special_files]]` とは分離する。

## エラー / 例外

- source overview 欠落は package error とする。
- 必須 top-level section 欠落は package error とする。
- 必須 heading managed section 欠落は package sync error とする。
- metadata 読み取り不能時は package sync を失敗させる。
- `--check` で差分がある場合は non-zero result を返せる。

## 横断ルール

- special file 契約は一般 `mds file` の section 規則とは分離する。ただし overview 自身は専用契約として `Purpose` / `Architecture` / `Rules` の narrative section を持つ。
- overview managed section の fixed visible heading は package sync 契約の一部として validation される。
- overview は package metadata と authoring docs を結ぶ trace point として扱う。

## 検証観点

- source overview 欠落を検出できる。
- source overview の必須 top-level section 欠落を検出できる。
- heading managed section のみが更新される。
- `--check` と write mode の差分が説明可能である。
- test overview が package sync に必須でないことが保たれる。

## 関連資料

- `../shared/SPEC-model-package-layout.md`
- `../shared/SPEC-authoring-markdown-format.md`
- `../../requirements/v1/REQ-quality-safe-package-bounded-generation.md`
- `../../requirements/v1/REQ-product-markdown-source-of-truth.md`
