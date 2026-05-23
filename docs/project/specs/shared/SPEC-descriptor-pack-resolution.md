---
id: SPEC-descriptor-pack-resolution
status: 承認済み
related:
  - ../../requirements/v1/REQ-quality-language-and-toolchain-independence.md
  - ../../requirements/v1/REQ-ux-human-ai-authoring-experience.md
  - ../../architecture.md
  - ../../validation.md
---

# Descriptor Pack Resolution

## 概要

`mds` 本体が言語・tool・package manager 固有 descriptor を builtin として所有せず、package / workspace / external pack から descriptor を解決する契約を定義する。

## 関連要求

- `REQ-quality-language-and-toolchain-independence`
- `REQ-ux-human-ai-authoring-experience`

## 入力

- package-local `.mds/descriptors/**`
- workspace shared descriptor directory
- descriptor source config
- descriptor source lock
- user/global descriptor store
- language / tool / package-manager descriptor TOML

## 出力

- resolved descriptor registry
- descriptor origin metadata
- lock-consistent descriptor source set
- descriptor diagnostics

## 挙動

- v1 clean runtime は mds 本体に language / tool / package-manager 固有 descriptor content を持たない。
- descriptor は local file と external descriptor source から解決する。
- descriptor source は local path、workspace path、git source などを表現できる。ただし v1 の最小実装は local path と workspace path を優先してよい。
- repo は `.mds/descriptor-sources.toml` で descriptor source を宣言できる。
- repo は `.mds/descriptor-sources.lock` で git rev / content hash / resolved path など、CI で再現可能な pin を保持できる。
- user/global descriptor store は開発者環境の利便性のために使えるが、CI 再現性が必要な package では repo-level source または lock を優先する。
- 解決順は package-local descriptor、workspace shared descriptor、repo descriptor source、user/global descriptor store、unknown fallback とする。
- package-local descriptor は同一 id の upstream descriptor を置換または override できる。部分 merge を提供する場合、merge rule は field 単位で決定的でなければならない。
- descriptor registry は各 descriptor の origin を保持し、doctor / explain / diagnostics で表示できる。
- descriptor source の取得・更新・lock は mds の責務だが、descriptor の言語固有内容は mds 本体の責務ではない。
- `mds init` の clean bootstrap seed は、package-local descriptor を生成するための authoring 初期値に限定する。resolver は seed を暗黙 fallback として読まず、生成後の `.mds/descriptors/**` または明示 descriptor source だけを registry 入力にする。

## 状態遷移 / 不変条件

- 同一 package root と同一 lock から得られる descriptor registry は決定的である。
- package-local descriptor は常に external source より優先される。
- mds 本体同梱の言語固有 descriptor が無くても、package が必要 descriptor source を持つ場合は representative flow を実行できる。
- descriptor source の更新は lock 更新と分離できる。
- descriptor origin は validation と UX の説明可能性のために失われない。

## エラー / 例外

- source config が読めない場合は descriptor source diagnostic を出す。
- lock と source config が矛盾する場合は warning または error とする。CI mode では error にできる。
- descriptor id / alias / suffix が複数 origin 間で衝突し、優先順位で決定できない場合は error とする。
- git source 取得が必要だが network が使えない場合は、lock 済み cache があればそれを使い、なければ error とする。
- unknown fallback は editor の degraded UX には使えるが、build / quality の必須 descriptor 不足を隠してはならない。

## 横断ルール

- mds 本体は descriptor schema、解決順、lock、diagnostics、explain UX を所有し、言語固有 descriptor content を所有しない。
- descriptor pack は template 集ではなく、再利用可能な descriptor source として扱う。
- `mds init` は大量 template を package にコピーしない。必要な場合だけ package-local descriptor seed、override、または eject を生成する。
- external descriptor source は v1 で必須の配布経路にできるが、mds 本体 repository に言語追加実装を要求しない。

## 検証観点

- package-local descriptor が external source より優先される。
- descriptor origin が doctor / explain で確認できる。
- lock 済み source で CI が再現可能に registry を組み立てられる。
- mds 本体同梱の言語固有 descriptor corpus なしで representative package が動く。
- descriptor parse / read failure が原因 file と origin を含む diagnostic になる。

## 関連資料

- `SPEC-language-extension-contract.md`
- `../../requirements/v1/REQ-quality-language-and-toolchain-independence.md`
- `../../requirements/v1/REQ-ux-human-ai-authoring-experience.md`
- `../../architecture.md`
- `../../validation.md`
