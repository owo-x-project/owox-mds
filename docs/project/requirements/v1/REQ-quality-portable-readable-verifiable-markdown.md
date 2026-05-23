---
id: REQ-quality-portable-readable-verifiable-markdown
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../specs/mds-core/index.md
  - ../../specs/vscode-extension/index.md
---

# Portable Readable Verifiable Markdown

## リリース位置づけ

v1 要件。初回提供で満たすべき文書品質要件。

## 目標

`mds file` が専用 viewer なしでも一般的な Markdown として広く見やすく、同時に機械的に検証可能であること。

## 根拠

- ユーザーはドキュメントとしての見やすさを重視している。
- source of truth が専用 viewer 前提になると導入性と保守性が下がる。
- project は CLI / LSP / editor integration を持つが、正本自体は一般的な Markdown として読める必要がある。

## 対象範囲

- 一般的な Markdown 記法での表現
- 専用 viewer 非依存の可読性
- 構造、参照、code fence、section の機械検証可能性
- CLI / LSP による文書診断の対象範囲

## 対象外

- proprietary な表示拡張を前提とする記法
- 見た目だけを優先して構造一貫性を失う文書
- 機械検証不能な自由形式を標準にすること

## 成功指標

- 一般的な Markdown viewer で主要情報が読める。
- 専用 viewer がなくても見出し、参照、code block の意味が保たれる。
- 文書構造の主要制約を CLI / LSP が機械的に検証できる。
- 可読性改善と検証可能性が両立している。

## 制約 / 品質条件

- 一般的な Markdown 記法を優先し、独自記法は必要最小限に抑えること。
- 可読性と機械検証可能性のどちらか一方だけを犠牲にしないこと。
- editor integration は補助であり、正本の理解に必須であってはならないこと。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../specs/mds-core/index.md`
- `../../specs/vscode-extension/index.md`
