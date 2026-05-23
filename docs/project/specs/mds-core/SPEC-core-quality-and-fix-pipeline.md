---
id: SPEC-core-quality-and-fix-pipeline
status: 承認済み
related:
  - ../shared/SPEC-authoring-markdown-format.md
  - ../shared/SPEC-generation-safety-and-derivation.md
  - ../../requirements/v1/REQ-quality-diagnostic-remap-to-mds.md
  - ../../requirements/v1/REQ-quality-portable-readable-verifiable-markdown.md
  - ../../requirements/v1/REQ-ux-low-context-reference-layout.md
subproject: mds-core
---

# mds-core Quality And Fix Pipeline

## 概要

`mds-core` が行う lint、typecheck、test、fix の実行モデルと、`mds lint --fix` による文書正規化契約を定義する。

## 関連要求

- `REQ-quality-diagnostic-remap-to-mds`
- `REQ-quality-portable-readable-verifiable-markdown`
- `REQ-ux-low-context-reference-layout`

## 入力

- package config
- optional capability schema files in `.mds/descriptors/languages/*.toml`
- optional package-local tool manifests in `.mds/descriptors/tools/*.toml` and `.mds/descriptors/linters/*.toml`
- impl md / test md
- link policy mode
- quality tool command inputs
- diagnostic capture rule
- fix mode options

## 出力

- quality diagnostics
- fixed Markdown docs
- remapped toolchain diagnostics

## 挙動

- `mds-core` は package config に従い、typecheck、lint、test を doc 単位で実行できる。
- phase 05 の quality slot command runtime precedence は、package config の explicit command、package config の explicit false、active package manager script、active package manager command、capability schema の `quality_defaults`、no external command の順で解決する。
- package config の explicit false は slot を disable し、後続 fallback を止める。unset は fallback 継続と区別される。
- package manager script / command と capability schema `quality_defaults` は、active package manager の resolved language と対象 doc language が一致する場合だけ runtime fallback 候補になる。
- external toolchain diagnostics は可能な限り Markdown 正本位置へ remap する。
- remap は source map と capture 可能な path / line / column 情報に基づいて行う。
- diagnostic capture rule は capability schema の `[[tooling.<slot>.diagnostics]]` に定義する。phase 01 では `pattern` `path_group` `line_group` `column_group` `message_group` `severity` `line_offset` の公開 field と ownership だけを固定し、runtime での capture resolution は phase 05 で正式化する。
- phase 05 の capture resolution は capability schema の `[tooling.<slot>]` behavior を primary とし、対象 package root 配下 `.mds/descriptors/tools/*.toml` と `.mds/descriptors/linters/*.toml` は slot behavior が input / output / append_file_arg / diagnostics のいずれにも explicit 契約を持たない場合だけ command-match fallback として使う。global built-in tool manifest は使わない。
- package config は command override を担うが、diagnostic capture group 定義は担わない。
- `mds lint --fix` は code block 内の整形だけでなく、文書正規化も責務に含む。
- 文書正規化には少なくとも link policy への正規化を含む。
- v1 では wiki-link と Markdown link の相互変換を `lint --fix` の対象に含める。
- 将来の文書正規化は、section 構造や軽微な format repair を含められるが、意味を書き換えない範囲に限る。

## 状態遷移 / 不変条件

- fix 後の文書は package の link policy と validation rule に適合すること。
- fix は code block 外の prose の意味を変えないこと。
- remapped diagnostics は誤った Markdown 位置を示さないこと。
- `--check` は変更必要箇所を検出するが、文書を書き換えないこと。

## エラー / 例外

- toolchain 不在は environment error とする。
- remap 不能 diagnostics は generic tool failure として報告できる。
- 自動修正不能な policy 違反は lint error のまま残せる。
- fix 適用結果が unsafe な場合は変更を拒否できる。

## 横断ルール

- fix は人間可読性と機械検証可能性を改善する方向だけで使う。
- external formatter 依存の code block fix と、`mds` 独自の文書正規化を同じ pipeline で扱えること。
- v2 で資料種別が広がっても、fix は policy-based normalization として拡張できること。

## 検証観点

- typecheck / lint / test diagnostics を Markdown 正本へ戻せる。
- `lint --fix` が link policy を正規化できる。
- `lint --fix --check` が変更必要箇所を検出できる。
- code block fix と文書正規化が衝突しない。

## 関連資料

- `../shared/SPEC-authoring-markdown-format.md`
- `../shared/SPEC-generation-safety-and-derivation.md`
- `../../requirements/v1/REQ-quality-diagnostic-remap-to-mds.md`
- `../../requirements/v1/REQ-quality-portable-readable-verifiable-markdown.md`
- `../../requirements/v1/REQ-ux-low-context-reference-layout.md`
