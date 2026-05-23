---
id: REQ-ux-section-title-independence
status: 採用
related:
  - ../../architecture.md
  - ../../validation.md
  - ../../specs/shared/SPEC-authoring-markdown-format.md
  - ../../specs/mds-core/SPEC-core-config-and-authoring-policy.md
---

# Section Title Independence

## リリース位置づけ

v1 要件。初回提供で満たすべき authoring 表示名独立性要件。

## 目標

`mds file` の意味解釈が特定の section title 文字列に固定されず、プロジェクト文化や言語に応じた表示名を使えること。

## 根拠

- ユーザーは project ルールや文化圏に応じて section title が変わることを重視している。
- 固定の見出し文字列だけに依存すると、人間の読みやすさと導入性が落ちる。
- 一方で、`mds` は機械検証と一貫した意味解釈を維持する必要がある。

## 対象範囲

- 一般 `mds file` の section semantic
- label preset / label override
- diagnostics、completion、template、validation における section 認識

## 対象外

- 各 file ごとに無制限な自由命名を許して意味解釈を曖昧にすること
- semantic を持たない飾り見出しの標準化
- 機械解釈不能なローカル慣習を v1 の中核仕様にすること
- dedicated special file が専用 spec で固定 visible heading / anchor を持つ契約

## 成功指標

- project ごとに section 表示名を選べる。
- 表示名が変わっても `mds` の validation、template、navigation が同じ semantic として動作する。
- 利用者は固定英語見出しを前提にしなくても `mds file` を運用できる。

## 制約 / 品質条件

- semantic は canonical な内部概念に正規化されること。
- 表示名の自由度は機械検証可能性を壊さない範囲に限定すること。
- 人間可読性と AI 可読性の両方を保つこと。
- fixed visible heading を必要とする例外は、special file 専用 spec と validation に明示すること。

## 関連資料

- `../../architecture.md`
- `../../validation.md`
- `../../specs/shared/SPEC-authoring-markdown-format.md`
- `../../specs/mds-core/SPEC-core-config-and-authoring-policy.md`
