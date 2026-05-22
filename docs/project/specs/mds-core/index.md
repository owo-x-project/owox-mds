# mds-core Specs

## 役割

このディレクトリは `mds-core` 固有の詳細仕様を置く。

## 対象

- config 読み込み
- config / schema 解釈
- package discovery
- Markdown 解析
- generation / source map
- quality / doctor / init

## 命名規則

- `SPEC-core-<short-title>.md`

## 参照

- `SPEC-core-config-and-authoring-policy.md`: config、doc kind、label、link policy mode の仕様
- `SPEC-core-overview-and-package-sync.md`: source overview special file と package sync の仕様
- `SPEC-core-quality-and-fix-pipeline.md`: quality 実行、diagnostic remap、`lint --fix` 文書正規化の仕様
