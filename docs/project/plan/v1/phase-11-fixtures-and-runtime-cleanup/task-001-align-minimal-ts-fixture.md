# Task 001: Align Minimal Ts Fixture

## 目的

`examples/minimal-ts` を v1 の正式 success-path fixture として、quality scripts、managed region、generated output 比較が成立する状態にする。

## 前提条件

- core / CLI の policy と template 契約が固まっている

## 作業内容

- `package.json` に `typecheck` `lint` `format` `test` scripts と必要 dependency snapshot を追加する
- `mds.config.toml` の quality slot を scripts と整合する最小値へそろえ、v1 success-path fixture では `type_checker = false` `test_runner = false` を明示する
- `overview.md` に required managed region を正式配置する
- generated source/test output と manifest の期待比較が安定するよう fixture を整える

## 完了条件

- `minimal-ts` が build / package sync / representative quality command / package-local typecheck smoke / package-local test smoke を成功できる
- source/test Markdown、generated output、manifest、overview managed region の対応が説明できる

## 検証方法

- `rtk npm --prefix examples/minimal-ts install --no-package-lock`
- `examples/minimal-ts` cwd で `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- build --package .`
- `examples/minimal-ts` cwd で `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- package sync --package . --check`
- `examples/minimal-ts` cwd で `rtk cargo run --manifest-path ../../Cargo.toml -p mds-cli -- lint --package .`
- `rtk npm --prefix examples/minimal-ts run typecheck`
- `rtk npm --prefix examples/minimal-ts run test`

## 依存関係

- `../phase-07-cli-and-lsp-adoption-foundation/task-002-align-new-command-and-templates.md`
- `../phase-06-core-policy-hardening/task-003-enforce-overview-and-package-sync-contract.md`

## 成果物

- `examples/minimal-ts/package.json`
- `examples/minimal-ts/mds.config.toml`
- `examples/minimal-ts/.mds/source/overview.md`
- `examples/minimal-ts/.mds/manifest.toml`
- `examples/minimal-ts/src/`
- `examples/minimal-ts/tests/`
