---
name: harness-init
description: テンプレート状態のハーネスを対象プロジェクト向けに初期化するときに使用する
---

## 目的

placeholder の `.agents/project.md` を確定させ、必要最小限のプロジェクト正本を初期化して、後続 task が進められる状態を作る。

## 前提資料

- `.agents/project.md`
- `.agents/skills/_shared/task-template.md`
- `.agents/skills/_shared/request-user-input-policy.md`
- `.agents/skills/harness-init/references/init-questionnaire.md`
- `.agents/skills/harness-init/references/project.template.md`
- `docs/project/index.md`

## やること

1. `.agents/tasks/task-harness-init.md` を作成し、初期化の文脈キャッシュにする
2. 固定アンケートに沿って `request_user_input` で必須項目を確認する
3. `.agents/project.md` の placeholder を確定値へ置き換える
4. `Kind` が `monorepo` の場合は必要な `docs/project/specs/<subproject>/index.md` を作成する
5. glossary、tech-stack、integrations、architecture、validation、team guide の初期化要否を段階的に確認する
6. `none` と `tbd` を分けて task に記録する
7. 初期化後に `harness-validation` で整合性確認を行う

## ルール

- 既存 placeholder を確定するまでは project 固有情報として扱わない
- 必須項目に未確認のまま残さない
- `none` と `未定` を混同しない
- docs/project にはプロジェクト正本だけを書く

## 確認事項

- `.agents/project.md` が確定値へ更新されている
- 必須アンケートに未確認項目が残っていない
- `none` と `tbd` が分かれている
- 後続 task が開始できる最低限の正本がある