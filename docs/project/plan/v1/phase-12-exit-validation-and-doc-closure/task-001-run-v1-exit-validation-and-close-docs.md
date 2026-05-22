# Task 001: Run V1 Exit Validation And Close Docs

## 目的

v1 完了判定に必要な build / test / compile / example command / docs 参照整合をまとめて確認し、残る docs 差分を閉じる。

## 前提条件

- `minimal-ts` と broken/remap fixture がそろっている
- `rtk npm --prefix examples/minimal-ts install --no-package-lock` と `rtk npm --prefix examples/broken-remap-ts install --no-package-lock` を実施し、package-local npm smoke に必要な local devDependencies が入っている
- core、CLI、LSP、VS Code の主要 spec 差分が解消されている

## 作業内容

- Rust workspace build / test、VS Code compile / diagnostics regression、examples command、package sync、diagnostic remap 確認を実施する
- `validation.md`、plan、spec、examples README の参照整合と完了状態を見直す
- v1 完了後に残るものを bug / polish / v2 項目へ切り分ける

## 完了条件

- v1 完了に必要な代表確認が一通り実施される
- docs 上の roadmap / validation / examples 参照が現実装と一致する
- 未完了事項が残る場合も、v1 blocking か post-v1 かを判定できる

## 検証方法

- `rtk cargo test` と必要な `rtk cargo build`。v1 release validation では Cargo warning も残さない
- `rtk npm --prefix editors/vscode run compile`
- `rtk npm --prefix editors/vscode run test:diagnostics-regression`
- `examples/minimal-ts` cwd で `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- package sync --package . --check`
- `examples/minimal-ts` cwd で `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- build --package .`
- `examples/minimal-ts` cwd で `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- lint --package .` を representative `mds` quality smoke として確認
- `examples/minimal-ts` cwd で `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- typecheck --package .` と `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- test --package .` を config-disabled surface の success / no-op 確認として実施
- `rtk npm --prefix examples/minimal-ts install --no-package-lock`
- `rtk npm --prefix examples/broken-remap-ts install --no-package-lock`
- `rtk npm --prefix examples/minimal-ts run typecheck` と `rtk npm --prefix examples/minimal-ts run test` を generated output の package-local smoke として実施
- `examples/broken-remap-ts` cwd で `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- build --package .`
- `rtk npm --prefix examples/broken-remap-ts run lint`
- `rtk cargo test -p mds-core --test parser_generation_mvp_test lint_preserves_mixed_remap_success_failure_and_noop_diagnostics`
- `rtk cargo test -p mds-core --test parser_generation_mvp_test lint_fix_remaps_second_code_fence_diagnostics_with_source_map`

## 補足

- `examples/minimal-ts` の blocking representative smoke は `package sync --check` / `build` / `lint` と package-local `npm run typecheck` / `npm run test` を基準にし、`mds typecheck` / `mds test` は config-disabled surface の success / no-op 確認として位置づける。
- example command は対象 package cwd から `--package .` で実行し、repo root `mds.config.toml` の unsupported config warning を validation output に混ぜない。
- `editors/vscode/regression/bridge-ux-checklist.md` の full bridge UX は manual-only、phase 12 では未実施、v1 blocking の自動確認は compile と diagnostics regression の pass を優先する。manual-only 結果は実施時だけ記録し、自動 gate と混同しない。

## 依存関係

- `../phase-11-fixtures-and-runtime-cleanup/task-001-align-minimal-ts-fixture.md`
- `../phase-11-fixtures-and-runtime-cleanup/task-002-add-broken-remap-fixture-and-live-regressions.md`
- `../phase-11-fixtures-and-runtime-cleanup/task-003-remove-obsolete-builtins-and-align-examples.md`

## 成果物

- `docs/project/validation.md`
- `docs/project/plan/v1/`
- `docs/project/specs/examples/`
- `examples/README.md`
