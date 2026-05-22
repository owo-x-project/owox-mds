---
id: SPEC-model-package-layout
status: 承認済み
related:
  - ../../requirements/v1/REQ-product-markdown-source-of-truth.md
  - ../../requirements/v1/REQ-quality-safe-package-bounded-generation.md
  - ../../architecture.md
  - ../../validation.md
---

# Package Layout Model

## 概要

`mds` package の発見条件、authoring root、doc kind、special file を定義する共有仕様。

## 関連要求

- `REQ-product-markdown-source-of-truth`
- `REQ-quality-safe-package-bounded-generation`

## 入力

- package root directory
- `mds.config.toml`
- package manager metadata
- `.mds/source/**/*.md`
- `.mds/test/**/*.md`

## 出力

- package 判定結果
- source / test doc kind 判定結果
- special file 判定結果

## 挙動

- package は `mds.config.toml` と認識可能な package manager metadata を持つ単位として扱う。
- v1 の canonical authoring root は `.mds/source` と `.mds/test` とする。
- `.mds/source` 配下の `*.lang.md` は impl md、`.mds/test` 配下の `*.lang.md` は test md として扱う。
- `.mds/source/overview.md` は v1 の special file とし、package 概要と package metadata 要約の正本を置く。
- `.mds/source/overview.md` は package layout special file であり、descriptor `[[special_files]]` が定義する language-specific output special file とは別契約とする。
- `.mds/source/overview.md` は v1 package で必須とする。
- `overview.md` は一般 `mds file` の必須構造をそのまま要求しない。special file 契約として、top-level visible section `## Purpose` `## Architecture` `## Rules` を必須にする。
- source overview の package metadata managed section は `## Architecture` 配下に置く。
- `mds package sync` は `overview.md` 内の fixed visible heading managed section を package metadata から同期する。

## 状態遷移 / 不変条件

- package root 外の authoring doc は当該 package の入力として扱わない。
- 1 file は source または test のどちらか 1 つの doc kind だけを持つ。
- `overview.md` は source root 配下にのみ存在できる。
- package ごとに source overview は 1 つ存在する。
- v1 では authoring root 名は固定であり、layout の一貫性を優先する。

## エラー / 例外

- `mds.config.toml` がない場合、package として扱わない。
- package manager metadata を認識できない場合、enabled package として扱わない。
- `.mds/source/overview.md` がない場合、v1 package は不正とする。
- `overview.md` に必須 top-level section がない場合、v1 package は不正とする。
- source overview に必要な managed section がない場合、`package sync` は失敗する。

## 横断ルール

- この仕様は一般 `mds file` 構造仕様と分離する。
- v2 で project 全体資料へ拡張しても、code package の canonical root 契約は互換性を壊さない形で扱う。

## 検証観点

- package discovery が root と metadata で安定判定できる。
- source / test の doc kind を誤判定しない。
- `overview.md` special file と一般 `mds file` を混同しない。
- `package sync` が fixed visible heading managed section だけを更新する。

## 関連資料

- `../../requirements/v1/REQ-product-markdown-source-of-truth.md`
- `../../requirements/v1/REQ-quality-safe-package-bounded-generation.md`
- `../../architecture.md`
- `../../validation.md`
