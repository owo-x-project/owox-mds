# Task 003: Adopt LSP Language Discovery

## 目的

`mds-lsp` の language discovery を built-in descriptor 前提から config/schema runtime 前提へ移す。

## 前提条件

- language identity と remap 契約が core で安定している

## 作業内容

- file suffix、fence、config/schema を使う language discovery を LSP 実装へ反映する
- active language 判定に必要な core API を built-in 前提から切り離す
- 後続の VS Code active context が再利用する discovery 契約をそろえる

## 完了条件

- LSP の language discovery が built-in descriptor 前提から外れる
- VS Code 側が再利用する active language 判定が runtime 契約で説明できる

## 検証方法

- LSP の language / bridge 関連 test を追加する
- representative fixture で active language 判定を確認する

## 依存関係

- `index.md`
- `../phase-06-core-policy-hardening/task-001-preserve-diagnostic-remap-with-source-map.md`

## 成果物

- `mds/lsp/src/`
- `mds/lsp/tests/`