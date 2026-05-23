# 技術スタック

## 目的

このファイルは、採用済み技術を役割単位で記録します。

## 読むべき場面

- 採用技術やバージョン方針を確認したいとき
- 新しい採用判断を記録したいとき

## 採用技術

- `Rust`: 中核実装言語。workspace edition は `2021`
- `Tokio`: `mds-lsp` の非同期実行基盤。version `1`
- `tower-lsp`: LSP 実装。version `0.20`
- `Serde`: config / manifest / protocol data の serialize / deserialize。version `1`
- `toml`: `mds.config.toml` と package metadata 処理。version `0.8`
- `serde_yaml`: YAML metadata 処理。version `0.9`
- `serde_json`: JSON metadata と protocol payload 処理。version `1`
- `regex`: config / diagnostics / parsing 補助。version `1`
- `ratatui`: CLI wizard UI。version `0.26`
- `crossterm`: CLI terminal 制御。version `0.27`
- `TypeScript`: VS Code extension 実装言語。version `5.3.3`
- `vscode-languageclient`: VS Code extension から LSP 接続する client。version `9.0.1`
- `VS Code Extension API`: editor integration 基盤。engine `^1.85.0`
- `Node.js / npm`: VS Code extension build と package 管理
- `Markdown`: 実装正本フォーマット
- `TOML`: project config と descriptor 系設定フォーマット

## 補足

- 依存 version は lockfile と各 package manifest を正本とする。
- 言語・ツールチェーン依存の差分は code 本体に散らさず descriptor と adapter に寄せる。
