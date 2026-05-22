# Task 003: Remove Obsolete Builtins And Align Examples

## 目的

新 runtime へ移行済みの obsolete built-in descriptor corpus / registry を完全削除し、examples を新前提へそろえる。

## 前提条件

- 新 runtime で主要機能が動作する

## 作業内容

- obsolete built-in descriptor corpus を完全削除する
- `mds-core` runtime から built-in descriptor / tool / package manager registry 合成を完全に外す
- `mds init` を v1 clean bootstrap へ置き換え、descriptor 不足時は `mds init descriptor` または descriptor source 追加へ誘導できるようにする
- examples の config / schema / docs を新 runtime 前提へ更新する
- cleanup 後も fixture と regression の責務分離が崩れないように整える
- docs の v1 必須経路説明と validation command から obsolete built-in corpus 前提を消す

## 完了条件

- examples が新 runtime で説明できる
- 旧 built-in descriptor / tool / package manager registry 依存が v1 必須経路から外れている
- obsolete built-in corpus directory が削除済み、または runtime に参照されない互換資産として v1 必須経路から明示的に外れている
- `rtk rg` で runtime / examples / docs の v1 必須経路に obsolete built-in descriptor corpus 前提が残っていないことを確認できる
- representative runtime flow が package-local capability schema / package manager manifest、または lock 済み descriptor source 前提で通る
- `mds init` の minimal bootstrap path が descriptor authoring / source 追加導線で後続 flow を説明できる

## 検証方法

- examples で package cwd から representative flow を実行確認する
- `mds init --yes` の minimal bootstrap path で descriptor 不足が authoring / source 追加導線として説明されることを確認する
- cleanup 後の config / schema 参照整合を確認する
- `rtk rg -n "mds/core/src/descriptors|built-in descriptor corpus|legacy built-in registry|obsolete built-in" docs/project/validation.md docs/project/plan/v1 docs/project/specs/examples examples/README.md` で stale docs 参照を確認する

## 依存関係

- `../phase-07-cli-and-lsp-adoption-foundation/task-001-align-init-wizard-screen-flow.md`
- `../phase-07-cli-and-lsp-adoption-foundation/task-003-adopt-lsp-language-discovery.md`
- `../phase-10-vscode-diagnostics-and-coverage/task-001-remap-diagnostics-and-add-extension-coverage.md`

## 成果物

- `examples/`
- built-in registry cleanup 関連実装
- `mds init` bootstrap 実装
