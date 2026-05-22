---
id: SPEC-cli-command-surface-and-execution
status: 承認済み
related:
  - ../shared/SPEC-descriptor-pack-resolution.md
  - ../shared/SPEC-generation-safety-and-derivation.md
  - ../mds-core/SPEC-core-config-and-authoring-policy.md
  - ../mds-core/SPEC-core-quality-and-fix-pipeline.md
  - ./SPEC-cli-descriptor-authoring-workflows.md
  - ../../requirements/v1/REQ-ux-human-ai-authoring-experience.md
subproject: mds-cli
---

# CLI Command Surface And Execution

## 概要

`mds` CLI の command surface、実行 mode、package targeting、結果表示、exit code 契約を定義する。

## 関連要求

- `REQ-ux-human-ai-authoring-experience`

## 入力

- CLI command name
- global options
- package path
- current working directory

## 出力

- stdout summary
- stderr diagnostics
- exit code

## 挙動

- v1 の主要 command は `init` `new` `descriptor` `build` `lint` `typecheck` `test` `doctor` `package sync` `update` とする。
- `--package` により対象 package を指定できる。
- `build` は `write` と `dry-run` を持つ。
- `lint --fix` と `lint --fix --check` を区別できる。
- `doctor` は text と json format を持てる。
- package command は `sync` subcommand を持つ。
- descriptor command は `check` `explain` `schema` `sources` を持てる。`eject` は package-local override が必要な場合の optional subcommand とする。
- CLI は package discovery と core execution の入口を提供し、command ごとの制約違反は早期に usage error として返す。

## 状態遷移 / 不変条件

- usage error は core 実行前に検出できること。
- stdout は plan / summary / generated files を中心に表示し、diagnostics は stderr に分離すること。
- 成功、quality error、environment missing、internal error は異なる exit code で区別できること。

## エラー / 例外

- unknown command / option は usage error とする。
- command ごとに不正な option 組み合わせは usage error とする。
- package 未発見や validation error は command failure とする。
- internal failure は dedicated exit code を返す。

## 横断ルール

- CLI は `mds-core` の機能を人間と AI が使いやすい entrypoint に整形する責務を持つ。
- CLI は section semantic や quality slot などの semantic policy を前面に出し、個別ツール実装や表示名の細部を過度に露出しない。
- colorized output は補助であり、非 color 環境でも内容理解が失われないこと。

## 検証観点

- command surface が usage と一致する。
- invalid option combination が適切に拒否される。
- stdout / stderr / exit code の役割分離が保たれる。

## 関連資料

- `../shared/SPEC-generation-safety-and-derivation.md`
- `../shared/SPEC-descriptor-pack-resolution.md`
- `../mds-core/SPEC-core-config-and-authoring-policy.md`
- `../mds-core/SPEC-core-quality-and-fix-pipeline.md`
- `../../requirements/v1/REQ-ux-human-ai-authoring-experience.md`
