---
id: REQ-ux-language-aware-embedded-lsp-bridge
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../specs/mds-lsp/index.md
  - ../../specs/vscode-extension/index.md
  - ../../specs/mds-core/index.md
---

# Language-Aware Embedded LSP Bridge

## リリース位置づけ

v1 要件。初回提供で満たすべき言語認識と埋め込み code bridge 要件。

## 目標

`mds file` 内の code block について、対象言語を自動検知し、VS Code 上で確認でき、該当言語の既存 LSP 機能を中継利用できること。

## 根拠

- ユーザーは `mds file` が何の言語かを自動検知し、status bar などで確認できることを望んでいる。
- ユーザーは `mds-lsp` が言語ごとの専用実装を増やさず、既存言語 LSP を橋渡しして使えることを望んでいる。
- 言語別専用実装を積み増す設計は保守コストと追従コストを上げる。

## 対象範囲

- `mds file` の言語検知
- editor 上での active language 表示
- code block を仮想文書や等価な bridge に写像して既存言語 LSP へ接続する仕組み
- completion、hover、definition など埋め込み code に対する言語機能再利用

## 対象外

- 各言語ごとに `mds-lsp` 内へ独自解析器と専用 LSP 実装を持つこと
- すべての editor で同じ UI 表示を保証すること
- 言語検知だけ行い、埋め込み code への言語機能再利用を行わない構成

## 成功指標

- `mds file` の対象言語や active code block 言語を editor 上で確認できる。
- 埋め込み code に対して、該当言語の既存 LSP 機能を橋渡しして使える。
- 新しい言語を追加しても、原則として言語専用の `mds-lsp` 実装追加を必須にしない。
- bridge 後の結果を `mds file` 上の位置へ戻して扱える。

## 制約 / 品質条件

- 言語認識は `mds` の authoring 構造と矛盾しないこと。
- bridge は source map や位置対応を壊さないこと。
- 言語機能再利用は言語非依存方針と低保守コスト方針を優先すること。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../specs/mds-lsp/index.md`
- `../../specs/vscode-extension/index.md`
- `../../specs/mds-core/index.md`
