# Specs

## 役割

このディレクトリは、要求を具体的な振る舞いへ落とし込む正本です。

## 置いてよいもの

- 入出力契約
- 状態遷移
- エラー条件
- 横断ルール
- 検証観点

## 置いてはいけないもの

- 要求の背景説明の大半
- 一時的な設計メモ
- ハーネス運用ルール

## 命名規則

- 共有仕様: `shared/SPEC-<category>-<short-title>.md`
- subproject 固有仕様: `<subproject>/SPEC-<category>-<short-title>.md`

## 参照ルール

- 2 つ以上の subproject にまたがる仕様は `shared/` に置く
- 1 つの subproject に閉じる仕様はその subproject 配下に置く

## 参照

- `shared/index.md`: 複数 subproject にまたがる共有仕様の入口
- `mds-core/index.md`: `mds-core` 固有仕様の入口
- `mds-cli/index.md`: `mds-cli` 固有仕様の入口
- `mds-lsp/index.md`: `mds-lsp` 固有仕様の入口
- `vscode-extension/index.md`: VS Code 拡張固有仕様の入口
- `examples/index.md`: examples 固有仕様の入口
