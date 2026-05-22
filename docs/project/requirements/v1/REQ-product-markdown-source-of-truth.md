---
id: REQ-product-markdown-source-of-truth
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../specs/mds-core/index.md
  - ../../specs/examples/index.md
---

# Markdown Source of Truth

## リリース位置づけ

v1 要件。初回提供で満たすべき中核要件。

## 目標

利用者が Markdown 文書を実装正本として管理し、source と test の両方をそこから扱えること。

## 根拠

- このプロジェクトの中心価値は、コードそのものではなく Markdown 実装文書を正本に置くことにある。
- 利用者は実装意図とコード断片を同じ文脈で保ちたい。
- examples と CLI 実装は、`.mds/source` と `.mds/test` を起点に派生コードを扱う運用を前提にしている。

## 対象範囲

- source 用 Markdown 文書と test 用 Markdown 文書
- Markdown から派生する source / test 出力
- package 単位での build、lint、typecheck、test、doctor、package sync の運用

## 対象外

- generated file を手編集する運用
- Markdown を使わない別 authoring 形式の標準採用
- 単一言語専用ツールとしての最適化

## 成功指標

- 利用者が package 内で source / test Markdown を分けて管理できる。
- `mds build` で Markdown 正本から派生コードを生成できる。
- `mds lint` `mds typecheck` `mds test` が Markdown 正本を起点に品質確認できる。
- examples が source of truth 運用の最小構成として継続利用できる。

## 制約 / 品質条件

- 正本は人間が読めるドキュメントであり続けること。
- 正本は AI も処理しやすい一貫した構造を持つこと。
- source と test は意図的に分離し、後続の検証や生成先判定に使えること。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../specs/mds-core/index.md`
- `../../specs/examples/index.md`
