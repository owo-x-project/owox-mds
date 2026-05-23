---
id: SPEC-ux-embedded-language-bridge
status: 承認済み
related:
  - ../../requirements/v1/REQ-ux-guided-editor-authoring.md
  - ../../requirements/v1/REQ-ux-language-aware-embedded-lsp-bridge.md
  - ../../architecture.md
  - ../../validation.md
---

# Embedded Language Bridge

## 概要

`mds file` 内の code block を既存言語機能へ橋渡しする shared contract と、editor 実装の責務境界を定義する共有仕様。

## 関連要求

- `REQ-ux-guided-editor-authoring`
- `REQ-ux-language-aware-embedded-lsp-bridge`

## 入力

- `mds file`
- active cursor position
- code block language label
- source map
- host editor language feature provider

## 出力

- active embedded language identity
- embedded completion / hover / definition / diagnostics result
- Markdown 正本位置へ remap された result

## 挙動

- `mds file` の active code block 言語は file suffix、fence、package config / schema から決定する。
- shared contract として、embedded code へ既存言語機能を橋渡しできることを要求する。
- `mds-lsp` は Markdown 構造理解、source map、generated remap、shared navigation 基盤を担う。
- editor integration は host editor 上の既存言語機能との接続、virtual surface、visible language indicator を担う。
- VS Code 実装では status bar に `mds <active-language> | <doc-kind>` のような形で、`mds` 文書であること、active language、doc kind を確認できることを v1 目標とする。
- bridge 対象は completion、hover、definition、references、rename、formatting と、generic safe bridge 可能な edit-backed code action を含む。command-only code action は v1 で degrade を許容する。
- shadow / virtual surface が `mds-embedded` URI を使う場合、URI query の `source` / `startLine` を使って current block 以外を含む location、text edit、workspace edit を Markdown 正本へ戻せること。

## 状態遷移 / 不変条件

- bridge 結果は利用者が理解できる Markdown 位置へ戻せること。
- bridge は言語ごとの `mds-lsp` 専用実装追加を必須にしない。
- active language 判定は同一 block に対して決定的であること。

## エラー / 例外

- 言語不明 block は bridge 対象外とし、診断または no-op を返す。
- host editor 側 provider 不在時は `mds` 独自診断を壊さず degrade する。
- command-only code action は generic safe bridge 対象外として unavailable / no-op degrade を許容する。
- remap 不能な bridge 結果は誤った Markdown 位置へ変換しない。
- host editor が特定機能を提供しない場合、その機能だけ unavailable として扱える。

## 横断ルール

- shared spec は橋渡し契約を定義し、editor 固有 UI 詳細は subproject spec へ委譲する。
- v2 拡張時は他 editor 実装を追加可能な責務分離を保つ。

## 検証観点

- active language を利用者が視認できる。
- embedded completion / hover / definition が既存言語機能から再利用される。
- references、rename、edit-backed code action、formatting なども provider がある範囲で再利用できる。
- generated / virtual result が Markdown 正本へ remap される。
- provider 不在時も authoring UX が破綻しない。

## 関連資料

- `../../requirements/v1/REQ-ux-guided-editor-authoring.md`
- `../../requirements/v1/REQ-ux-language-aware-embedded-lsp-bridge.md`
- `../../architecture.md`
- `../../validation.md`
