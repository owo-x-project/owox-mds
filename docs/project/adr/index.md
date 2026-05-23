# ADR

## 役割

このディレクトリは、重要な判断とその理由を記録する正本です。

## 置いてよいもの

- 継続的な影響を持つ判断
- 代替案と採否理由
- 後続資料への関連付け

## 置いてはいけないもの

- 単なる作業ログ
- 会議メモだけの記録
- 手順詳細

## 命名規則

- `active/ADR-<NNN>-<short-title>.md`
- `archive/ADR-<NNN>-<short-title>.md`

## 参照ルール

- 現在有効な判断は `active/`
- 参照優先度を落とした過去資料は `archive/`
- 置換関係は状態名ではなく本文または front matter の関連情報で示す

## 参照

- `active/`: 現在有効な ADR の配置先。`ADR-001-remove-built-in-descriptor-corpus.md` で obsolete built-in descriptor corpus removal の判断を管理
- `archive/`: 参照優先度を落とした ADR の配置先。現在は個票なし
