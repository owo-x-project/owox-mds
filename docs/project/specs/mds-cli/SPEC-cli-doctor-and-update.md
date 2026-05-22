---
id: SPEC-cli-doctor-and-update
status: 承認済み
related:
  - ../mds-core/SPEC-core-config-and-authoring-policy.md
  - ../../integrations/github-distribution.md
  - ../../requirements/v1/REQ-quality-safe-package-bounded-generation.md
subproject: mds-cli
---

# CLI Doctor And Update

## 概要

`doctor` と `update` command による環境診断と自己更新の契約を定義する。

## 関連要求

- `REQ-quality-safe-package-bounded-generation`

## 入力

- package config
- doctor format
- installed toolchains
- requested update version
- GitHub distribution endpoint

## 出力

- doctor summary
- doctor diagnostics
- update status

## 挙動

- `doctor` は package ごとに required / optional toolchain の可用性を確認する。
- toolchain version floor は `doctor.version_floor` で明示された場合に評価する。
- `doctor` は text と json の両方を返せる。
- `update` は v1 で GitHub Releases / install script を前提とした自己更新 command とする。
- `update --version <x.y.z>` は指定 version へ更新を試みる。
- version 未指定時は latest release を確認して更新する。

## 状態遷移 / 不変条件

- doctor は project を変更せず診断だけを返す。
- doctor の required tool 不足は environment error として観測できる。
- update は成功時だけ binary 置換を完了し、失敗時は失敗を明示する。

## エラー / 例外

- required toolchain 不足は doctor error とする。
- optional toolchain 不足は doctor warning とする。
- version floor が満たせない場合は doctor error または warning とする。
- GitHub 到達不能、install script 失敗、requested version 不在は update failure とする。
- latest release 取得失敗時は、手動で release version を選んで install script を実行できる recovery hint を返す。
- install script が non-zero で失敗した場合は、対象 version を付けた manual retry hint を返す。

## 横断ルール

- doctor の version policy は CLI に hardcode するより config に従う方向を優先する。
- update は GitHub distribution integration と整合すること。
- update 失敗時も利用者が手動復旧手段を理解できるメッセージを返すこと。

## 検証観点

- doctor が required / optional を区別して出力する。
- doctor が config 由来 version floor を評価できる。
- update が latest / explicit version の両方で動作する。
- update 失敗時に復旧ヒントを返す。
- latest fetch failure と install script non-zero failure の両方で manual recovery hint を返す。

## 関連資料

- `../mds-core/SPEC-core-config-and-authoring-policy.md`
- `../../integrations/github-distribution.md`
- `../../requirements/v1/REQ-quality-safe-package-bounded-generation.md`
