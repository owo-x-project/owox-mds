# Task 001: Preserve Diagnostic Remap With Source Map

## 目的

config/schema runtime へ移行しても、tool diagnostics の Markdown 正本 remap を source map 前提で維持する。

## 前提条件

- quality slot と capture rule が config/schema runtime で解決できる

## 作業内容

- remap 条件と fallback 条件を明確化する
- path / line / column を持つ tool output の remap を representative case で確認する
- remap 不能時に誤った Markdown 位置を返さない contract を test で固定する

## 完了条件

- CLI / LSP / editor が同じ Markdown 正本位置を見られる
- remap 不能 case が誤誘導を起こさない

## 検証方法

- representative tool 出力で remap 成功 / 失敗 / no-op を確認する
- core diagnostics / quality test を実行する

## 依存関係

- `../phase-05-core-quality-and-doctor-runtime/task-001-replace-quality-slot-and-capture-resolution.md`

## 成果物

- `mds/core/src/quality.rs`
- `mds/core/src/diagnostics.rs`
- `mds/core/tests/`
- `docs/project/validation.md`