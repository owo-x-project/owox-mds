# Task 005: Add Descriptor Authoring CLI

## 目的

`mds init descriptor` と `mds descriptor` により、利用者が言語固有 content の本体同梱や大量 template なしで descriptor を作成、検証、説明、schema 出力できる入口を追加する。

## 前提条件

- core descriptor source / pack resolution の境界が決まっている
- CLI init / new の wizard 入口が v1 方針へそろっている

## 作業内容

- `mds init descriptor language|tool|package-manager` の wizard / non-interactive apply 境界を実装する
- `mds descriptor check` で TOML parse、required field、alias/suffix 衝突、special file 衝突、tool diagnostic capture rule を検証する
- `mds descriptor explain` で target / id / command がどの origin の descriptor へ解決されたかを表示する
- `mds descriptor schema` で descriptor kind ごとの JSON Schema を出力する
- `mds descriptor sources` で repo / user / global source と lock 状態を表示する
- `mds init` の descriptor 不足時メッセージを、builtin 追加ではなく descriptor wizard または source 追加へ誘導する
- CLI help / docs の代表 command は `SPEC-cli-descriptor-authoring-workflows.md` と `GUIDE-descriptor-authoring.md` にそろえ、具体出力文言は実装完了後に再照合する

## 完了条件

- 未知言語名でも wizard 質問から最小 language descriptor を生成できる
- generated descriptor が `mds descriptor check` を通る
- malformed descriptor と衝突が原因 file 付きで説明される
- `descriptor explain` が package-local / workspace / source pack / global の origin を表示する
- CLI help / usage が spec と一致する

## 検証方法

- CLI integration test で `init descriptor`、`descriptor check`、`descriptor explain`、`descriptor schema` を確認する
- malformed descriptor / collision fixture を追加する
- `rtk cargo test` を実行する

## 依存関係

- `../phase-03-core-schema-loader-boundary/task-002-add-descriptor-source-and-pack-resolution.md`
- `task-001-align-init-wizard-screen-flow.md`
- `task-002-align-new-command-and-templates.md`
- `../../../specs/mds-cli/SPEC-cli-descriptor-authoring-workflows.md`
- `../../../specs/shared/SPEC-descriptor-pack-resolution.md`
- `../../../specs/shared/GUIDE-descriptor-authoring.md`

## 成果物

- `mds/cli` command surface
- `mds/core` descriptor validation / explain APIs
- CLI integration tests
- descriptor schema output
