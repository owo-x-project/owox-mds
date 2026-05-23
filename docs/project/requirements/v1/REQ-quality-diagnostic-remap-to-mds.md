---
id: REQ-quality-diagnostic-remap-to-mds
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../specs/mds-core/SPEC-core-quality-and-fix-pipeline.md
  - ../../specs/mds-lsp/SPEC-lsp-authoring-navigation-remap.md
---

# Diagnostic Remap To mds

## リリース位置づけ

v1 要件。初回提供で満たすべき diagnostic traceability 要件。

## 目標

外部ツールが返す診断結果や参照先を、可能な限り `mds file` 上の位置へ変換し、利用者が正本の Markdown 側で問題を扱えること。

## 根拠

- ユーザーは tool 出力内の参照先を `mds` へ変換し、診断結果を `mds file` で扱いたい。
- source of truth が Markdown 側にある以上、generated file や一時 code 上の位置だけを見せる UX は不十分。
- CLI、LSP、editor が同じ正本位置を見られる必要がある。

## 対象範囲

- lint / typecheck / test / fix の tool diagnostics
- generated file や temp code から Markdown 正本への source map remap
- CLI / LSP / editor 上での診断表示位置

## 対象外

- path / line / column を含まない完全自由形式の出力を常に高精度で変換すること
- source map を持たない無関係な外部出力の remap
- 参照先情報が存在しない tool 出力を捏造して位置変換すること

## 成功指標

- path / line / column を持つ tool diagnostics を `mds file` 側へ remap できる。
- remap 後の診断を CLI、LSP、editor で一貫して扱える。
- remap 不能な場合は誤った Markdown 位置を出さず、失敗として扱える。

## 制約 / 品質条件

- remap は source map や等価な trace 情報に基づくこと。
- free-form stderr より、機械可読または capture 可能な出力形式を優先すること。
- 誤誘導より未変換を優先すること。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../specs/mds-core/SPEC-core-quality-and-fix-pipeline.md`
- `../../specs/mds-lsp/SPEC-lsp-authoring-navigation-remap.md`
