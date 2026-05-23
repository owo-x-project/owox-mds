# Capability Schema Runtime Migration

## 背景

- 現行実装は built-in descriptor、tool registry、package manager registry への依存が強い。
- 仕様では、言語 identity は impl md の file suffix と code fence から判定でき、出力先は package config で決められる前提を置ける。
- 今後の新言語、新ツール対応で `mds` 本体に固有知識を継ぎ足す構造は、保守コストと追従コストを増やす。
- 一方で、diagnostic remap、source map、special file、quality slot、editor bridge などは引き続き共通 kernel として必要である。

## 提案内容

- built-in descriptor / tool profile / package manager registry 依存を縮小し、package config と外部 capability schema を中核にする。
- language identity は `*.lang.md` と code fence label を基準にする。
- package config には package 単位 policy と override を残し、language/file rule、special file rule、quality default、diagnostic capture rule は capability schema へ寄せる。
- quality integration は `typecheck` `lint` `fix` `test` の slot semantic を中心にする。phase 01 では public schema/config surface だけを固定し、command 解決の precedence / fallback / order は phase 05 で正式化する。
- diagnostic remap は source map と capture 可能な path / line / column 情報に基づいて行う。
- CLI wizard は tool 選択中心ではなく、section semantic、link policy、quality slot summary、AI optional branch を中心にする。

## Phase 01 で固定する最小 surface

### 配置形式と参照方法

- capability schema は既存 descriptor と同じ TOML file 形式を使い、対象 package root 配下 `.mds/descriptors/languages/*.toml` に置く。
- `mds.config.toml` に schema path は持たせない。`mds-core` と `mds-lsp` は対象 package / path から descriptor root を決め、その root 配下 schema を自動発見する。
- schema 参照は path 文字列ではなく `id` `aliases` `match_suffixes`、`language.primary_ext` を含む language contract と impl md suffix / code fence label の組で行う。

### capability schema に固定する項目

- language identity: top-level `id` `aliases` `match_suffixes`、`language.primary_ext`、`language.root_module_markdown_names`
- output rule: `files.source` `files.test` の `strip_lang_ext` `prefix` `suffix` `extension`
- special file rule: `[[special_files]]` の `match` `kind` `output` `root`
- quality slot default: `[quality_defaults]` の `typecheck` `lint` `fix` `test`、`[tooling.<slot>]` の `input` `output` `append_file_arg`
- diagnostic capture rule: `[[tooling.<slot>.diagnostics]]` の `pattern` `path_group` `line_group` `column_group` `message_group` `severity` `line_offset`

### package config に残す項目

- `package` `authoring` `check` / `checks` `roots` `output` `adapters` `quality` `doctor` `package_sync` / `package-sync` `labels`
- package config は package 単位 policy、output root / override、quality slot override、doctor expectation を担う。language file naming と diagnostic capture group 定義は持ち込まない。
- phase 01 は quality slot / capture の公開項目と責務境界だけを固定する。built-in registry から config/schema runtime へ移る migration fallback / 削除順序は phase 02 の task、config / package manager scripts / capability schema 間の quality slot / capture runtime precedence / order は phase 05 の task へ委ねる。
- output path 解決は package config の `roots.*` `output.*` と capability schema の `files.*` `special_files` を合成する。

### diagnostic capture rule の最小 schema

- diagnostic capture は `[[tooling.<slot>.diagnostics]]` 配下だけを phase 01 public surface とする。
- 必須項目は `pattern`。
- 任意項目は `path_group` `line_group` `column_group` `message_group` `severity` `line_offset`。既定値は `path` `line` `column` `message` `error` `0`。
- source map remap が使う最小情報は path / line / column / message。phase 01 では追加 metadata や tool 固有 capture variant までは固定しない。

## Phase 02 で固定する migration compatibility rules

### fallback の適用条件

- legacy fallback は、config/schema runtime だけでは既存 v1 flow を維持できない間に限り許可する。
- 対象は既に同梱済みの built-in descriptor、tool profile、package manager registry の読み取り互換だけとする。capability schema と package config で固定した public contract、および対象 package 自身の metadata 読み取りで同等の runtime data がそろった package / fixture では fallback を標準経路にしない。
- phase 02 が固定するのは compatibility window だけである。config、package manager scripts、capability schema 間の quality slot / capture precedence / fallback / order は phase 05 で正式化する。

### 禁止事項

- legacy fallback を理由に public schema/config surface を増やさない。
- 新言語、新ツール、新 package manager、新 diagnostic capture rule、新 quality slot semantic を built-in 追加で受けない。
- examples、fixtures、docs、後続 task で旧 built-in registry を新しい必須経路として説明しない。互換維持以外の dual maintenance を認めない。

### 停止条件と削除順序

- 対象 package / fixture の representative flow が capability schema と package config で固定した contract、および対象 package 自身の metadata 読み取りだけで説明でき、representative validation が legacy built-in descriptor / tool profile / package manager registry 前提なしで通った時点で、その対象の fallback を停止する。
- package manager metadata reader の実装場所が未確定でも、phase 02 の停止条件は変えない。
- fallback 停止後、旧 built-in 依存の再導入を禁止する。
- obsolete built-in cleanup と examples 整合は phase 11 `task-003-remove-obsolete-builtins-and-align-examples.md` で実施する。phase 02-10 は fallback 縮退と停止条件確認までとし、phase 11 以前に removal 完了を求めない。

## 代替案

- built-in descriptor を維持し、必要な言語と tool を都度追加する。
  - 不採用理由: 本体改修が増え、将来の拡張コストが高い。
- descriptor を完全廃止し、宣言も持たない。
  - 不採用理由: special file、quality capture、package metadata 読み取り、editor bridge の契約が失われる。
- built-in descriptor を残しつつ、config 上書きだけ許す。
  - 不採用理由: kernel と project policy の責務が曖昧なまま残る。

## 利点

- 新言語 / 新ツール対応の多くを config / schema 追加で進められる。
- `mds` 本体の変更頻度を下げられる。
- section semantic、link policy、quality slot、diagnostic remap など project ごとの差分を正しく宣言できる。
- v2 の project-wide traceability 拡張でも、kernel と policy の責務分離を維持しやすい。

## リスク

- schema 設計が弱いと、built-in descriptor より複雑で使いにくくなる。
- migration 途中では、旧 registry と新 schema が二重化する可能性がある。
- package manager metadata 読み取りや diagnostic capture の共通化が不十分だと、かえって project 側負担が増える。

## 残る未確定事項

- package manager metadata reader を schema にどこまで持たせるか。reader 配置未確定は phase 02 fallback 停止条件を保留する理由にしない

## 正式化先候補

- `docs/project/architecture.md`
- `docs/project/specs/shared/SPEC-language-extension-contract.md`
- `docs/project/specs/mds-core/SPEC-core-config-and-authoring-policy.md`
- `docs/project/specs/mds-core/SPEC-core-quality-and-fix-pipeline.md`
- `docs/project/specs/mds-cli/SPEC-cli-init-and-new-workflows.md`
- `docs/project/specs/mds-cli/SPEC-cli-init-wizard-screen-flow.md`
- `docs/project/plan/v1/`

## 関連資料

- `../index.md`
- `../../architecture.md`
- `../../specs/shared/SPEC-language-extension-contract.md`
- `../../specs/mds-core/SPEC-core-config-and-authoring-policy.md`
- `../../specs/mds-core/SPEC-core-quality-and-fix-pipeline.md`
