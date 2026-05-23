---
id: SPEC-generation-safety-and-derivation
status: 承認済み
related:
  - ../../requirements/v1/REQ-quality-safe-package-bounded-generation.md
  - ../../requirements/v1/REQ-product-markdown-source-of-truth.md
  - ../../architecture.md
  - ../../validation.md
---

# Generation Safety And Derivation

## 概要

`mds file` から派生コードと関連生成物を作るときの safety contract と derivation contract を定義する共有仕様。

## 関連要求

- `REQ-quality-safe-package-bounded-generation`
- `REQ-product-markdown-source-of-truth`

## 入力

- package
- impl md / test md
- output pattern config
- existing generated files

## 出力

- generated source files
- generated test files
- copied assets
- manifest
- source map

## 挙動

- build は doc kind と output rule に従って派生ファイルを計画する。
- write mode は generated file を出力し、dry-run mode は出力計画だけを返す。
- 生成ファイルには管理 header と source hash を持たせる。
- source map は Markdown 正本と生成位置の対応を保持する。
- copied asset は source root 配下の非生成 Markdown / asset を対象にできる。

## 状態遷移 / 不変条件

- 出力先は常に package root 内に閉じる。
- `mds` 管理 header を持たない既存 file は自動上書きしない。
- source hash は生成元 Markdown の内容に対応する。
- generated file と source map は同じ generation plan から導出される。

## エラー / 例外

- package 外へ出る output path は error とする。
- unmanaged file 上書き要求は error とする。
- output pattern 展開不能や不正 placeholder は error とする。
- 読み取り不能 asset や doc は error とする。

## 横断ルール

- safety は言語別最適化より優先する。
- generated file 側の editor 機能は最終的に Markdown 正本へ戻せること。
- v2 拡張時も、派生物と正本の区別は保つこと。

## 検証観点

- dry-run と write の差分が安全に説明できる。
- unmanaged file 保護が効く。
- package 外 path が拒否される。
- source map を使って Markdown 正本へ戻れる。

## 関連資料

- `../../requirements/v1/REQ-quality-safe-package-bounded-generation.md`
- `../../requirements/v1/REQ-product-markdown-source-of-truth.md`
- `../../architecture.md`
- `../../validation.md`
