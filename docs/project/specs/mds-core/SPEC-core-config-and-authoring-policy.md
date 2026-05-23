---
id: SPEC-core-config-and-authoring-policy
status: 承認済み
related:
  - ../shared/SPEC-model-package-layout.md
  - ../shared/SPEC-authoring-markdown-format.md
  - ../shared/SPEC-language-extension-contract.md
  - ../../requirements/v1/REQ-product-markdown-source-of-truth.md
  - ../../requirements/v1/REQ-ux-section-title-independence.md
  - ../../requirements/v1/REQ-quality-language-and-toolchain-independence.md
subproject: mds-core
---

# mds-core Config And Authoring Policy

## 概要

`mds-core` が解釈する package config、capability schema、authoring policy、label override、doc kind 判定、link policy mode の契約を定義する。

## 関連要求

- `REQ-product-markdown-source-of-truth`
- `REQ-ux-section-title-independence`
- `REQ-quality-language-and-toolchain-independence`

## 入力

- `mds.config.toml`
- package root
- authoring doc path
- optional capability schema files in `.mds/descriptors/languages/*.toml`
- optional package manager manifests in `.mds/descriptors/package-managers/*.toml`
- package manager metadata / scripts

## 出力

- package config
- source/test root 判定
- doc kind / doc profile 判定
- label override
- link policy mode
- doctor policy と tool / version expectation
- quality slot input config と capture rule

## 挙動

- `mds-core` は `mds.config.toml` から `package` `authoring` `check` / `checks` `roots` `output` `adapters` `quality` `doctor` `package_sync` / `package-sync` `labels` を解釈し、対象 package root 配下 `.mds/descriptors/languages/*.toml` から capability schema を、`.mds/descriptors/package-managers/*.toml` から package manager manifest を自動発見する。
- `doctor.version_floor` は command 名または command path を key、`x` `x.y` `x.y.z` string を value に取る version floor map として解釈する。
- v1 clean では legacy `[doctor].required` / `[doctor].optional` は受理しない。利用者へ明示 diagnostic を返し、tool policy は `quality.<lang>.required` / `quality.<lang>.optional` へ移す。
- v1 の `mds.config.toml` は capability schema path を持たない。schema 参照は active package root 配下の directory scan と language identity 解決で行う。
- v1 の canonical root は `.mds/source` `.mds/test` とし、config でもこの root を用いる。
- link policy mode は package 単位で `wiki-only` `markdown-only` `mixed` の 3 モードを持つ。
- link policy の既定値は `wiki-only` とする。
- label preset と label override は canonical semantic に対する表示名マッピングとして package 単位で設定できる。
- doctor policy は `quality.<lang>.required` / `quality.<lang>.optional` と `doctor.version_floor` から解釈できる。
- phase 01 で固定する capability schema surface は `id` `aliases` `match_suffixes`、`language.primary_ext` `language.root_module_markdown_names`、`files.source` / `files.test` の `strip_lang_ext` `prefix` `suffix` `extension`、`[[special_files]]` の `match` `kind` `output` `root`、`[quality_defaults]` の `typecheck` `lint` `fix` `test`、`[tooling.<slot>]` の `input` `output` `append_file_arg`、`[[tooling.<slot>.diagnostics]]` の `pattern` `path_group` `line_group` `column_group` `message_group` `severity` `line_offset` とする。
- package config は package 単位 policy、output root / override、quality slot override、doctor expectation を担う。capability schema は language 単位 file rule、special file、quality default、diagnostic capture behavior を担う。
- descriptor `[[special_files]]` は language-specific output special file を定義する。package-level overview path（`.mds/source/overview.md` と任意の `.mds/test/overview.md`）は package layout special file として別契約で扱う。
- output path 解決は package config の `roots.*` `output.*` と capability schema の `files.*` `special_files` を合成する。
- module identity 解決は source/test markdown path から `.md` を外した後、descriptor `match_suffixes` の最長一致を優先して strip する。`match_suffixes` が空のときだけ `language.primary_ext` を path suffix fallback として使う。
- `language.root_module_markdown_names` は explicit な `*.md` 名だけでなく extensionless な module basename も受け付ける。extensionless entry は matched suffix strip 後の runtime markdown 名に対して評価する。
- phase 05 では quality slot command の runtime precedence を package config の explicit command、package config の explicit false、active package manager script、active package manager command、capability schema の `quality_defaults`、no external command の順に固定する。
- package config の explicit false と unset は別状態として保持し、explicit false は runtime fallback を止める。
- active package manager script / command と capability schema `quality_defaults` は、active package manager resolved language と target doc language が一致する場合だけ runtime fallback 候補になる。
- diagnostic capture の公開 input は capability schema の `[[tooling.<slot>.diagnostics]]` に固定し、runtime では `[tooling.<slot>]` behavior を primary、対象 package root 配下 `.mds/descriptors/tools/*.toml` と `.mds/descriptors/linters/*.toml` の command match を補助入力とする。global built-in tool manifest は使わない。
- source doc は path と内容から impl / spec / overview を判定できる。
- prose-only source doc は root module doc や spec 的 source doc として許容する。

## 状態遷移 / 不変条件

- 1 package の config 解釈結果は決定的であること。
- link policy は package 内で一貫し、validation と fix の両方に使われること。
- source/test root、doc kind、doc profile 判定は generation / validation / LSP で共有されること。

## エラー / 例外

- 不正 TOML や unsupported config value は error または warning とする。
- canonical root に反する構成は v1 では不正として扱える。
- enabled package に対応する package manager manifest が package root 配下に無い場合は error とする。
- 不明 label override key は error とする。
- link policy と文書内容が矛盾する場合、lint error または fix 対象とする。

## 横断ルール

- config 契約は CLI、LSP、VS Code extension から一貫して観測されること。
- 将来 v2 で資料種別が増えても、package 単位 policy 解釈の形は維持できること。

## 検証観点

- canonical root と doc kind が一貫判定される。
- 3 link policy mode が解釈される。
- label override が section / table column に反映される。
- prose-only source doc が意図どおり許容される。

## 関連資料

- `../shared/SPEC-model-package-layout.md`
- `../shared/SPEC-authoring-markdown-format.md`
- `../shared/SPEC-language-extension-contract.md`
- `../../requirements/v1/REQ-product-markdown-source-of-truth.md`
- `../../requirements/v1/REQ-ux-section-title-independence.md`
- `../../requirements/v1/REQ-quality-language-and-toolchain-independence.md`
