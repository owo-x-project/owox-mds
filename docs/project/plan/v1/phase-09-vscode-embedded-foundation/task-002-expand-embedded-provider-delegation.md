# Task 002: Expand Embedded Provider Delegation

## 目的

embedded code block から references、rename、edit-backed code action、formatting まで host editor provider を再利用できる状態にする。

## 前提条件

- extension が virtual / shadow surface を安定して作れる

## 作業内容

- references、rename、edit-backed code action、formatting provider を登録し、既存 completion / hover / definition bridge とそろえる
- generated surface は `mds.resolveGeneratedPosition` `mds.remapGeneratedRange` `mds.remapGeneratedLocations` `mds.remapGeneratedTextEdits` `mds.remapGeneratedTextDocumentEdits` を使い、text edit、workspace edit、location list を Markdown 正本位置へ再対応付けする
- shadow surface は `mds-embedded` URI query の `source` / `startLine` と既存 block offset map を使い、current block 以外を含む location / text edit / workspace edit を Markdown 正本へ戻す
- v1 の code action scope は edit-backed action の edit / diagnostics remap と command strip までに限定する
- command-only code action は generic safe bridge 対象外として unavailable / no-op degrade を許容し、raw embedded command を流さない
- provider 不在時の degrade 動作を既存 bridge と同じ方針へそろえる

## 完了条件

- provider がある言語で references / rename / edit-backed code action / formatting が `mds file` 内から再利用できる
- edit / location remap が Markdown 正本に戻る
- edit-backed code action は remap 後に command strip され、command-only code action は unavailable / no-op へ degrade しても raw embedded command をそのまま流さない

## 検証方法

- extension compile と bridge scenario 確認を行う
- `rtk git diff --check` を通す
- sample block 上で各 provider の request / remap を確認する

## 依存関係

- `index.md`
- `../phase-08-cli-coverage-and-lsp-advanced-surface/task-003-expand-authoring-and-bridge-command-surface.md`

## 成果物

- `editors/vscode/src/extension.ts`
- `editors/vscode/package.json`