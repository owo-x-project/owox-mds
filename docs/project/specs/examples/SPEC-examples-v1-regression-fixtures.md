---
id: SPEC-examples-v1-regression-fixtures
status: 承認済み
related:
  - ../../requirements/v1/REQ-product-markdown-source-of-truth.md
  - ../../requirements/v1/REQ-ux-human-ai-authoring-experience.md
  - ../../requirements/v1/REQ-quality-diagnostic-remap-to-mds.md
  - ../../validation.md
subproject: examples
---

# v1 Regression Fixtures

## 概要

`examples/` を v1 でどのような回帰 fixture として扱うか、その最小必須セット、repository 構成、将来拡張余地を定義する。

## 関連要求

- `REQ-product-markdown-source-of-truth`
- `REQ-ux-human-ai-authoring-experience`
- `REQ-quality-diagnostic-remap-to-mds`

## 入力

- `examples/` 配下の example package
- `mds.config.toml`
- package-local language descriptor in `.mds/descriptors/languages/*.toml`
- package-local package-manager manifest in `.mds/descriptors/package-managers/*.toml`
- source/test Markdown
- generated source/test output

## 出力

- v1 必須回帰 fixture セット
- repository に保持してよい example 構成
- examples を使う検証観点

## 挙動

- v1 の `examples/` の第一目的は onboarding 展示ではなく回帰 fixture とする。
- v1 の success-path 必須 fixture は `examples/minimal-ts` のみとする。
- `examples/broken-remap-ts` は diagnostic remap success / failure / no-op を固定する専用 live regression fixture とし、success-path の必須セットは広げない。
- 各 example は 1 つまたは少数の明確な学習 / 検証軸を持ち、過剰に多機能化しない。
- v1 fixture package は representative flow を package-local config / descriptor / package metadata、または lock 済み descriptor source だけで説明できる状態を保ち、歴史的な built-in registry 移行前提を必須経路にしない。
- `mds-cli` representative validation は対象 package cwd から `--package .` で実行し、repo root の config warning を fixture 結果に混ぜない。
- example package には source of truth である `.mds/source` `.mds/test` と、期待される generated output を含めてよい。
- `.build/` `target/` cache などの build artifact は example repository 構成から除外する。
- Python / Rust example は v1 repository fixture contract に含めない。
- v2 を見据えて、将来 additional language や project-wide document 管理例を追加できる directory 構造だけは妨げない。

## 状態遷移 / 不変条件

- v1 success-path 必須回帰 fixture は常に少なくとも 1 package 存在し、`minimal-ts` がその基準になる。
- remap regression が必要な場合は `broken-remap-ts` を success-path から独立して維持する。
- generated output は source of truth と対応しており、build 後の期待形として比較可能であること。
- build artifact は fixture の正本に含めないこと。
- example 追加時も、回帰軸と学習軸が説明できること。

## エラー / 例外

- build artifact を fixture と誤って repository に含める状態は不正とする。
- 必須 fixture が source/test/generated のいずれかを欠く場合、v1 examples として不完全とする。
- example が検証軸を過剰に混在させ、目的を説明できない場合は v1 fixture として不適切とする。

## 横断ルール

- examples は単なる展示ではなく、spec / implementation 変更時の回帰確認資産であることを優先する。
- generated output を含める場合でも、artifact と混同しない directory hygiene を保つこと。
- v2 拡張余地は残すが、v1 では code package の最小回帰 fixture に集中する。

## 検証観点

- `examples/minimal-ts` cwd で `package sync --check` / `build` / `lint` の代表確認ができ、`mds typecheck` / `mds test` は fixture config に従って success / no-op になる。
- `examples/broken-remap-ts` cwd で diagnostic remap success / failure / no-op の代表確認ができる。
- success-path fixture と broken/remap fixture の責務が混線していない。
- representative flow が package-local language descriptor / package-manager manifest 前提で通る。
- repository に build artifact が残っていない。
- generated output と source of truth の対応が説明可能である。
- examples 変更が requirement / spec / validation と整合する。

## 関連資料

- `../../requirements/v1/REQ-product-markdown-source-of-truth.md`
- `../../requirements/v1/REQ-ux-human-ai-authoring-experience.md`
- `../../requirements/v1/REQ-quality-diagnostic-remap-to-mds.md`
- `../../validation.md`
