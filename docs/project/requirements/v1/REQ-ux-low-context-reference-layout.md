---
id: REQ-ux-low-context-reference-layout
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../glossary/core.md
  - ../../specs/mds-core/index.md
  - ../../specs/mds-lsp/index.md
---

# Low-Context Reference Layout

## リリース位置づけ

v1 要件。初回提供で満たすべき mds file 記述要件。

## 目標

`mds file` が AI と人間の両方にとって、可能な限り少ないコンテキスト消費で必要情報へ到達できる構造を持つこと。

## 根拠

- `mds file` は人間だけでなく AI も読む前提である。
- ユーザーはコンテキスト / トークン消費の少なさと、必要情報を簡単に見つけられることを重視している。
- project は source of truth として継続的に読む文書を扱うため、探索しやすさが保守コストへ直結する。

## 対象範囲

- `mds file` 内の見出し構成
- 近接参照、相互参照、overview、関連資料の置き方
- 人間と AI が必要情報へ到達するまでの探索コスト

## 対象外

- すべての情報を 1 ファイルへ平坦化すること
- viewer 依存機能や独自 UI に頼る情報探索
- トークン削減だけを優先して可読性を損なう構成

## 成功指標

- 利用者が必要な情報の所在をファイル構造と参照だけで追える。
- AI が全文走査に頼らず、近い参照から必要情報を辿れる。
- 関連情報が適切に分割され、重複より参照で結ばれている。
- overview や関連資料が探索起点として機能する。

## 制約 / 品質条件

- 参照は不足でも過剰でもなく、探索コスト削減に効く場所へ置くこと。
- ファイル分割は token 節約と人間可読性の両立を優先すること。
- `<!-- -->` のように視認しにくく token だけ増やす補助マーカーを、情報配置や探索導線の手段として使わないこと。
- 情報配置は機械的に追跡しやすく、相対参照で辿れること。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../glossary/core.md`
- `../../specs/mds-core/index.md`
- `../../specs/mds-lsp/index.md`
