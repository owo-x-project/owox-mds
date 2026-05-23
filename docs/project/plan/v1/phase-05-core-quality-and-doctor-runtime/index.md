# Phase 05: Core Quality And Doctor Runtime

## 目的

quality slot、diagnostic capture rule、doctor policy を config/schema runtime 起点へ移し、後続 authoring / CLI / LSP task が同じ policy を再利用できる状態にする。

## 前提条件

- language / file rule 解決が config/schema runtime へ移行している

## 完了条件

- quality slot、capture rule、doctor policy が package policy で説明できる
- 後続 task が hardcode tool profile を前提にしない

## 検証方法

- core の quality / doctor / diagnostics 系 test を確認する

## task 一覧

- `task-001-replace-quality-slot-and-capture-resolution.md`: quality slot と capture rule 解決を置き換える
- `task-002-move-doctor-policy-to-config-runtime.md`: doctor の required / optional / version floor policy を置き換える

## 依存関係

- `../phase-04-core-language-and-file-resolution/task-001-replace-language-identity-and-doc-profile-resolution.md`
- `../phase-04-core-language-and-file-resolution/task-002-replace-output-special-file-and-root-module-rules.md`
- `../../../specs/mds-core/SPEC-core-quality-and-fix-pipeline.md`
- `../../../specs/mds-cli/SPEC-cli-doctor-and-update.md`

## 参照

- `task-001-replace-quality-slot-and-capture-resolution.md`: quality / capture rule の置換
- `task-002-move-doctor-policy-to-config-runtime.md`: doctor policy の置換