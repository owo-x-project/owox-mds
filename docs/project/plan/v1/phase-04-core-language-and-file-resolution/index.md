# Phase 04: Core Language And File Resolution

## 目的

language / file rule 解決を built-in 前提から config/schema runtime へ移し、後続 policy task が同じ core runtime を共有できる状態にする。

## 前提条件

- core の config/schema loader 境界が固定されている

## 完了条件

- language identity、doc profile、output rule、special file rule、root module rule が config/schema 起点で説明できる
- 後続の quality / overview / LSP task が built-in 依存なく着手できる

## 検証方法

- core の config / markdown / generation 系 test と representative fixture を確認する

## task 一覧

- `task-001-replace-language-identity-and-doc-profile-resolution.md`: language identity と doc profile 判定を置き換える
- `task-002-replace-output-special-file-and-root-module-rules.md`: output / special file / root module rule を置き換える

## 依存関係

- `../phase-03-core-schema-loader-boundary/task-001-establish-core-schema-loader-boundary.md`
- `../../../specs/shared/SPEC-language-extension-contract.md`
- `../../../specs/mds-core/SPEC-core-config-and-authoring-policy.md`

## 参照

- `task-001-replace-language-identity-and-doc-profile-resolution.md`: language / profile 判定の置換
- `task-002-replace-output-special-file-and-root-module-rules.md`: file rule の置換