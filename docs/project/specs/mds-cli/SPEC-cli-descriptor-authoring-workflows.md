---
id: SPEC-cli-descriptor-authoring-workflows
status: 承認済み
related:
  - ../shared/SPEC-descriptor-pack-resolution.md
  - ../shared/SPEC-language-extension-contract.md
  - ./SPEC-cli-command-surface-and-execution.md
  - ./SPEC-cli-init-and-new-workflows.md
  - ../../requirements/v1/REQ-quality-language-and-toolchain-independence.md
  - ../../requirements/v1/REQ-ux-human-ai-authoring-experience.md
subproject: mds-cli
---

# CLI Descriptor Authoring Workflows

## 概要

`mds init` と `mds descriptor` による descriptor 作成、検証、説明、schema 出力の UX 契約を定義する。

## 関連要求

- `REQ-quality-language-and-toolchain-independence`
- `REQ-ux-human-ai-authoring-experience`

## 入力

- interactive wizard input
- descriptor kind: `language` / `tool` / `package-manager`
- existing package files
- existing descriptor files
- descriptor source / pack config
- target path / package root

## 出力

- generated descriptor TOML
- descriptor diagnostics
- descriptor resolution explanation
- descriptor schema
- optional package-local override / eject file

## 挙動

- `mds init descriptor <kind>` は descriptor authoring wizard として扱う。
- `mds init descriptor language` は mds が言語名を知っていることを前提にせず、質問への回答から最小 language descriptor を生成する。
- language wizard は少なくとも descriptor id、aliases、match suffix、fence label、source extension、test extension、source/test output rule、root module markdown name、quality default の有無を扱う。
- `mds init descriptor tool` は command prefix、slot behavior、input/output mode、file arg 付与、diagnostic capture rule を扱う。
- `mds init descriptor package-manager` は metadata files、lockfiles、metadata reader、script/command fallback を扱う。
- wizard は通常 path では最小 descriptor だけを生成し、diagnostic capture や advanced merge rule は advanced step とする。
- `mds descriptor check [path|--package <path>]` は descriptor TOML、source config、lock、解決結果を検証する。
- `mds descriptor explain <target>` は target authoring file、language key、command、package manager id のいずれかを受け、どの descriptor がどの origin から解決されたかを表示する。
- `mds descriptor schema --kind <kind> --format json-schema` は editor 補完や docs 生成に使える descriptor schema を出力する。
- `mds descriptor sources` は repo/user/global descriptor source と lock 状態を表示できる。
- `mds descriptor eject <id>` を提供する場合、resolved descriptor を package-local override の初期値として書き出す。通常の `init` は eject しない。
- `mds init` 本体は package 初期化の導線として、descriptor 不足時に `mds init descriptor` または descriptor source 追加を案内する。

## 状態遷移 / 不変条件

- wizard の preview / apply 結果は同じ descriptor content を示す。
- generated descriptor は `mds descriptor check` を通る。
- `descriptor explain` は runtime 解決順と同じ resolver を使う。
- schema 出力は実装が受け付ける descriptor schema と一致する。
- mds は言語固有 template を保守せず、descriptor schema と authoring wizard の質問だけを保守する。

## エラー / 例外

- kind 不明、必須回答欠落、出力 path 衝突は usage error または wizard validation error とする。
- `--force` なしで既存 descriptor を上書きしない。
- `descriptor check` は TOML parse error、required field 欠落、alias/suffix 衝突、package overview special file 衝突、tool diagnostic capture group 不整合を報告する。
- `descriptor explain` の target が解決不能な場合、探索した source と次に取れる action を表示する。
- network が必要な descriptor source 操作は offline / locked 状態を区別して診断する。

## 横断ルール

- descriptor authoring 支援は template 大量配布ではなく、schema-aware wizard、validation、explain、schema output で行う。
- CLI 出力は descriptor の origin、優先順位、衝突理由を人間と AI が読める形にする。
- docs は言語別完成 template 集ではなく、最小 descriptor、override、pack、diagnostic capture の recipe を中心にする。

## 検証観点

- `mds init descriptor language` が未知言語名でも最小 descriptor を生成できる。
- `mds descriptor check` が malformed descriptor と衝突を原因 file 付きで検出できる。
- `mds descriptor explain` が package-local / workspace / source pack / global の origin を表示できる。
- `mds descriptor schema` の出力を editor schema として利用できる。
- `mds init` が descriptor 不足時に builtin 追加ではなく authoring / source 追加へ誘導する。

## 関連資料

- `../shared/SPEC-descriptor-pack-resolution.md`
- `../shared/SPEC-language-extension-contract.md`
- `./SPEC-cli-command-surface-and-execution.md`
- `./SPEC-cli-init-and-new-workflows.md`
- `../../requirements/v1/REQ-quality-language-and-toolchain-independence.md`
- `../../requirements/v1/REQ-ux-human-ai-authoring-experience.md`
