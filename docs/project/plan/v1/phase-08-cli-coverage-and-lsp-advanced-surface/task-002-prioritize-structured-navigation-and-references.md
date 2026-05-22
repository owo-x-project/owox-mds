# Task 002: Prioritize Structured Navigation And References

## 目的

definition / references / symbol 探索を structured-first に整え、heuristic reference を v1 の best-effort として分離できる状態にする。

## 前提条件

- workspace index が impl/test/overview を含めて更新される

## 作業内容

- module、shared definition、link、covers など構造参照を優先する navigation 実装へ見直す
- heuristic textual match は fallback とし、誤誘導を避けるルールを明確にする
- generated remap を含む navigation の返し方を整える

## 完了条件

- 構造参照がある場合は heuristic より先に返る
- heuristic references が best-effort として保たれ、誤誘導を抑えられる

## 検証方法

- navigation / references / symbols test を追加する
- success-path fixture と broken/remap fixture で link / symbol / remap を確認する

## 依存関係

- `../phase-07-cli-and-lsp-adoption-foundation/task-004-index-overview-test-and-refresh-events.md`

## 成果物

- `mds/lsp/src/capabilities/navigation.rs`
- `mds/lsp/src/capabilities/symbols.rs`
- `mds/lsp/tests/`