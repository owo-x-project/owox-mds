---
id: REQ-ux-guided-editor-authoring
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../specs/mds-lsp/index.md
  - ../../specs/vscode-extension/index.md
---

# Guided Editor Authoring

## リリース位置づけ

v1 要件。初回提供で満たすべき editor authoring 補助要件。

## 目標

利用者が `mds` の書き方を事前に十分把握していなくても、LSP と editor 補助によって `mds file` を作成できること。

## 根拠

- ユーザーは `mds` 記法を完全に覚えていなくても authoring できる水準を求めている。
- 学習コストの低さは v1 の中核価値の一つである。
- `mds` authoring が補助なし前提だと、source of truth 運用の導入障壁が高くなる。

## 対象範囲

- `mds file` 作成時の completion
- 主要 section、code fence、snippet、構造ルールの editor 補助
- authoring 中の診断と修正誘導

## 対象外

- 補助なしで全利用者が即時に記法を習得すること
- 自然言語から完全自動で全文書を生成すること
- editor 非対応環境で同等 UX を保証すること

## 成功指標

- 新規利用者が editor 補助を使って最小の `mds file` を作成できる。
- section 名、fence label、主要構造を補完や snippet から選べる。
- 構造ミスや不足を authoring 中に診断できる。
- 記法の丸暗記がなくても基本的な作成フローを完了できる。

## 制約 / 品質条件

- 補助は `mds file` の可読性と一般的 Markdown 性を壊さないこと。
- 補助は特定言語だけに閉じず、共通 authoring 体験を優先すること。
- 補助がないと作成不能になる設計にしないこと。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../specs/mds-lsp/index.md`
- `../../specs/vscode-extension/index.md`
