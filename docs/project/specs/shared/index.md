# Shared Specs

## 役割

このディレクトリは、複数の subproject にまたがる仕様を置くための場所です。

## 置いてよいもの

- 共通 API 契約
- 共通状態ルール
- 共通認可や横断要件に関する詳細仕様

## 置いてはいけないもの

- 1 つの subproject に閉じる仕様
- 設計草案や調査メモ

## 命名規則

- `SPEC-<category>-<short-title>.md`

## 参照ルール

- subproject 固有仕様が必要な場合は、対象の `<subproject>/index.md` と個票を追加する

## 参照

- `SPEC-model-package-layout.md`: package、authoring root、doc kind、overview special file の共有仕様
- `SPEC-authoring-markdown-format.md`: 一般 `mds file` の section、link policy、可読性、隠し HTML comment marker 不使用を含む共有仕様
- `SPEC-descriptor-pack-resolution.md`: mds 本体に言語固有 descriptor content を持たず external / workspace / package-local descriptor source から解決する共有仕様
- `GUIDE-descriptor-authoring.md`: descriptor 作成、検証、override、source pack 化の利用者向け guide
- `SPEC-generation-safety-and-derivation.md`: generation safety、manifest、source map の共有仕様
- `SPEC-language-extension-contract.md`: config/schema と package manager 連携による多言語拡張の共有仕様
- `SPEC-ux-embedded-language-bridge.md`: embedded code を既存言語機能へ橋渡しする共有仕様
- `SPEC-ux-navigation-and-traceability.md`: definition、references、traceability の共有仕様
