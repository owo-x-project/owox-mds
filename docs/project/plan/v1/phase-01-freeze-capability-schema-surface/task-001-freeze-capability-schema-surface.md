# Task 001: Freeze Capability Schema Surface

## 目的

language identity、output rule、special file rule、quality slot、diagnostic capture rule を表現する capability schema の最小項目を固定する。

## 前提条件

- proposal が active に存在する
- shared/core spec の更新方針が参照できる

## 作業内容

- capability schema で表現する最小項目を列挙する
- package config に残す項目と外部 schema に出す項目を分ける
- shared/core quality spec へ必要な参照更新を入れる

## 完了条件

- schema の最小項目一覧が proposal / spec で一貫している
- built-in 前提でしか説明できない必須項目が残らない

## 検証方法

- proposal、shared spec、core config spec、core quality spec を読み、language identity、output、capture の責務が一貫していることを確認する

## 依存関係

- `index.md`
- `../../../proposals/active/proposal-capability-schema-runtime.md`
- `../../../specs/shared/SPEC-language-extension-contract.md`
- `../../../specs/mds-core/SPEC-core-config-and-authoring-policy.md`
- `../../../specs/mds-core/SPEC-core-quality-and-fix-pipeline.md`

## 成果物

- `docs/project/proposals/active/proposal-capability-schema-runtime.md`
- `docs/project/specs/shared/SPEC-language-extension-contract.md`
- `docs/project/specs/mds-core/SPEC-core-config-and-authoring-policy.md`
- `docs/project/specs/mds-core/SPEC-core-quality-and-fix-pipeline.md`