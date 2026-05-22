---
id: REQ-quality-language-and-toolchain-independence
status: 採用
related:
  - ../../architecture.md
  - ../../tech-stack.md
  - ../../specs/mds-core/index.md
  - ../../specs/examples/index.md
---

# Language And Toolchain Independence

## リリース位置づけ

v1 要件。初回提供で満たすべき拡張性と保守性の要件。

## 目標

特定の言語や単一ツールチェーンに縛られず、複数言語へ同じ authoring 体験を提供できること。

## 根拠

- ユーザーは市場の言語やツールチェーン変化に左右されにくい設計を重視している。
- v1 fixture は TypeScript package を使うが、language contract 自体は package-local schema と metadata により言語非依存に拡張できる必要がある。
- project は CLI、core、LSP、editor integration を複数言語に広げられる前提で構成されている。

## 対象範囲

- 複数言語 package への対応
- 言語別 quality command の切り替え
- editor / LSP 側の language discovery
- config / schema contract と focused validation による言語横断確認

## 対象外

- すべての言語や package manager の即時対応
- 各言語固有ツールの最適化競争
- 1 言語専用に全体 UX を固定する判断

## 成功指標

- 少なくとも複数言語で同じ source of truth 運用モデルを維持できる。
- 言語追加時、既存 package 運用や CLI 体系を壊さず拡張できる。
- v1 examples が package-local schema / metadata 前提の representative flow を保ち、language 拡張を妨げない。
- 利用者が言語ごとに別思想の authoring 体験を強制されない。

## 制約 / 品質条件

- 共通 UX を優先し、言語差分は必要最小限に抑えること。
- 特定ツールチェーンの流行に依存しすぎないこと。
- 保守コスト増大を避けるため、言語追加は既存モデルに整合する範囲で行うこと。

## 関連資料

- `../../architecture.md`
- `../../tech-stack.md`
- `../../specs/mds-core/index.md`
- `../../specs/examples/index.md`
