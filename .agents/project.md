# Project

## Name
owox-mds

## Description
Markdown を実装正本にする `mds` の monorepo。Rust workspace を中心に、コード生成 core、CLI、LSP、VS Code 拡張、動作確認用 examples を含む。

## Language
Rust, TypeScript, Markdown, TOML

## Kind
monorepo

## Subprojects
- `mds-core`: Markdown 実装文書の解析、descriptor、config、package 発見、生成計画、quality 実行を担う core library
- `mds-cli`: `mds` コマンド、対話式 init、self-update を提供する CLI
- `mds-lsp`: Markdown 実装文書向け診断、補完、ナビゲーション、source map 連携を提供する LSP
- `vscode-extension`: VS Code 上で `mds` authoring と `mds-lsp` 接続を提供する editor extension
- `examples`: TypeScript / Python / Rust の最小サンプル群。仕様確認と開発者体験レビュー用

## Teams
- `platform`: `mds` 本体、editor tooling、examples、関連 docs の保守を担うチーム

## Integrations
- `github-distribution`: GitHub Releases と raw `install.sh` を使う配布・自己更新経路
