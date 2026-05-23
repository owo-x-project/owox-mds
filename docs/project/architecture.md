# Architecture

## 目的

このファイルは、プロジェクト全体で守る不変条件、責務分離、設計方針を定義します。

## 読むべき場面

- 共通原則を変えるとき
- 責務境界を見直すとき
- 仕様や実装に横断影響があるとき

## 不変条件

- Markdown 実装文書が正本。生成コードは派生物として扱う。
- 生成物は `mds` 管理 header を持つ既存ファイルだけ上書きする。未管理ファイルは壊さない。
- 出力先は常に package root 内に閉じる。package 外への書き込みを許さない。
- package metadata の snapshot は実 package metadata と同期させる。差分があれば `mds package sync` を要求する。
- LSP と editor 連携は source map を介して Markdown 正本へ戻せることを前提にする。
- 言語 identity は impl md の file suffix、code fence label、package config / schema から決定できること。

## 責務分離

- `mds-core`: config / schema 解釈、package discovery、Markdown parsing、generation、quality、doctor、init/new の中核責務
- `mds-cli`: 引数解釈、対話式 init、実行結果表示、self-update の入口責務
- `mds-lsp`: workspace indexing、diagnostics、completion、navigation、generated file remap の責務
- `vscode-extension`: VS Code 上の言語登録、埋め込み code block 抽出、virtual document、LSP client bridge の責務
- `examples`: 言語別最小構成の回帰確認、仕様確認、開発者体験レビューの責務

## 設計方針

- 言語・ツールチェーン非依存を重視する。個別言語知識や tool 振る舞いは built-in 実装ではなく、package config と外部 schema へ追い出す。
- ドキュメントとして読みやすく、同時に source of truth として堅牢な Markdown authoring 体験を維持する。
- シンプルで分かりやすく、学習コストが低い構成を優先する。
- 市場の言語やツールチェーン変化に過度に引きずられず、長期保守コストが低い仕組みを優先する。
- 人間と AI の両方が読む・編集する前提で、記法の揺れや不確実さに耐える保守的な挙動を取る。
- quality、navigation、editor bridge は slot semantic、source map、capture rule を優先し、tool 固有ロジックの built-in 追加を抑制する。

## 関連資料

- `index.md`
- `validation.md`
- `tech-stack.md`
- `specs/mds-core/index.md`
- `specs/mds-cli/index.md`
- `specs/mds-lsp/index.md`
- `specs/vscode-extension/index.md`
- `specs/examples/index.md`
