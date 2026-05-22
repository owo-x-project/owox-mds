# mds サンプルプロジェクト

このディレクトリには、mds の動作を確認するためのサンプルプロジェクトが含まれています。

## サンプル一覧

| ディレクトリ | 内容 | v1 位置づけ |
| --- | --- | --- |
| `minimal-ts` | TypeScript の最小構成 | 必須 success-path fixture |
| `broken-remap-ts` | diagnostic remap regression 用 TypeScript fixture | broken/remap regression fixture |

## 使い方

package-local smoke を走らせる前に、`examples/minimal-ts` と `examples/broken-remap-ts` の local devDependencies を install します。`broken-remap-ts` の `lint` / `format` は local `prettier` 前提です。broken/remap regression は `examples/broken-remap-ts` を使い、success-path smoke とは分離します。

どちらの fixture も `.mds/descriptors/languages/ts.toml` と `.mds/descriptors/package-managers/npm.toml` を package 内に持ち、representative flow は package-local config / descriptor / package metadata だけで説明できる状態を保ちます。v1 方針では同じ flow を lock 済み descriptor source からも説明できます。

phase 12 exit validation では、`minimal-ts` の blocking representative smoke を `package sync --check` / `build` / `lint` と generated output 向け package-local `npm run typecheck` / `npm run test` に置く。`mds typecheck` / `mds test` は fixture config に従う config-disabled surface の success / no-op 確認として併記する。`broken-remap-ts` は package-local lint を含む remap regression fixture として分離する。`mds-cli` representative command は対象 package directory から `--package .` で実行し、repo root の `mds.config.toml` warning を混ぜない。

```bash
# repo root から実行: package-local smoke 前提
rtk npm --prefix examples/minimal-ts install --no-package-lock
rtk npm --prefix examples/broken-remap-ts install --no-package-lock

# cwd: examples/minimal-ts
rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- package sync --package . --check
rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- build --package .
rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- lint --package .
rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- typecheck --package .
rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- test --package .

# repo root から実行: generated output の blocking package-local smoke
rtk npm --prefix examples/minimal-ts run typecheck
rtk npm --prefix examples/minimal-ts run test

# cwd: examples/broken-remap-ts
rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- build --package .

# repo root から実行: broken/remap package-local lint smoke
rtk npm --prefix examples/broken-remap-ts run lint
```

## サンプルの構成

v1 では `minimal-ts` を必須 success-path fixture、`broken-remap-ts` を remap regression fixture として扱います。

- `mds.config.toml` — mds の設定ファイル
- `.mds/descriptors/languages/*.toml` — package-local language descriptor
- `.mds/descriptors/package-managers/*.toml` — package-local package-manager manifest
- `package.json` — v1 fixture の package metadata
- `.mds/source/overview.md` — source root の overview
- `.mds/source/*.lang.md` — tableless source Markdown
- `.mds/test/*.lang.md` — tableless test Markdown

生成後、`src/` と `tests/` に package-local language descriptor と `mds.config.toml` の output policy を合成した派生コードが作られます。descriptor の作成や検証手順は `../docs/project/specs/shared/GUIDE-descriptor-authoring.md` を参照します。

`minimal-ts/package.json` には `typecheck` `lint` `format` `test` scripts を保持し、生成済み `src/` `tests/` を直接 quality 検証できます。v1 success-path fixture では `mds` 側 representative quality を `lint` に固定し、generated output smoke は package-local `typecheck` と `test` を blocking representative smoke に置きます。`mds typecheck` と `mds test` は config-disabled surface の success / no-op 確認として残します。`broken-remap-ts` は `foo/bar.ts.md` と `foo/source-map.ts.md` を基準に remap success / failure / no-op regression を切り出し、success-path の理解と検証を汚さない。
