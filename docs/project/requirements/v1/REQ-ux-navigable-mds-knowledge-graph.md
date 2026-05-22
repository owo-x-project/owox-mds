---
id: REQ-ux-navigable-mds-knowledge-graph
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../specs/mds-lsp/index.md
  - ../../specs/vscode-extension/index.md
---

# Navigable mds Knowledge Graph

## リリース位置づけ

v1 要件。初回提供で満たすべき navigation と探索性要件。

## 目標

利用者が `mds file` の依存、参照、定義、関連箇所へ簡単に飛び、必要情報を探索できること。

## 根拠

- ユーザーは依存や参照、定義などに簡単に飛べることを求めている。
- `mds` は source of truth として複数文書のつながりを扱うため、探索性が低いと維持コストが上がる。
- AI と人間の両方にとって、移動しやすさは token / context 節約にも効く。

## 対象範囲

- definition への移動
- references の列挙
- imports / exposes / shared definition / module 間の移動
- project 内の関連文書探索

## 対象外

- すべての静的解析を言語ごとに完全再実装すること
- editor 外の任意 UI へ依存した探索機構
- source of truth と無関係な汎用全文検索だけを要件とすること

## 成功指標

- 利用者が主要な定義元と参照先へ editor 操作から移動できる。
- module、shared definition、依存関係の探索起点を提供できる。
- generated file 由来の位置も Markdown 正本へ戻して辿れる。
- 探索機能により関連箇所確認のコストが下がる。

## 制約 / 品質条件

- navigation は `mds file` の論理構造を起点にすること。
- bridge や remap がある場合も、最終的な到達点は利用者に理解しやすいこと。
- 探索性改善は可読性や構造単純性を壊さないこと。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../specs/mds-lsp/index.md`
- `../../specs/vscode-extension/index.md`
