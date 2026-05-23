---
id: REQ-quality-safe-package-bounded-generation
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../specs/mds-core/index.md
---

# Safe Package-Bounded Generation

## リリース位置づけ

v1 要件。初回提供で満たすべき安全性要件。

## 目標

生成と検証の挙動が保守的で、package 境界と既存ファイルの安全性を壊さないこと。

## 根拠

- source of truth 運用は、派生物の破壊や意図しない書き込みが起きると採用しにくい。
- ユーザーは堅牢さ、保守コストの低さ、運用上の安心感を重視している。
- 既存実装は未管理ファイル保護、package root 外出力拒否、package metadata 同期確認を持つ。

## 対象範囲

- output path の制約
- generated file 上書き条件
- package metadata snapshot の同期確認
- package 単位での lint / doctor / package sync 運用

## 対象外

- package 外の任意ディレクトリを生成先にする運用
- 既存未管理ファイルの強制上書き
- 破壊的変更を既定動作にする CLI 仕様

## 成功指標

- package 外への出力要求は拒否される。
- `mds` 管理下でない既存ファイルは自動上書きされない。
- package metadata snapshot 不整合を検出できる。
- 利用者が安全性を損なわず dry-run と write を使い分けられる。

## 制約 / 品質条件

- 既定挙動は保守的であること。
- エラー時は silent failure ではなく、利用者が対処可能な診断を返すこと。
- 安全性は言語別実装差分より優先されること。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../specs/mds-core/index.md`
