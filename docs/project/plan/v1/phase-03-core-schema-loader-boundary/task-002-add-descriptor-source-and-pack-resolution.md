# Task 002: Add Descriptor Source And Pack Resolution

## 目的

`mds-core` が言語・tool・package-manager 固有 descriptor content を本体に持たず、package-local / workspace / descriptor source / global store から descriptor registry を決定的に組み立てられる loader 境界を追加する。

## 前提条件

- schema surface と migration compatibility が固定されている
- `SPEC-descriptor-pack-resolution` が descriptor source / lock / origin / override の契約を定義している

## 作業内容

- descriptor source config と lock の読み取り境界を core に追加する
- package-local、workspace shared、repo descriptor source、user/global descriptor store の解決順を実装へ反映する
- descriptor origin metadata を registry に保持し、doctor / explain が再利用できる API として返す
- descriptor read / parse / source resolution failure を原因 file / origin 付き diagnostic にする
- package-local override と external source descriptor の衝突検出ルールを実装または明文化する

## 完了条件

- mds 本体の言語固有 descriptor content なしで、lock 済み descriptor source から registry を構築できる
- package-local descriptor が external source より優先される
- registry 解決結果に origin metadata が含まれる
- descriptor source / lock / TOML parse failure が診断として観測できる

## 検証方法

- `mds/core` の descriptor / package discovery 系 test を追加または更新する
- package-local override、workspace source、lock 済み source、衝突、malformed descriptor の focused test を確認する
- `rtk cargo test` を実行する

## 依存関係

- `task-001-establish-core-schema-loader-boundary.md`
- `../phase-02-freeze-migration-compatibility/task-001-define-migration-compatibility-rules.md`
- `../../../specs/shared/SPEC-descriptor-pack-resolution.md`
- `../../../specs/shared/SPEC-language-extension-contract.md`

## 成果物

- `mds/core/src/descriptor.rs`
- `mds/core/src/model.rs`
- `mds/core/tests/`
- descriptor source / lock fixtures
