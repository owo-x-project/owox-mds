---
id: SPEC-authoring-markdown-format
status: 承認済み
related:
  - ../../requirements/v1/REQ-ux-low-context-reference-layout.md
  - ../../requirements/v1/REQ-ux-section-title-independence.md
  - ../../requirements/v1/REQ-quality-portable-readable-verifiable-markdown.md
  - ../../requirements/v1/REQ-product-markdown-source-of-truth.md
  - ../../architecture.md
  - ../../validation.md
---

# Authoring Markdown Format

## 概要

一般 `mds file` の section 構造、リンク記法、可読性、検証可能性を定義する共有仕様。

## 関連要求

- `REQ-ux-low-context-reference-layout`
- `REQ-ux-section-title-independence`
- `REQ-quality-portable-readable-verifiable-markdown`
- `REQ-product-markdown-source-of-truth`

## 入力

- impl md
- test md
- authoring format policy in config

## 出力

- 機械検証可能な `mds file`
- 可読な section 構造
- 参照解決可能な link surface

## 挙動

- 一般 `mds file` は Markdown 見出しと backtick code fence を基本構造とする。
- section の意味は canonical semantic で扱い、表示 title はその semantic の 1 つの表現として扱う。
- この仕様は一般 `mds file` を対象とし、special file は各専用 spec の構造契約を優先する。
- `<!-- -->` のような HTML comment marker は、authoring rule、hidden metadata、進行マーカー、AI 向け隠し指示として使わない。
- source doc は少なくとも `Purpose` を持つ。
- runtime behavior を直接持つ impl md は `Source` を持つ。
- root module doc や spec 的 source doc で runtime behavior を直接持たないものは prose-only を許可する。
- test md は少なくとも `Purpose` `Covers` `Cases` `Test` を持つ。
- package は semantic ごとに label preset または label override を持てる。
- validation、template、completion、quick fix は表示 title ではなく canonical semantic を基準に動作する。
- 参照記法の標準 baseline は Obsidian 互換 wiki-link とする。
- v1 の link policy mode は `wiki-only` `markdown-only` `mixed` の 3 モードを正式サポートする。
- v1 の既定設定では `wiki-only` を用い、wiki-link のみを許容する。
- `mds lint --fix` は設定方針に従って wiki-link と Markdown link を相互変換できる。
- 参照は情報探索コストを下げる位置に置き、重複より link を優先する。

## 状態遷移 / 不変条件

- 文書の主要情報は見出し単位で探索できる。
- code block 外の prose だけ読んでも目的と参照先の導線が分かる。
- link policy は package 単位で一貫する。
- section semantic と表示 title の対応は package 単位で一貫する。
- viewer 固有機能がなくても、見出し、本文、code block 自体は読める。
- prose-only source doc は code generation 入力を持たない。

## エラー / 例外

- 必須 section 欠落は validation error とする。
- 未知の section title で semantic 解釈できない場合は validation error または unsupported warning とする。
- link policy に反する記法は lint error または fix 対象とする。
- code fence 整合性崩れや解釈不能構造は validation error とする。
- prose-only source doc に `Source` が無いこと自体は error にしない。

## 横断ルール

- 可読性と機械検証可能性の両立を優先し、どちらか片方だけに最適化しない。
- 可視本文に現れない marker に重要情報を埋め込まず、人間と AI の両方が通常の Markdown 面だけで理解できる形を優先する。
- section title independence は一般 `mds file` の semantic 解釈規則に適用し、`overview.md` managed section のような special file 固有の fixed visible heading 契約には自動適用しない。
- v2 で資料種別が増えても、見出し / prose / link / fence の基本 authoring model は共有可能であること。

## 検証観点

- impl md / test md の最小 section 構造が検証できる。
- wiki-link / Markdown link policy が package 単位で強制される。
- `lint --fix` で link policy へ正規化できる。
- prose と section 配置だけで関連情報へ辿れる。

## 関連資料

- `../../requirements/v1/REQ-ux-low-context-reference-layout.md`
- `../../requirements/v1/REQ-ux-section-title-independence.md`
- `../../requirements/v1/REQ-quality-portable-readable-verifiable-markdown.md`
- `../../requirements/v1/REQ-product-markdown-source-of-truth.md`
- `../../architecture.md`
- `../../validation.md`
