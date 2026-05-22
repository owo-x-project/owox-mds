---
id: SPEC-lsp-authoring-navigation-remap
status: 承認済み
related:
  - ../shared/SPEC-authoring-markdown-format.md
  - ../shared/SPEC-ux-embedded-language-bridge.md
  - ../shared/SPEC-ux-navigation-and-traceability.md
  - ../../requirements/v1/REQ-ux-guided-editor-authoring.md
  - ../../requirements/v1/REQ-ux-language-aware-embedded-lsp-bridge.md
  - ../../requirements/v1/REQ-ux-navigable-mds-knowledge-graph.md
subproject: mds-lsp
---

# mds-lsp Authoring Navigation Remap

## 概要

`mds-lsp` が `mds file` 向けに提供する diagnostics、guided authoring、navigation、workspace index、generated remap 契約を定義する。

## 関連要求

- `REQ-ux-guided-editor-authoring`
- `REQ-ux-language-aware-embedded-lsp-bridge`
- `REQ-ux-navigable-mds-knowledge-graph`

## 入力

- workspace folders
- package root と `mds.config.toml`
- impl md / test md / `overview.md`
- generated file path と source map
- editor からの LSP request

## 出力

- diagnostics
- completion items
- code actions
- definition / references / symbol search 結果
- generated-to-markdown remap result
- Markdown-to-generated resolve result

## 挙動

- `mds-lsp` は workspace を index し、package、module、symbol、generated file、source map を保持する。
- `mds-lsp` は `mds file` の構造 validation を行い、必須 section、exports/imports documentation、split source/test、link 解決などを診断する。
- guided authoring の最低保証として、section 補完、fence label 補完、snippet 補完、missing-section quick fix を提供する。
- definition / references / symbols は `mds` の module、shared definition、link、generated remap を起点に解決する。
- generated file に対する位置情報は source map を用いて Markdown 正本へ remap できる。
- Markdown 正本位置から generated file 位置を解決する bridge command を提供できる。
- embedded language bridge のために、editor 実装が利用できる remap command と workspace command surface を提供する。

## 状態遷移 / 不変条件

- workspace index は package 構造と source map に整合すること。
- generated remap は誤った Markdown location を返さないこと。
- definition / references の構造参照解決は heuristic より優先すること。
- heuristic references は best-effort だが、構造参照を壊さないこと。

## エラー / 例外

- 未解決 link や symbol は diagnostics として返せる。
- 曖昧な definition は複数候補を返せる。
- source map で同一 span に戻せない generated range は remap 不能として扱う。
- 未index workspace や config 読み取り失敗時は partial capability に degrade できる。

## 横断ルール

- `mds-lsp` は言語ごとの専用 semantic implementation を増やすより、Markdown 構造理解と remap 契約へ責務を集中する。
- host editor 固有 UI や provider 接続は editor 側責務とし、`mds-lsp` はそれを支える protocol / data / remap を提供する。
- v2 拡張時も、`mds-lsp` は project-wide traceability 基盤へ自然に拡張できること。

## 検証観点

- 必須 section / link / documentation rules の diagnostics が返る。
- section / fence / snippet / quick fix により guided authoring が成立する。
- definition / references / symbol search が module と shared definition で動作する。
- generated remap command が Markdown 正本位置を返す。
- editor bridge が使う command surface が安定している。

## 関連資料

- `../shared/SPEC-authoring-markdown-format.md`
- `../shared/SPEC-ux-embedded-language-bridge.md`
- `../shared/SPEC-ux-navigation-and-traceability.md`
- `../../requirements/v1/REQ-ux-guided-editor-authoring.md`
- `../../requirements/v1/REQ-ux-language-aware-embedded-lsp-bridge.md`
- `../../requirements/v1/REQ-ux-navigable-mds-knowledge-graph.md`
