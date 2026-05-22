# Proposals

## 役割

このディレクトリは、正式仕様の前段階にある設計草案や提案を置くための場所です。

## 置いてよいもの

- 設計草案
- API 草案
- データ構造案
- 構成変更案
- 移行案

## 置いてはいけないもの

- 採用済み仕様の正本
- 過去ログだけのメモ
- ハーネス運用ルール

## 命名規則

- `active/proposal-<topic>.md`
- `archive/proposal-<topic>.md`

## 昇格ルール

- 採用した proposal は requirement / spec / ADR への昇格を確認する
- 参照優先度を落とした proposal は `archive/` へ移す

## 参照

- `active/`: 採否判断前の proposal の配置先
- `active/proposal-capability-schema-runtime.md`: built-in descriptor / tool registry 依存を config/schema runtime へ移す提案
- `archive/`: 参照優先度を落とした proposal の配置先。現在は個票なし
