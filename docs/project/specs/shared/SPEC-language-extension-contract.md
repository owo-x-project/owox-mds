---
id: SPEC-language-extension-contract
status: 承認済み
related:
  - ./SPEC-descriptor-pack-resolution.md
  - ../../requirements/v1/REQ-quality-language-and-toolchain-independence.md
  - ../../requirements/v1/REQ-ux-language-aware-embedded-lsp-bridge.md
  - ../../architecture.md
  - ../../tech-stack.md
---

# Language And Capability Schema Contract

## 概要

複数言語を共通 authoring model へ載せるための package config、capability schema、package manager 連携の責務境界を定義する共有仕様。

## 関連要求

- `REQ-quality-language-and-toolchain-independence`
- `REQ-ux-language-aware-embedded-lsp-bridge`

## 入力

- `*.lang.md` file naming
- code fence language label
- package config
- optional capability schema files in `.mds/descriptors/languages/*.toml`
- optional package manager manifests in `.mds/descriptors/package-managers/*.toml`
- optional descriptor sources and descriptor source lock
- package manager metadata / scripts

## 出力

- language identity
- output naming rule
- package metadata reader
- quality slot public surface
- diagnostic capture rule

## 挙動

- capability schema は対象 package root 配下 `.mds/descriptors/languages/*.toml` の standalone TOML として自動発見する。`mds.config.toml` に schema path は持たせない。
- package manager manifest は対象 package root 配下 `.mds/descriptors/package-managers/*.toml` の standalone TOML として自動発見する。v1 clean runtime は global built-in package manager registry を使わない。
- package-local descriptor が無い場合でも、workspace shared descriptor、repo descriptor source、user/global descriptor store から解決できる。解決順と lock は `SPEC-descriptor-pack-resolution` に従う。
- 言語 identity は impl md の file suffix と code fence language label を基準に決定する。
- capability schema の phase 01 最小 surface は `id` `aliases` `match_suffixes`、`language.primary_ext` `language.root_module_markdown_names`、`files.source` / `files.test` の `strip_lang_ext` `prefix` `suffix` `extension`、`[[special_files]]` の `match` `kind` `output` `root`、`[quality_defaults]` の `typecheck` `lint` `fix` `test`、`[tooling.<slot>]` の `input` `output` `append_file_arg`、`[[tooling.<slot>.diagnostics]]` の `pattern` `path_group` `line_group` `column_group` `message_group` `severity` `line_offset` とする。
- output naming rule は capability schema の `files.*` `special_files` と package config の `roots.*` `output.*` を合成して決定する。package config 側へ language file naming rule は持ち込まない。
- quality integration は `typecheck` `lint` `fix` `test` の slot semantic を中核にし、phase 01 では package config の slot override、package manager scripts、capability schema の `quality_defaults` が公開 input surface になる点だけを固定する。command 解決の precedence / fallback / order は phase 05 で正式化する。
- diagnostic capture rule は capability schema の `[[tooling.<slot>.diagnostics]]` に置く。phase 01 では `pattern` `path_group` `line_group` `column_group` `message_group` `severity` `line_offset` の公開 field と ownership だけを固定し、runtime での capture resolution は phase 05 で正式化する。
- v1 clean の representative flow では package-local language descriptor / package manager manifest、または lock 済み descriptor source から解決した descriptor を前提にする。fresh package では `mds init descriptor` と descriptor source 追加導線でこの前提へ入る。
- 新しい言語追加は、既存 authoring model を壊さず config / schema 追加で拡張できることを目標にする。
- editor integration は language identity を file suffix、fence、config / schema から発見できること。

## 状態遷移 / 不変条件

- 同一 package 内の language resolution は決定的であること。
- capability schema discovery は対象 package root に対して決定的であること。
- descriptor source lock がある場合、registry discovery は lock に対して決定的であること。
- 言語追加のために core の共通 authoring ルールを書き換えることを原則要求しない。
- quality slot public surface は slot semantic と package policy の責務境界を保つ。

## エラー / 例外

- 不明な language key は validation warning または error とする。
- 必要 config / schema / descriptor source 欠落で package 処理継続不能なら error とする。
- language identity が曖昧な file は editor / build の両方で不安定動作を起こさないよう拒否または診断する。

## 横断ルール

- v1 は複数言語を support するが、全言語即時対応は要求しない。
- mds 本体は言語固有 descriptor content を builtin として所有しない。
- v2 へ拡張しても、言語追加は config / schema 中心で進める。

## 検証観点

- `examples/minimal-ts` と `examples/broken-remap-ts` が package-local language descriptor または lock 済み descriptor source / package metadata 前提で同一 model に乗る。
- Python / Rust support の検証は v1 repository example ではなく、config / schema contract と focused test で行う。
- 新 language 追加時に共通 core 変更が最小で済む。
- quality slot public surface が言語ごとに一貫する。
- diagnostic capture と remap が source map 前提で成立する。

## 関連資料

- `../../requirements/v1/REQ-quality-language-and-toolchain-independence.md`
- `../../requirements/v1/REQ-ux-language-aware-embedded-lsp-bridge.md`
- `SPEC-descriptor-pack-resolution.md`
- `../../architecture.md`
- `../../tech-stack.md`
