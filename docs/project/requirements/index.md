# Requirements

## 役割

このディレクトリは、何を実現するかを管理する正本です。

## 置いてよいもの

- 目標
- 根拠
- 成功指標
- 対象範囲と対象外
- 制約や品質条件

## 置いてはいけないもの

- 実装手段の詳細
- テスト手順の詳細
- 一時的な作業メモ

## 命名規則

- `v1/REQ-<category>-<short-title>.md`
- `v2/REQ-<category>-<short-title>.md`

## 参照ルール

- 要求変更時は spec / validation / ADR への影響を確認する

## v1 要件

- `v1/REQ-product-markdown-source-of-truth.md`: Markdown を実装正本として扱うための中核 requirement
- `v1/REQ-quality-safe-package-bounded-generation.md`: package 境界と既存ファイルを守る安全性 requirement
- `v1/REQ-quality-language-and-toolchain-independence.md`: 特定言語や単一ツールチェーンへ閉じない requirement
- `v1/REQ-ux-human-ai-authoring-experience.md`: 人間と AI 両対応で、隠し marker に依存しない authoring 体験 requirement
- `v1/REQ-ux-guided-editor-authoring.md`: `mds` 記法未習得でも editor 補助で作成できる requirement
- `v1/REQ-ux-language-aware-embedded-lsp-bridge.md`: 言語自動検知と埋め込み code への既存 LSP 橋渡し requirement
- `v1/REQ-ux-navigable-mds-knowledge-graph.md`: 依存、参照、定義へ簡単に移動できる requirement
- `v1/REQ-ux-low-context-reference-layout.md`: 低コンテキスト消費で、視認しにくい補助 marker に頼らず必要情報へ到達できる参照配置 requirement
- `v1/REQ-ux-section-title-independence.md`: section title 文字列に依存しない semantic 解釈 requirement
- `v1/REQ-quality-diagnostic-remap-to-mds.md`: tool 診断の参照先を `mds file` へ戻す requirement
- `v1/REQ-quality-portable-readable-verifiable-markdown.md`: 一般的 Markdown としての可読性と機械検証可能性 requirement

## v2 目標要件

- `v2/REQ-product-v2-project-wide-document-governance.md`: `mds` を project 全体の資料管理へ発展させる将来 requirement

## 参照

- `v1/index.md`: v1 要件の入口
- `v2/index.md`: v2 目標要件の入口
