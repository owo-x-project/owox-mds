# Phase 07: CLI And LSP Adoption Foundation

## 目的

core policy に依存する CLI / LSP の基盤追従を並行で進め、入口体験と workspace 基盤を v1 仕様へそろえる。

## 前提条件

- core の remap、link policy、overview/package sync 契約が固まっている

## 完了条件

- CLI init/new/descriptor と LSP language discovery / workspace index が新 runtime と矛盾しない
- 次 phase の CLI coverage と LSP advanced task が同じ基盤を再利用できる

## 検証方法

- CLI wizard / new test、LSP index / refresh test、representative fixture 確認を行う

## task 一覧

- `task-001-align-init-wizard-screen-flow.md`: wizard の画面責務を v1 にそろえる
- `task-002-align-new-command-and-templates.md`: `mds new` 契約と template を v1 にそろえる
- `task-003-adopt-lsp-language-discovery.md`: LSP の language discovery を config/schema runtime 前提へ移す
- `task-004-index-overview-test-and-refresh-events.md`: workspace index と refresh trigger を完成させる
- `task-005-add-descriptor-authoring-cli.md`: descriptor authoring / check / explain / schema CLI を追加する

## 依存関係

- `../phase-06-core-policy-hardening/task-001-preserve-diagnostic-remap-with-source-map.md`
- `../phase-06-core-policy-hardening/task-002-enforce-link-policy-normalization.md`
- `../phase-06-core-policy-hardening/task-003-enforce-overview-and-package-sync-contract.md`
- `../phase-03-core-schema-loader-boundary/task-002-add-descriptor-source-and-pack-resolution.md`
- `../../../specs/mds-cli/SPEC-cli-init-and-new-workflows.md`
- `../../../specs/mds-cli/SPEC-cli-descriptor-authoring-workflows.md`
- `../../../specs/mds-cli/SPEC-cli-init-wizard-screen-flow.md`
- `../../../specs/mds-lsp/SPEC-lsp-authoring-navigation-remap.md`

## 参照

- `task-001-align-init-wizard-screen-flow.md`: wizard flow の追従
- `task-002-align-new-command-and-templates.md`: `new` と template の追従
- `task-003-adopt-lsp-language-discovery.md`: LSP language discovery の追従
- `task-004-index-overview-test-and-refresh-events.md`: workspace index / refresh の完成
- `task-005-add-descriptor-authoring-cli.md`: descriptor authoring CLI の追加
