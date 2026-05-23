# V1 Roadmap

## 目的

capability schema migration と v1 completion を単一の実行順へ統合し、`mds-core` `mds-cli` `mds-lsp` `vscode-extension` `examples` を v1 完了状態へ安全に到達させる。

## スコープ

- config/schema runtime への移行
- core authoring policy、quality policy、overview/package sync 契約の完成
- CLI、LSP、VS Code extension の v1 仕様追従
- `examples/` fixture、live regression、最終 validation、docs closure

## 非スコープ

- `requirements/v2/` に属する project-wide document governance
- VS Code 以外の editor 実装追加
- v1 spec 対象外の Python / Rust example 復活

## 前提

- v1 requirement / spec は `docs/project/requirements/v1/` と `docs/project/specs/` を正本とする
- source map と diagnostic remap は Markdown 正本へ戻る契約を維持する
- language identity は file suffix、fence label、package config / schema から決定できる必要がある

## 完了定義

- mds 本体の言語・tool・package-manager 固有 descriptor content 依存が v1 必須経路から外れ、runtime 参照、docs の必須経路説明、examples validation が package-local descriptor または lock 済み descriptor source で説明できる
- core、CLI、LSP、VS Code、examples の代表挙動が v1 spec と fixture で観測できる
- plan / validation / examples / 関連 spec の参照が単一の `plan/v1/` 前提へそろう

## 現状

最終更新: 2026-05-23

### 検証済み

- `rtk cargo test`: 317 passed
- `rtk npm --prefix editors/vscode run compile`: passed
- `rtk npm --prefix editors/vscode run test:diagnostics-regression`: passed
- `examples/minimal-ts`: `package sync --check` / `build` / `lint` / `mds typecheck` / `mds test` passed
- `examples/minimal-ts`: package-local `npm run typecheck` / `npm run test` passed
- `examples/broken-remap-ts`: `mds lint --package .` passed

### 実装・仕様反映済み

- overview / package sync 契約は `Purpose` / `Architecture` / `Rules` と fixed visible heading managed section の仕様へ更新済み。
- descriptor / tool / package-manager TOML read / parse failure は診断へ流す実装と回帰 test を追加済み。
- LSP quick fix template は hidden HTML TODO を生成しない可視文言へ更新済み。
- VS Code embedded hover fallback は Markdown 正本 range へ remap 済み。
- VS Code extension は descriptor / config 変更時に active language / authoring root discovery を refresh する構成へ更新済み。
- descriptor source / pack resolution は core に実装済み。package-local / workspace / local source / locked git cache path、origin metadata、collision diagnostics、source config / lock diagnostics を focused test で確認済み。
- `mds init descriptor` と `mds descriptor check/explain/schema/sources` は CLI に実装済み。CLI integration test と representative command で確認済み。
- repo docs の descriptor authoring guide / pack guide / stale runtime 前提整理は更新済み。
- generated Markdown template の hidden HTML marker は削除済み。`mds init` / `mds new` / examples は可視 Markdown contract に従う。
- package sync の managed section 更新は `Architecture` semantic section 以降、`Rules` semantic section 直前までの package overview 領域に限定し、`architecture` / `rules` label override も同じ semantic として扱う。
- VS Code extension は LSP の resolved language registry command を primary source として使い、package-local / workspace shared / descriptor source 由来の言語解決と active context 表示をそろえる。
- `mds init` の npm / TypeScript bootstrap descriptor は init-only seed として明文化済み。runtime resolver の built-in fallback ではなく、書き出された package-local descriptor が正本になる。

### 未完了 / v1 blocking 候補

- VS Code bridge manual checklist は completion / hover / definition / references / rename / formatting / edit-backed code action / active language refresh / Markdown range remap を対象にする。現時点では manual-only / non-blocking の残リスクとして扱い、自動 gate 通過と混同しない。

### Phase 状態

| Phase | 状態 | メモ |
| --- | --- | --- |
| 01 schema surface | 部分完了 | descriptor surface は既存 spec に反映済み。新 descriptor pack/source 前提は phase 03 task で実装待ち。 |
| 02 migration compatibility | 部分完了 | built-in 依存を必須経路から外す方針は明文化済み。descriptor source 境界の追加反映待ち。 |
| 03 core schema loader boundary | 完了 | package-local / workspace / descriptor source / locked cache path の registry 解決と origin metadata を実装済み。 |
| 04 core language and file resolution | 完了 | representative flow と descriptor source 経由の解決を確認済み。 |
| 05 core quality and doctor runtime | 完了 | representative quality smoke と tool descriptor origin / explain 連携を確認済み。 |
| 06 core policy hardening | 部分完了 | overview、remap、link 周辺の代表確認は通過。最終 validation は phase 12。 |
| 07 CLI and LSP adoption foundation | 完了 | init/new/LSP 基盤に加え、descriptor authoring CLI を実装済み。 |
| 08 CLI coverage and LSP advanced surface | 完了 | descriptor CLI integration coverage を追加し、cargo test 通過。 |
| 09 VS Code embedded foundation | 完了 | LSP resolved language registry primary と descriptor/config refresh を実装済み。manual bridge UX は non-blocking checklist で別管理。 |
| 10 VS Code diagnostics and coverage | 完了 | compile と diagnostics regression 通過。manual bridge UX は phase 12 の manual-only 項目。 |
| 11 fixtures and runtime cleanup | 完了 | examples smoke 通過。descriptor source 前提への docs / cleanup 整合を更新済み。 |
| 12 exit validation and doc closure | 部分完了 | final validation と docs closure は実施済み。VS Code manual checklist は manual-only / non-blocking 残リスクとして記録済み。 |

## 依存関係

- `../../proposals/active/proposal-capability-schema-runtime.md`
- `../../specs/shared/SPEC-authoring-markdown-format.md`
- `../../specs/shared/SPEC-descriptor-pack-resolution.md`
- `../../specs/shared/SPEC-generation-safety-and-derivation.md`
- `../../specs/shared/SPEC-language-extension-contract.md`
- `../../specs/shared/SPEC-ux-embedded-language-bridge.md`
- `../../specs/shared/SPEC-ux-navigation-and-traceability.md`
- `../../specs/mds-core/SPEC-core-config-and-authoring-policy.md`
- `../../specs/mds-core/SPEC-core-overview-and-package-sync.md`
- `../../specs/mds-core/SPEC-core-quality-and-fix-pipeline.md`
- `../../specs/mds-cli/SPEC-cli-command-surface-and-execution.md`
- `../../specs/mds-cli/SPEC-cli-descriptor-authoring-workflows.md`
- `../../specs/mds-cli/SPEC-cli-init-and-new-workflows.md`
- `../../specs/mds-cli/SPEC-cli-init-wizard-screen-flow.md`
- `../../specs/mds-cli/SPEC-cli-doctor-and-update.md`
- `../../specs/mds-lsp/SPEC-lsp-authoring-navigation-remap.md`
- `../../specs/vscode-extension/SPEC-vscode-embedded-editor-experience.md`
- `../../specs/examples/SPEC-examples-v1-regression-fixtures.md`
- `../../specs/examples/SPEC-examples-minimal-ts-fixture.md`
- `../../validation.md`

## 検証方針

- 各 phase 完了時に、その phase の task 群が同じ前提で並行着手できる粒度へ分かれていることを確認する
- core policy 変更は warning-free の Rust test / fixture 確認、editor 変更は `rtk npm --prefix editors/vscode run compile` と必要な diagnostics regression、fixture 変更は package cwd からの representative command 確認で閉じる
- 最終 phase では build / test / compile / example command / docs 参照整合を横断確認する

## 参照

- `phase-01-freeze-capability-schema-surface/index.md`: schema surface の固定
- `phase-02-freeze-migration-compatibility/index.md`: migration policy の固定
- `phase-03-core-schema-loader-boundary/index.md`: core loader / descriptor source boundary の固定
- `phase-04-core-language-and-file-resolution/index.md`: language / file rule 解決の移行
- `phase-05-core-quality-and-doctor-runtime/index.md`: quality / doctor runtime の移行
- `phase-06-core-policy-hardening/index.md`: remap / link / overview policy の固定
- `phase-07-cli-and-lsp-adoption-foundation/index.md`: CLI / LSP 基盤の追従
- `phase-08-cli-coverage-and-lsp-advanced-surface/index.md`: CLI coverage / LSP advanced surface
- `phase-09-vscode-embedded-foundation/index.md`: VS Code 基盤 UX の追従
- `phase-10-vscode-diagnostics-and-coverage/index.md`: VS Code diagnostics / coverage の固定
- `phase-11-fixtures-and-runtime-cleanup/index.md`: fixture / cleanup の整備
- `phase-12-exit-validation-and-doc-closure/index.md`: exit validation、descriptor authoring docs、docs closure
