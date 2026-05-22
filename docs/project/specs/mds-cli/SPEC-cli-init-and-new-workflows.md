---
id: SPEC-cli-init-and-new-workflows
status: 承認済み
related:
  - ../shared/SPEC-descriptor-pack-resolution.md
  - ../shared/SPEC-model-package-layout.md
  - ../shared/SPEC-authoring-markdown-format.md
  - ../mds-core/SPEC-core-config-and-authoring-policy.md
  - ../mds-core/SPEC-core-overview-and-package-sync.md
  - ./SPEC-cli-descriptor-authoring-workflows.md
  - ../../requirements/v1/REQ-ux-guided-editor-authoring.md
  - ../../requirements/v1/REQ-product-markdown-source-of-truth.md
subproject: mds-cli
---

# CLI Init And New Workflows

## 概要

`mds init` と `mds new` による初期化、template 起票、guided authoring 入口の契約を定義する。

## 関連要求

- `REQ-ux-guided-editor-authoring`
- `REQ-product-markdown-source-of-truth`

## 入力

- target package path
- interactive wizard input
- non-interactive flags
- `new` target path
- `new` doc kind
- template option

## 出力

- generated config / docs / AI kit files
- generated new document template
- init plan / setup plan
- generated descriptor bootstrap guidance

## 挙動

- `mds init` は v1 で wizard を正式サポートし、既定の entrypoint とする。
- `mds init --yes` は non-interactive apply mode とする。
- wizard の主目的は package 初期化と authoring policy 設定であり、AI kit は最後の optional step とする。
- wizard は固定 contract をむやみに入力させず、可変 policy だけを扱う。
- wizard は raw な section title 一覧を入力させず、project-level section semantic profile を設定対象とする。
- section semantic は project profile を基本とし、必要な場合だけ advanced で label override を編集できる。
- wizard は link policy と section semantic profile を初期 policy として設定できる。
- wizard の quality setup は language / tool 個別入力ではなく、`typecheck` `lint` `fix` `test` の semantic slot を基準にする。
- quality slot は package manager、existing scripts、package config / schema 由来の候補から自動検出し、通常は確認だけ、必要な場合だけ advanced override を行う。
- doctor 用の tool version floor は wizard の通常入力には含めず、`mds` 自身の version だけを config に自動記録する。
- v1 clean bootstrap では、対象 root に package-local package manager manifest が無い場合でも、既存 `package.json` があり `packageManager` field と lockfile が npm と矛盾しなければ、`init` は `.mds/descriptors/package-managers/npm.toml` と `.mds/descriptors/languages/ts.toml` を起票して後続 planning と quality summary を進める。
- npm 以外の package manager は、`init` 実行前に package-local package manager manifest を用意する。
- `init` は `mds.config.toml`、source overview、初期 source/test 文書、必要に応じて AI kit を起票できる。
- `init` は descriptor 不足時、言語固有 builtin 追加ではなく、`mds init descriptor <kind>` または descriptor source 追加を案内する。
- `mds init descriptor <kind>` は descriptor authoring wizard として扱い、詳細は `SPEC-cli-descriptor-authoring-workflows` に従う。
- `mds new` は template 起票 command として扱う。
- v1 の `new` contract は `mds new <path> <kind> [options]` を基本形とし、kind 未指定時は CLI が選択を促す。
- `new` は doc kind ごとに適切な template を選び、label override と language detection を反映する。
- `new` は impl md、test md、overview special file、root module doc などを起票できる。

## 状態遷移 / 不変条件

- `init` の preview / plan と apply 結果は整合すること。
- `new` で生成される path は canonical authoring root に配置されること。
- generated template は v1 authoring format と link policy に適合すること。
- generated template は project-level section semantic profile と整合すること。
- descriptor wizard で生成される TOML は `mds descriptor check` を通ること。

## エラー / 例外

- v1 clean bootstrap 条件を満たす `package.json` も package-local package manager manifest も無い場合、project init は拒否できる。
- 非管理 file 上書きは `--force` なしで行わない。
- `new` で kind や language を解決できない場合、選択または error に fallback する。
- descriptor 不足で language / package manager を解決できない場合、`init` は不足 descriptor の作成または descriptor source 追加を案内する。
- wizard cancel は file を書き込まず終了できる。

## 横断ルール

- `init` / `new` は guided authoring の最初の入口として、学習コストを下げることを優先する。
- `init` は大量の言語別 template を package にコピーしない。
- template は現実装の都合ではなく、shared spec の authoring contract に従うこと。
- wizard は tool 名や section title の細部を毎回入力させるより、semantic policy と自動検出を優先する。
- 将来 v2 へ広げるときも、初期化と新規文書作成の入口として拡張できること。

## 検証観点

- wizard と non-interactive init の両方で package を初期化できる。
- descriptor 不足時に `mds init descriptor` / descriptor source 追加へ誘導できる。
- wizard が section semantic profile、link policy、quality slot summary を扱える。
- `new` が kind ごとに適切な template を起票できる。
- label override と language detection が template に反映される。
- `--force` なしで unmanaged file を上書きしない。

## 関連資料

- `../shared/SPEC-model-package-layout.md`
- `../shared/SPEC-descriptor-pack-resolution.md`
- `../shared/SPEC-authoring-markdown-format.md`
- `../mds-core/SPEC-core-config-and-authoring-policy.md`
- `../mds-core/SPEC-core-overview-and-package-sync.md`
- `SPEC-cli-descriptor-authoring-workflows.md`
- `../../requirements/v1/REQ-ux-guided-editor-authoring.md`
- `../../requirements/v1/REQ-product-markdown-source-of-truth.md`
