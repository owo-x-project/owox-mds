# Task 001: Replace Quality Slot And Capture Resolution

## 目的

quality slot command と diagnostic capture rule の解決を built-in tool profile 依存から config/schema 依存へ移す。

## 前提条件

- language / file rule 解決が config/schema runtime へ移行している

## 作業内容

- `typecheck` `lint` `fix` `test` slot の command 解決経路を整理する
- capture rule の config/schema surface を実装へ反映する
- built-in tool profile 依存を fallback 範囲まで縮小する

## 完了条件

- quality slot と capture rule が config/schema から解決される
- capture rule の違う fixture でも同じ runtime 契約で動く

## 検証方法

- `mds/core` の quality / diagnostics test を実行する
- representative quality command と diagnostics capture を確認する

## 依存関係

- `index.md`
- `../phase-04-core-language-and-file-resolution/task-001-replace-language-identity-and-doc-profile-resolution.md`

## 成果物

- `mds/core/src/quality.rs`
- `mds/core/src/diagnostics.rs`
- `mds/core/src/config.rs`
- `mds/core/tests/`