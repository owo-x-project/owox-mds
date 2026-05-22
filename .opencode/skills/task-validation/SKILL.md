---
name: task-validation
description: 検証、受け入れ確認、品質確認が主目的の task を開始し、実施結果を整理するときに使用する
argument-hint: "目的=<何を検証するか> 進め方=<自走|対話>"
---

## 目的

対象変更または現状について、定められた validation と完了条件に照らして検証し、結果、未達、残課題を明確にする。

## 前提資料

- `.agents/project.md`
- `.opencode/skills/_shared/task-template.md`
- `.opencode/skills/_shared/execution-modes.md`
- `.opencode/skills/_shared/request-user-input-policy.md`
- `docs/project/index.md`
- `docs/project/glossary/core.md`
- `docs/project/tech-stack.md`
- `docs/project/patterns/index.md`
- `docs/project/validation.md`
- 関連 requirement / spec / ADR / code / test

## やること

1. 対象 `.agents/tasks/task-*.md` を作成または更新し、検証対象、合格条件、証跡の取り方、停止条件を明文化する
2. 必要なら `question` で `目的`、`進め方`、優先検証項目や合格条件の解釈を確認する
3. `.opencode/skills/_shared/execution-modes.md` を参照し、今回を `自走` か `対話` のどちらで進めるかと、どこで確認を挟むかを task に残す
4. validation と関連正本に沿って検証を実施する
5. 項目ごとに結果、証跡、未達、未確認を整理する
6. validation 自体の不足があれば、その更新要否を task に残す

## ルール

- 合格条件が曖昧なまま判定しない
- 実施結果と推測を混ぜない
- 未確認項目を合格扱いしない
- `docs/project/tech-stack.md` と `docs/project/patterns/index.md` を読み飛ばさない
- 重大な未達は隠さない

## 確認事項

- 合格条件が明確である
- 目的と進め方が明確である
- 項目ごとの結果と証跡が整理されている
- `docs/project/tech-stack.md` と `docs/project/patterns/index.md` を参照した
- 未達と未確認が分かれている
- validation 更新要否を確認した