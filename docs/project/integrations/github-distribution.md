# GitHub Distribution

## 目的

`mds` の release 配布と自己更新経路を提供する。

## 利用箇所

- `mds-cli` の `update` command
- ルート `install.sh`
- GitHub Releases 上の配布 archive

## 責任チーム

- `platform`

## 制約

- GitHub API 依存。latest version 取得時は API 到達性と rate limit の影響を受ける
- install script は raw GitHub content と release archive URL に依存する
- 認証は通常不要だが、公開配布前提が崩れると成立しない
- Release tag は配布 archive 名と install / update の version 解決に使う
- Release build では tag 由来 version を `MDS_RELEASE_VERSION` として binary 表示へ注入し、`mds --version` と GitHub Release tag のずれを CI で検出する
- Cargo / VS Code Marketplace など package manager 固有の version 制約がある場合は、Release tag から各 package 用 version へ正規化して使う

## 障害時の扱い

- update / install は失敗し得る
- 既存のローカル build、source 読み取り、docs 更新は継続できる
- 障害時はローカル build や手動 install を代替経路とする
- package manager 用 version 正規化に失敗した場合は release artifact 生成を中止する

## 参照

- `../architecture.md`
- `../validation.md`
