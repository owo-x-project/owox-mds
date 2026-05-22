---
id: SPEC-examples-minimal-ts-fixture
status: 承認済み
related:
  - ./SPEC-examples-v1-regression-fixtures.md
  - ../../requirements/v1/REQ-product-markdown-source-of-truth.md
  - ../../requirements/v1/REQ-ux-human-ai-authoring-experience.md
  - ../../requirements/v1/REQ-quality-diagnostic-remap-to-mds.md
  - ../../validation.md
subproject: examples
---

# minimal-ts Fixture

## 概要

`examples/minimal-ts` を v1 の必須成功系 fixture としてどう構成し、何を検証できる状態に保つかを定義する。

## 関連要求

- `REQ-product-markdown-source-of-truth`
- `REQ-ux-human-ai-authoring-experience`
- `REQ-quality-diagnostic-remap-to-mds`

## 入力

- `examples/minimal-ts/mds.config.toml`
- `examples/minimal-ts/.mds/descriptors/languages/ts.toml`
- `examples/minimal-ts/.mds/descriptors/package-managers/npm.toml`
- `examples/minimal-ts/package.json`
- `examples/minimal-ts/.mds/source/overview.md`
- `examples/minimal-ts/.mds/source/*.ts.md`
- `examples/minimal-ts/.mds/test/*.ts.md`

## 出力

- generated source files
- generated test files
- manifest
- package sync 対象 overview
- quality 実行可能な minimal TypeScript package

## 挙動

- `minimal-ts` は v1 の必須成功系 fixture とする。
- `minimal-ts` は source of truth、generation、package sync、quality 実行、基本 navigation/link の代表確認に使えること。
- `minimal-ts` は package-local `ts` descriptor と `npm` package-manager manifest を同梱し、representative flow を local schema と package metadata だけで説明できること。
- `minimal-ts` は wiki-only link policy を採用する。
- `minimal-ts` の `package.json` には、少なくとも `typecheck` `lint` `format` `test` に対応する npm scripts を正式に持つ。
- `minimal-ts` の quality slot は config と scripts の組み合わせで実行可能な最小値へ揃える。
- v1 success-path fixture では `mds` 側 representative quality command を `lint` とし、`type_checker = false` `test_runner = false` で `mds typecheck` と `mds test` を明示 disable する。
- package-local の `typecheck` `test` scripts は generated output smoke 用に保持する。
- `minimal-ts` の source overview は `Package Summary` `Dependencies` `Dev Dependencies` の managed region を正式に持つ。
- `minimal-ts` は generated source/test output を repository に含め、期待生成結果として比較可能にする。
- diagnostic remap の失敗系確認は `minimal-ts` とは別 package に分離し、この fixture 自体は通常成功する状態を保つ。
- `minimal-ts` の `mds-cli` representative validation は package cwd から `--package .` で実行し、repo root の config warning を混ぜない。

## 状態遷移 / 不変条件

- `minimal-ts` は常に build / package sync / representative quality command を成功できること。
- source/test Markdown、generated output、manifest の対応が説明可能であること。
- fixture の主眼は少数の明確な検証軸に保ち、多機能な総合例へ肥大化しないこと。
- build artifact は repository fixture に含めないこと。

## エラー / 例外

- npm scripts 欠落で代表 quality slot が自然に実行できない状態は不正とする。
- source overview が managed region を欠く場合、package sync fixture として不完全とする。
- generated output が期待比較対象として保持されていない場合、回帰 fixture として不完全とする。

## 横断ルール

- `minimal-ts` は success-path fixture として安定性を優先する。
- diagnostic remap の失敗系は別 package へ分離し、成功系 fixture の理解容易性を保つ。
- init / new / build / lint / package sync と package-local typecheck/test smoke の利用体験を最小コストで追える構造にする。

## 検証観点

- `examples/minimal-ts` cwd で `mds build --package .` が成功する。
- `examples/minimal-ts` cwd で `mds package sync --package . --check` が managed region を検証できる。
- `examples/minimal-ts` cwd で `mds lint --package .` が成功する。
- representative flow が package-local language descriptor / package-manager manifest 前提で通る。
- wiki-only link policy と generated output 比較が成立する。

## 関連資料

- `./SPEC-examples-v1-regression-fixtures.md`
- `../../requirements/v1/REQ-product-markdown-source-of-truth.md`
- `../../requirements/v1/REQ-ux-human-ai-authoring-experience.md`
- `../../requirements/v1/REQ-quality-diagnostic-remap-to-mds.md`
- `../../validation.md`
