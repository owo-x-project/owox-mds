---
id: SPEC-cli-init-wizard-screen-flow
status: 承認済み
related:
  - ./SPEC-cli-init-and-new-workflows.md
  - ../mds-core/SPEC-core-config-and-authoring-policy.md
  - ../mds-core/SPEC-core-quality-and-fix-pipeline.md
  - ../../requirements/v1/REQ-ux-guided-editor-authoring.md
  - ../../requirements/v1/REQ-ux-section-title-independence.md
  - ../../requirements/v1/REQ-quality-diagnostic-remap-to-mds.md
subproject: mds-cli
---

# CLI Init Wizard Screen Flow

## 概要

`mds init` wizard の v1 画面順、各画面の責務、条件分岐、入力項目を定義する。

## 関連要求

- `REQ-ux-guided-editor-authoring`
- `REQ-ux-section-title-independence`
- `REQ-quality-diagnostic-remap-to-mds`

## 画面一覧

1. Welcome
2. Section Profile Preset
3. Custom Section Labels (conditional)
4. Link Policy
5. Quality Summary
6. Quality Advanced (conditional)
7. AI Kit Toggle
8. AI Targets (conditional)
9. AI Categories (conditional)
10. Confirm

## 画面仕様

### 1. Welcome

- 目的: 何を初期化するかを短く説明し、固定 contract と可変項目を区別する。
- 表示内容:
  - canonical roots = `.mds/source` `.mds/test`
  - source overview 必須
  - section semantic は project profile で管理
  - link policy を今回選ぶ
  - quality は semantic slot で自動検出される
- 入力: 次へ進む、戻る不可、キャンセル可

### 2. Section Profile Preset

- 目的: project-level section semantic profile の基本方針を選ぶ。
- 選択肢:
  - `English`
  - `Japanese`
  - `Custom`
- 備考: 表示 title を決める画面であり、semantic 自体を選ばせる画面ではない。

### 3. Custom Section Labels

- 条件: `Custom` を選んだ場合のみ表示する。
- 目的: 必須 semantic の表示名だけを編集する。
- 対象 semantic:
  - `Purpose`
  - `Contract`
  - `Source`
  - `Covers`
  - `Cases`
  - `Test`
- 備考: v1 では必須 semantic だけを wizard で編集し、その他は advanced config へ委ねる。

### 4. Link Policy

- 目的: package の参照記法 policy を選ぶ。
- 選択肢:
  - `wiki-only`
  - `markdown-only`
  - `mixed`
- 備考: この画面は v1 で必須。

### 5. Quality Summary

- 目的: quality integration を slot semantic で確認しつつ、診断が最終的に `mds file` へ戻る前提を利用者へ伝える。
- 表示内容:
  - `typecheck`
  - `lint`
  - `fix`
  - `test`
- 各 slot について表示するもの:
  - detected command
  - detection source: package manager / existing scripts / config / schema / none
  - override 有無
- 備考: 通常利用ではこの summary を確認してそのまま次へ進める。

### 6. Quality Advanced

- 条件: 利用者が詳細編集を選んだ場合のみ表示する。
- 目的: slot ごとの command override を編集する。
- 編集対象:
  - `typecheck`
  - `lint`
  - `fix`
  - `test`
- 非対象:
  - 個別 tool 選択
  - doctor required / optional 一覧
  - version floor 編集
- 備考: quality integration は slot semantic を中心とし、tool 固有 UI を中核にしない。

### 7. AI Kit Toggle

- 目的: package 初期化の後に AI kit を追加するか確認する。
- 選択肢:
  - 追加する
  - 追加しない
- 備考: AI は package init の主目的ではなく optional step。

### 8. AI Targets

- 条件: AI kit を追加する場合のみ表示する。
- 目的: 対象 AI CLI を複数選択する。

### 9. AI Categories

- 条件: AI target を 1 つ以上選んだ場合のみ表示する。
- 目的: target ごとに instructions / skills / commands の生成カテゴリを選ぶ。

### 10. Confirm

- 目的: 実際に生成される差分を要約して最終確認する。
- 表示内容:
  - 生成 / 更新する file 一覧
  - section profile summary
  - link policy
  - quality slot summary
  - AI kit の有無と対象
- 備考: 内部設定を全部表示するのではなく、利用者が判断に必要な差分要約を優先する。

## 状態遷移 / 不変条件

- Welcome では固定 contract と可変 policy を区別して見せる。
- section semantic は project profile として一貫管理する。
- quality setup は slot semantic を中心に扱い、通常フローでは個別 tool 入力を要求しない。
- AI 関連画面は optional branch として package 初期化本線から分岐する。

## エラー / 例外

- 認識済み package manager metadata がない場合、wizard 開始前または Welcome で中断できる。
- quality slot が自動検出できない場合、summary 上で unresolved と表示し advanced 編集へ誘導できる。
- Custom label が必須 semantic を満たさない場合、次へ進む前に validation error を出せる。
- キャンセル時は file を書き込まない。

## 横断ルール

- wizard は固定 contract を毎回設定させるのではなく、project 差分を生む policy だけを聞く。
- 画面は人間に分かりやすく、AI が読み取っても再現しやすい順序と責務で構成する。
- 将来 v2 で資料種別や policy が増えても、Welcome / policy / optional branch / confirm の骨格を維持できること。

## 検証観点

- 利用者が Welcome から fixed contract を理解できる。
- Section Profile と Link Policy の設定結果が config と template に反映される。
- Quality Summary が slot semantic 中心に表示される。
- remap 前提の quality flow を wizard 上で説明できる。
- unresolved quality slot が advanced edit へ正しく誘導される。
- Confirm が差分要約として機能する。

## 関連資料

- `./SPEC-cli-init-and-new-workflows.md`
- `../mds-core/SPEC-core-config-and-authoring-policy.md`
- `../mds-core/SPEC-core-quality-and-fix-pipeline.md`
- `../../requirements/v1/REQ-ux-guided-editor-authoring.md`
- `../../requirements/v1/REQ-ux-section-title-independence.md`
- `../../requirements/v1/REQ-quality-diagnostic-remap-to-mds.md`
