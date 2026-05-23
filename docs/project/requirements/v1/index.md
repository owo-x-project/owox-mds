# Requirements v1

## 役割

このディレクトリは v1 で満たす requirement 個票を置く。

## 置いてよいもの

- 初回提供で満たすべき requirement
- v1 の成功条件
- v1 の制約と品質条件

## 置いてはいけないもの

- v2 以降の将来要件
- 実装手段の詳細
- 一時的な作業メモ

## 命名規則

- `REQ-<category>-<short-title>.md`

## 参照

- `REQ-product-markdown-source-of-truth.md`: Markdown を実装正本として扱うための中核 requirement
- `REQ-quality-safe-package-bounded-generation.md`: package 境界と既存ファイルを守る安全性 requirement
- `REQ-quality-language-and-toolchain-independence.md`: 特定言語や単一ツールチェーンへ閉じない requirement
- `REQ-ux-human-ai-authoring-experience.md`: 人間と AI 両対応で、隠し marker に依存しない authoring 体験 requirement
- `REQ-ux-guided-editor-authoring.md`: `mds` 記法未習得でも editor 補助で作成できる requirement
- `REQ-ux-language-aware-embedded-lsp-bridge.md`: 言語自動検知と埋め込み code への既存 LSP 橋渡し requirement
- `REQ-ux-navigable-mds-knowledge-graph.md`: 依存、参照、定義へ簡単に移動できる requirement
- `REQ-ux-low-context-reference-layout.md`: 低コンテキスト消費で、視認しにくい補助 marker に頼らず必要情報へ到達できる参照配置 requirement
- `REQ-ux-section-title-independence.md`: section title 文字列に依存しない semantic 解釈 requirement
- `REQ-quality-diagnostic-remap-to-mds.md`: tool 診断の参照先を `mds file` へ戻す requirement
- `REQ-quality-portable-readable-verifiable-markdown.md`: 一般的 Markdown としての可読性と機械検証可能性 requirement
