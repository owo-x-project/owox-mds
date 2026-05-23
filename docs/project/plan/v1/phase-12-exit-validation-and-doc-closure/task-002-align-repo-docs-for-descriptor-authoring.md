# Task 002: Align Repo Docs For Descriptor Authoring

## 目的

repo docs を mds 本体の言語・tool・package-manager 固有 descriptor content に依存しない v1 方針へそろえ、利用者が descriptor を書く、検証する、pack/source として再利用する導線を理解できる状態にする。

## 前提条件

- descriptor source / pack resolution の実装方針が spec と plan に反映されている
- `mds init descriptor` と `mds descriptor` の CLI contract が固まっている
- examples が package-local descriptor または lock 済み descriptor source 前提で説明できる

## 作業内容

- README / examples README / validation / spec 参照から stale な runtime 同梱 descriptor 前提を削除する
- descriptor authoring guide を追加し、最小 language descriptor、tool descriptor、package-manager manifest、diagnostic capture rule、override、source pack の recipe を整理する
- descriptor source / lock / origin / explain / check / schema の使い分けを repo docs に追加する
- `mds init` と `mds descriptor` の代表コマンドを docs に追加する
- examples が package-local descriptor または descriptor source 前提で理解できるよう説明を更新する
- VS Code bridge manual-only checklist の未実施リスクを `validation.md` と phase 12 の残リスクに明記する

## 完了条件

- repo docs が「mds 本体は言語固有 descriptor content を所有しない」方針と矛盾しない
- descriptor 作成から検証、再利用、pack/source 化までの利用者導線が docs から辿れる
- stale な runtime 同梱 descriptor / legacy registry の v1 必須経路説明が残っていない
- v1 exit validation の docs 参照整合に含められる

## 検証方法

- `rtk rg -n "built-in descriptor corpus|legacy built-in registry|builtin descriptor|package-local capability schema" README.md docs/project examples/README.md` で stale 表現を確認する
- descriptor guide の代表 command が spec と一致し、CLI 実装完了後に help と再照合できることを確認する
- phase 12 exit validation の docs closure と一緒にレビューする

## 依存関係

- `../phase-03-core-schema-loader-boundary/task-002-add-descriptor-source-and-pack-resolution.md`
- `../phase-07-cli-and-lsp-adoption-foundation/task-005-add-descriptor-authoring-cli.md`
- `task-001-run-v1-exit-validation-and-close-docs.md`
- `../../../specs/shared/SPEC-descriptor-pack-resolution.md`
- `../../../specs/mds-cli/SPEC-cli-descriptor-authoring-workflows.md`

## 成果物

- `README.md`
- `examples/README.md`
- `docs/project/validation.md`
- descriptor authoring guide
- updated spec / plan references
