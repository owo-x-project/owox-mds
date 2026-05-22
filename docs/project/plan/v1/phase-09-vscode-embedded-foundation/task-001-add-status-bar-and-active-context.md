# Task 001: Add Status Bar And Active Context

## 目的

VS Code 上で `mds file` の active language と doc kind を status bar から即座に把握できるようにする。

## 前提条件

- extension が cursor 位置の active code block 言語と doc kind を解決できる

## 作業内容

- status bar item を追加し、`mds <active-language> | <doc-kind>` 相当の表示を行う
- cursor 移動、editor 切り替え、unknown language、non-`mds` file の状態更新を整える
- config/schema runtime 前提の active language discovery を VS Code 側表示へ接続する

## 完了条件

- `mds-markdown` editor 上で active language / doc kind が追従表示される
- unknown / unavailable 状態でも誤表示しない

## 検証方法

- extension compile と簡易 manual test で cursor 移動時の表示更新を確認する
- provider 不在 language と non-`mds` file の表示も確認する

## 依存関係

- `index.md`
- `../phase-07-cli-and-lsp-adoption-foundation/task-003-adopt-lsp-language-discovery.md`
- `../phase-08-cli-coverage-and-lsp-advanced-surface/task-003-expand-authoring-and-bridge-command-surface.md`

## 成果物

- `editors/vscode/src/extension.ts`
- `editors/vscode/package.json`