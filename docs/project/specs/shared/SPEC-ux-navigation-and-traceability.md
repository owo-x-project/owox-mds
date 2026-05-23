---
id: SPEC-ux-navigation-and-traceability
status: 承認済み
related:
  - ../../requirements/v1/REQ-ux-navigable-mds-knowledge-graph.md
  - ../../requirements/v1/REQ-ux-low-context-reference-layout.md
  - ../../requirements/v1/REQ-product-v2-project-wide-document-governance.md
  - ../../architecture.md
  - ../../validation.md
---

# Navigation And Traceability

## 概要

`mds file` 間の定義移動、参照探索、依存追跡、generated file から正本への trace を定義する共有仕様。

## 関連要求

- `REQ-ux-navigable-mds-knowledge-graph`
- `REQ-ux-low-context-reference-layout`
- `REQ-product-v2-project-wide-document-governance` を見据える

## 入力

- module 名
- shared definition 名
- wiki-link / Markdown link
- imports / exports / covers / generated location

## 出力

- definition target
- reference list
- related location list
- generated-to-markdown remap result

## 挙動

- module、shared definition、明示 link を起点に定義移動できる。
- 参照探索は構造化された参照だけでなく、heuristic な textual match も v1 の対象に含める。
- heuristic match は best-effort とし、構造参照より誤検出しやすいことを前提にする。
- 実装は将来の信頼度区別を妨げない形にする。
- generated file 由来の location は可能な限り Markdown 正本へ戻せる。

## 状態遷移 / 不変条件

- 構造参照がある場合、それを優先して解決する。
- remap 成功時、利用者の最終到達点は Markdown 正本を優先する。
- navigation は package 境界と module identity を壊さない。

## エラー / 例外

- 定義先が一意に定まらない場合、複数候補を返せる。
- heuristic 参照探索で誤検出があり得る場合でも、空振りより誤誘導を優先しない実装とする。
- remap 不能 location は fabricated Markdown location へ変換しない。

## 横断ルール

- v1 では heuristic を含めるが、構造参照より弱い保証として扱う。
- 将来は構造参照と heuristic 参照を区別できる余地を残す。
- v2 で project 全体資料へ広げる際も、trace contract を拡張できる形にする。

## 検証観点

- wiki-link / Markdown link / module / symbol から移動できる。
- references が構造参照と heuristic 参照を一定水準で拾う。
- generated file から Markdown 正本へ戻れる。
- 複数候補や未解決時の挙動が説明可能である。

## 関連資料

- `../../requirements/v1/REQ-ux-navigable-mds-knowledge-graph.md`
- `../../requirements/v1/REQ-ux-low-context-reference-layout.md`
- `../../requirements/v2/REQ-product-v2-project-wide-document-governance.md`
- `../../architecture.md`
- `../../validation.md`
