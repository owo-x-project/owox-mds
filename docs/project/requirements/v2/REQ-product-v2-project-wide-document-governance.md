---
id: REQ-product-v2-project-wide-document-governance
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../glossary/core.md
  - ../../specs/shared/index.md
---

# Project-Wide Document Governance

## リリース位置づけ

v2 目標要件。v1 完了後に拡張を検討する将来要件。

## 目標

`mds` を source と test の管理に留めず、プロジェクト全体の資料管理へ発展できること。

## 根拠

- ユーザーは `mds` の検証可能性と情報トレース性能を、実装文書以外の project 資料へも広げたい。
- 実装、仕様、検証、判断、調査の連続性が強まるほど、project 全体の保守効率が上がる。
- 現在の `docs/project/` 構造は、将来の横断資料管理へ接続できる下地になる。

## 対象範囲

- source / test 以外の project 文書
- requirement / spec / validation / ADR / research / proposal などとの trace
- 検証可能な資料管理と情報追跡

## 対象外

- v1 で project 全資料をただちに `mds` へ統合すること
- source code 生成と無関係な任意文書の無秩序な取り込み
- 文書管理だけを目的に source of truth 品質を下げる判断

## 成功指標

- v2 で source / test 以外の project 資料へ拡張する設計余地が残る。
- 実装、仕様、検証、判断の trace を一貫した形で辿れる。
- 拡張後も検証可能性と保守性を維持できる。

## 制約 / 品質条件

- v1 の中核価値を壊さず段階的に拡張すること。
- 資料管理拡張は trace 性と検証可能性に実利がある範囲で行うこと。
- source / test authoring 体験を悪化させる形で拡張しないこと。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../glossary/core.md`
- `../../specs/shared/index.md`
