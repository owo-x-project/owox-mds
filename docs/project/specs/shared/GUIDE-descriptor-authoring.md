# Descriptor Authoring Guide

## 目的

descriptor を package-local に作り、検証し、必要に応じて source pack として再利用するための v1 guide。

この guide は `SPEC-descriptor-pack-resolution.md` と `SPEC-cli-descriptor-authoring-workflows.md` の利用者向け入口。CLI の細かな表示文言は実装に従うが、代表 command と責務分離は spec に合わせる。

## 基本方針

- `mds` 本体は language / tool / package-manager 固有 descriptor content を所有しない。
- package は `.mds/descriptors/**` の package-local descriptor、workspace shared descriptor、repo descriptor source、user/global store のいずれかから descriptor を解決する。
- v1 の再現性が必要な repo は `.mds/descriptor-sources.toml` と lock を使い、CI では lock 済み source を優先する。
- package-local descriptor は upstream source より優先され、override や eject の受け皿になる。
- origin metadata は diagnostics / doctor / `mds descriptor explain` で説明可能な状態を保つ。

## 最小 language descriptor

配置例:

```text
.mds/descriptors/languages/ts.toml
```

最小形:

```toml
id = "ts"
aliases = ["typescript"]
match_suffixes = ["ts"]

[language]
primary_ext = "ts"
root_module_markdown_names = ["index.ts.md"]

[files.source]
strip_lang_ext = false
prefix = ""
suffix = ""
extension = "ts"

[files.test]
strip_lang_ext = true
prefix = ""
suffix = ".test"
extension = "ts"

[quality_defaults]
lint = "npm run lint"
test = "npm test"
```

`id` は descriptor の主 key。`aliases` と `match_suffixes` は衝突検出対象。`files.source` / `files.test` は Markdown file name から generated file path への写像を決める。

`mds init descriptor language` は未知言語でも質問からこの最小形を作れる入口。既存 file を壊さないこと、生成結果が `mds descriptor check` を通ることが v1 の期待。

## tool descriptor

tool descriptor は command 固有の実行入出力や diagnostic capture を共有したい場合に使う。language descriptor の `[tooling.<slot>]` が primary で、tool descriptor は command-match fallback や source pack での再利用に向く。

配置例:

```text
.mds/descriptors/tools/eslint.toml
```

代表形:

```toml
id = "eslint"
aliases = ["eslint-js"]
command = "eslint"
slots = ["lint"]

[tooling.lint]
input = "stdin"
append_file_arg = true

[[tooling.lint.diagnostics]]
pattern = '^(?P<path>.+?):(?P<line>\d+):(?P<column>\d+): (?P<message>.+)$'
path_group = "path"
line_group = "line"
column_group = "column"
message_group = "message"
```

実装が受け付ける field は `mds descriptor schema --kind tool --format json-schema` を正本にする。guide の例は recipe であり、最終 schema と差分が出た場合は schema 出力を優先する。

## package-manager manifest

package-manager manifest は package metadata、lockfile、fallback command を読む入口。

配置例:

```text
.mds/descriptors/package-managers/npm.toml
```

代表形:

```toml
id = "npm"
aliases = ["node", "nodejs"]
display_name = "Node.js (npm)"
lang = "ts"
metadata_files = ["package.json"]
lockfiles = ["package-lock.json"]
metadata_reader = "node-package-json"

[commands]
install = "npm install"
build = "npm run build"
typecheck = "npm run typecheck"
lint = "npm run lint"
test = "npm test"
```

package-local package-manager manifest があると、`mds package sync` と quality slot fallback が package metadata / scripts を説明できる。

## diagnostic capture rule

diagnostic capture は tool output を Markdown 正本へ戻すための規則。

必須観点:

- `pattern` は path / line / column / message を capture できる形にする。
- `path_group`、`line_group`、`column_group`、`message_group` は capture 名と合わせる。
- `line_offset` が必要な tool だけ offset を指定する。
- capture 不能な output は generic tool failure として扱い、誤った Markdown 位置へ remap しない。

代表確認:

```bash
rtk mds descriptor check --package .
rtk mds lint --package .
```

## override

package-local descriptor は source pack より優先される。共通 source を使いながら package 固有の差分が必要な場合は、同一 `id` の descriptor を package-local に置く。

使い分け:

- 小さい差分: package-local override。
- resolved descriptor の複製から始めたい場合: `mds descriptor eject <id>` を提供する実装なら eject を使う。
- team 横断で再利用する差分: source pack へ昇格。

override は部分 merge を使う場合でも決定的でなければならない。衝突や曖昧な alias / suffix は `mds descriptor check` で原因 file 付き diagnostic にする。

## source pack recipe

source pack は descriptor の再利用単位。template 集ではなく、`languages/`、`tools/`、`package-managers/` を持つ descriptor source として扱う。

代表構成:

```text
descriptor-pack/
  languages/
    ts.toml
  tools/
    eslint.toml
  package-managers/
    npm.toml
```

repo 側の宣言例:

```toml
[[sources]]
id = "team-defaults"
path = "../descriptor-pack"
```

lock には resolved path、content hash、git rev など CI 再現に必要な pin を保持する。具体 field は implementation schema と `mds descriptor schema` に従う。

代表 command:

```bash
rtk mds descriptor sources --package .
rtk mds descriptor check --package .
rtk mds descriptor explain ts --package .
```

## command の使い分け

- `mds init descriptor language|tool|package-manager`: package-local descriptor を wizard / non-interactive input から作る。
- `mds descriptor check [path|--package <path>]`: descriptor TOML、source config、lock、衝突、diagnostic capture rule を検証する。
- `mds descriptor explain <target> --package <path>`: language key、command、package manager id、authoring file がどの origin へ解決されたか確認する。
- `mds descriptor schema --kind <kind> --format json-schema`: editor 補完、CI validation、docs 生成が参照する schema を出す。
- `mds descriptor sources --package <path>`: repo/user/global source と lock 状態を確認する。
- `mds descriptor eject <id>`: optional。resolved descriptor を package-local override の初期値にする。

## validation

descriptor 変更時の最小確認:

```bash
rtk mds descriptor check --package .
rtk mds descriptor explain <target> --package .
```

package flow への影響も確認する場合:

```bash
rtk mds package sync --package . --check
rtk mds build --package .
rtk mds lint --package .
```

source pack や lock を変えた場合は `mds descriptor sources` で origin と lock 状態を確認する。CI では network 依存の source fetch と lock 済み cache 利用を区別して診断できることを確認する。

## 関連資料

- `SPEC-descriptor-pack-resolution.md`
- `../mds-cli/SPEC-cli-descriptor-authoring-workflows.md`
- `SPEC-language-extension-contract.md`
- `../../validation.md`
