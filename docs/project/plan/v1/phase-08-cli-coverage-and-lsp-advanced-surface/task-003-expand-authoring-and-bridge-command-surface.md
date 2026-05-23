# Task 003: Expand Authoring And Bridge Command Surface

## 目的

guided authoring の最低保証を保ったまま、editor 実装が再利用する remap / bridge command surface を v1 仕様に足る水準へ広げる。

## 前提条件

- workspace index と core remap 契約が安定している

## 作業内容

- section / fence / snippet / missing-section quick fix の guided authoring contract を不足分までそろえる
- generated-to-Markdown と Markdown-to-generated の command surface を見直す
- VS Code が references、rename、code action、formatting を橋渡しするための command / data surface を追加する

## 完了条件

- editor 側が再利用する command surface が spec どおり安定している
- remap 不能 case が誤った Markdown 位置を返さない

## 検証方法

- authoring / code action / command surface test を追加する
- VS Code handoff 想定の request / response を確認する

## 依存関係

- `../phase-07-cli-and-lsp-adoption-foundation/task-004-index-overview-test-and-refresh-events.md`
- `../phase-06-core-policy-hardening/task-001-preserve-diagnostic-remap-with-source-map.md`

## 成果物

- `mds/lsp/src/capabilities/completion.rs`
- `mds/lsp/src/capabilities/code_action.rs`
- `mds/lsp/src/server.rs`
- `mds/lsp/tests/`