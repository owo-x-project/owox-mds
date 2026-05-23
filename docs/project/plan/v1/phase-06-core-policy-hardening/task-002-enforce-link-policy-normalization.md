# Task 002: Enforce Link Policy Normalization

## 目的

package 単位の link policy 3 mode と `mds lint --fix` による文書正規化を v1 契約どおり固定する。

## 前提条件

- language / file rule 解決と quality runtime が config/schema 起点へ移行している

## 作業内容

- `wiki-only` `markdown-only` `mixed` link policy の解釈と validation を追加する
- `mds lint --fix` の wiki-link / Markdown link 正規化を追加する
- package policy と Markdown authoring rule の整合を確認する

## 完了条件

- link policy 違反が lint error または fix 対象として観測できる
- `lint --fix` が policy へ正規化できる

## 検証方法

- link policy 3 mode の parser / lint / fix test を追加する
- representative fixture で lint / fix の結果を確認する

## 依存関係

- `../phase-05-core-quality-and-doctor-runtime/task-001-replace-quality-slot-and-capture-resolution.md`
- `../../../specs/shared/SPEC-authoring-markdown-format.md`

## 成果物

- `mds/core/src/config.rs`
- `mds/core/src/markdown.rs`
- `mds/core/src/quality.rs`
- `mds/core/tests/`