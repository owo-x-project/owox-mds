# Patterns

## 役割

このディレクトリは、複数の仕様や実装で再利用する設計・実装パターンを管理する正本です。

## 置いてよいもの

- 共通パターン
- 適用条件
- 例外条件
- 関連する正本への参照

## 置いてはいけないもの

- 単発のローカル事情
- 詳細な作業手順
- ハーネス運用ルール

## 命名規則

- `impl-<slug>.md`
- `data-<slug>.md`
- `ui-<slug>.md`
- `ux-<slug>.md`
- `api-<slug>.md`
- `test-<slug>.md`
- `ops-<slug>.md`

## 参照ルール

- requirement や spec の代替にはしない
- 追加時は spec / implementation / validation への影響を確認する

## 参照

- 現在は個票なし: 追加時はカテゴリ接頭辞付きの pattern 個票を置く