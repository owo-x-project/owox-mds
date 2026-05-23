# Validation

## 目的

このファイルは、変更時に確認すべき検証方針を記録します。

## 読むべき場面

- 変更後に何をどう確認すべきか整理したいとき
- 検証観点を追加または更新したいとき

## 検証項目

- `mds-core` `mds-cli` `mds-lsp` を変更したら、少なくとも対象 crate の build と test を確認する。
- `editors/vscode` を変更したら、repo root から `rtk npm --prefix editors/vscode run compile` を通す。diagnostic mirror を変えたら `rtk npm --prefix editors/vscode run test:diagnostics-regression` も確認する。
- 生成、config、schema、quality、source map 挙動を変えたら、少なくとも `examples/minimal-ts` を使って `package sync --check` / `build` / `lint` と package-local `npm run typecheck` / `npm run test` を確認する。v1 success-path fixture の `mds typecheck` / `mds test` は config-disabled surface の success / no-op 確認として別に位置づける。`mds-cli` 経由の example command は対象 package directory から `--package .` で実行し、repo root の `mds.config.toml` を base config として読ませない。
- `overview.md` / package sync 契約を変えたら、`.mds/source/overview.md` special file の fixed heading `### Package Summary` `### Dependencies` `### Dev Dependencies` と `package sync --check` / write mode の差分を focused test で確認する。managed section は `Architecture` semantic section 以降、`Rules` semantic section 直前までの package overview 領域だけを更新し、`architecture` / `rules` label override を使う package でも同じ semantic 境界で確認する。
- 仕様更新や authoring 体験変更がある場合、対応する `examples/` を必ず更新し、開発者体験と使いやすさをレビューする。
- diagnostic remap を変えたら、成功 / remap failure / no-op の parity を focused Rust test で固定し、`examples/broken-remap-ts` の build と package-local lint、必要なら `editors/vscode` の diagnostics regression も確認する。
- descriptor schema、descriptor source、lock、origin metadata、override、diagnostic capture rule を変えたら、`rtk mds descriptor check --package .` 相当の検証で TOML parse、required field、alias/suffix 衝突、source config / lock 整合、capture group 不整合を確認する。CLI 細部が未確定の間は `SPEC-cli-descriptor-authoring-workflows.md` と core focused test を基準にする。
- descriptor source pack を変えたら、`mds descriptor sources` / `mds descriptor explain` 相当で package-local / workspace / repo source / user-global store の origin と lock 状態を確認し、CI では lock 済み source で再現できることを確認する。
- `mds file` の構造ルールを変えたら、一般的な Markdown としての可読性と、機械検証可能性の両方を確認する。
- 参照配置や file 分割方針を変えたら、人間と AI の両方にとって探索コストが下がるか、少なくとも悪化しないかを確認する。
- LSP / VS Code extension の変更では、記法未習得の利用者でも completion / snippet / diagnostics 補助で最小 `mds file` を作れるか確認する。
- 埋め込み code bridge や言語認識を変えたら、active language 表示、LSP resolved language registry、言語 LSP 機能の再利用、Markdown 位置への再対応付けを確認する。
- navigation を変えたら、definition / references / related symbol 探索が `mds file` 起点で成立するか確認する。

## 実行メモ

- Rust workspace 全体確認は `rtk cargo test` と必要な `rtk cargo build` を基準にし、v1 release validation では Cargo warning も残さない。
- examples 回帰確認は `mds-cli` を使う実運用寄りのコマンドを優先し、v1 exit では `examples/minimal-ts` の `package sync --check` / `build` / `lint` と package-local `npm run typecheck` / `npm run test` を blocking representative smoke に置く。`mds typecheck` / `mds test` は config-disabled surface の success / no-op 確認として併記する。`examples/broken-remap-ts` は `build` と package-local lint を基準にする。
- example command は `examples/minimal-ts` または `examples/broken-remap-ts` を cwd にして `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- ... --package .` を使う。repo root から `--package examples/...` を実行すると root `mds.config.toml` の unsupported config warning が混入するため、v1 exit validation の標準手順にしない。
- VS Code bridge UX 全体は manual-only checklist を併用するが、v1 blocking の自動確認は `rtk npm --prefix editors/vscode run compile` と `rtk npm --prefix editors/vscode run test:diagnostics-regression` を優先する。manual-only bridge UX は completion、hover、definition、references、rename、formatting、edit-backed code action、active language refresh、Markdown range remap を対象にし、実施した場合だけ結果を記録する。未実施なら phase 12 の残リスクとして明記し、自動 gate と混同しない。
