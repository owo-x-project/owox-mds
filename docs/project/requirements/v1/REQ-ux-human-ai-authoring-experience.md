---
id: REQ-ux-human-ai-authoring-experience
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../specs/mds-cli/index.md
  - ../../specs/mds-lsp/index.md
  - ../../specs/vscode-extension/index.md
  - ../../specs/examples/index.md
---

# Human And AI Authoring Experience

## リリース位置づけ

v1 要件。初回提供で満たすべき利用体験要件。

## 目標

人間と AI の両方が、低い学習コストで Markdown 正本を読み、編集し、確認できること。

## 根拠

- ユーザーは読みやすさ、分かりやすさ、低学習コスト、人間と AI の両対応を重視している。
- 既存実装は CLI、LSP、VS Code extension、examples を持ち、authoring 補助を提供している。
- 仕様変更時に examples 更新と使用感レビューを必須にしたいという明示要望がある。

## 対象範囲

- CLI による init / new / build / lint / typecheck / test / doctor / package sync / update
- LSP による diagnostics、navigation、補完、source map 連携
- VS Code extension による language registration、preview 補助、LSP client bridge
- examples を使う onboarding と仕様変更時レビュー

## 対象外

- 特定エディタ 1 種だけに閉じる利用体験
- AI 専用、または人間専用に偏ったフォーマット最適化
- examples を持たないまま仕様変更を進める運用

## 成功指標

- 新規利用者が CLI と examples を起点に最小構成を理解できる。
- authoring 中の主要な問題を editor 上で診断できる。
- generated file と Markdown 正本の位置対応を辿れる。
- 仕様や UX を変えた変更では examples も追従し、使用感レビューが行われる。

## 制約 / 品質条件

- 文章は読みやすく、編集規則は単純であること。
- `<!-- -->` のような HTML comment marker を、authoring rule、進行マーカー、AI 向け隠し指示として持ち込まないこと。
- UX 改善は source of truth モデルと安全性を壊さない範囲で行うこと。
- examples は単なる展示ではなく、継続的な回帰確認資産であること。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../specs/mds-cli/index.md`
- `../../specs/mds-lsp/index.md`
- `../../specs/vscode-extension/index.md`
- `../../specs/examples/index.md`
