# Task 001: Align Init Wizard Screen Flow

## 目的

`mds init` wizard を v1 spec の Welcome / policy / optional branch / confirm 構造へそろえる。

## 前提条件

- core が section profile、link policy、quality slot summary を返せる

## 作業内容

- Welcome、Section Profile Preset、Custom Section Labels、Link Policy、Quality Summary、Quality Advanced、AI Kit、Confirm 画面を再構成する
- quality setup を tool 名入力中心から slot semantic summary 中心へ置き換える
- Confirm に差分要約、link policy、quality summary、AI kit 有無を集約する

## 完了条件

- wizard が spec の画面順と条件分岐を満たす
- 生成 plan / config / template が wizard の選択結果と整合する

## 検証方法

- wizard screen 遷移 test または snapshot を追加する
- cancel、default flow、custom label、advanced quality flow を確認する

## 依存関係

- `index.md`
- `../phase-06-core-policy-hardening/task-002-enforce-link-policy-normalization.md`
- `../phase-06-core-policy-hardening/task-003-enforce-overview-and-package-sync-contract.md`

## 成果物

- `mds/cli/src/wizard.rs`
- `mds/cli/src/main.rs`
- `mds/cli/tests/wizard_test.rs`