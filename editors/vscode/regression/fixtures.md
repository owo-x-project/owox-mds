# VS Code Regression Fixtures

Phase 10 の VS Code 回帰確認は、下記 fixture を基準に行う。

## Canonical Workspace Fixture

- Repository root: `/workspace`
- Success-path fixture: `examples/minimal-ts`
- Success authoring source: `examples/minimal-ts/.mds/source/greet.ts.md`
- Success generated output: `examples/minimal-ts/src/greet.ts`
- Success test output: `examples/minimal-ts/tests/greet.test.ts`
- Success package config: `examples/minimal-ts/mds.config.toml`

## Broken Remap Fixture

- Package fixture: `examples/broken-remap-ts`
- Remap source doc: `examples/broken-remap-ts/.mds/source/foo/source-map.ts.md`
- Mixed remap doc: `examples/broken-remap-ts/.mds/source/foo/bar.ts.md`
- Generated output: `examples/broken-remap-ts/src/foo/source-map.ts`
- Generated test output: `examples/broken-remap-ts/tests/foo/source-map.test.ts`
- Package config: `examples/broken-remap-ts/mds.config.toml`
- Local tooling prerequisite: `rtk npm --prefix examples/broken-remap-ts install`

## What This Fixture Covers

- active language / doc kind 表示
- generated-file bridge remap
- embedded shadow-document bridge remap
- edit-backed code action / formatting / navigation の TypeScript host delegation
- diagnostic mirror の generated success / embedded success / remap failure suppression / unmanaged Markdown no-op / stale purge

## Automated Checks

- `rtk npm --prefix editors/vscode run test:diagnostics-regression`
- `rtk npm --prefix editors/vscode run compile`

`test:diagnostics-regression` は diagnostic mirror の state/cache path を no-harness で確認する。generated success、embedded success、remap failure suppression、unmanaged Markdown no-op、stale purge をここで分離確認する。bridge UX 全体は自動実行していないため、下記 checklist と併用する。

## Manual Session Setup

1. `/workspace` を VS Code で開く。
2. `rtk npm --prefix examples/broken-remap-ts install` を済ませる。local `prettier` / `typescript` 前提。
3. extension 開発ホストを起動する。
4. success-path 確認では `examples/minimal-ts/.mds/source/greet.ts.md` を開く。
5. remap regression 確認では `examples/broken-remap-ts/.mds/source/foo/source-map.ts.md` と `examples/broken-remap-ts/.mds/source/foo/bar.ts.md` を開く。
6. 必要に応じて `examples/broken-remap-ts/src/foo/source-map.ts` と `examples/broken-remap-ts/tests/foo/source-map.test.ts` を並べて差分と remap を確認する。