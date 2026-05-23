# Bridge UX Scenario Checklist

前提 fixture は `fixtures.md` を使う。

## Status

- Active Context: manual-only / non-blocking / post-v1 evidence
- Embedded Language Delegation: manual-only / non-blocking / post-v1 evidence
- Diagnostic Mirror: manual-only / non-blocking / post-v1 evidence
- phase 12 status: 未実施 / accepted manual-only risk
- v1 blocking 判定: `npm --prefix editors/vscode run compile` と `npm --prefix editors/vscode run test:diagnostics-regression` を優先し、この checklist は補助証跡として扱う
- active language registry 判定: VS Code 拡張は `mds.resolvedLanguages` を primary source とし、LSP unavailable 時だけ package-local descriptor discovery へ degrade する

## Active Context

- [ ] `examples/minimal-ts/.mds/source/greet.ts.md` の TypeScript code fence 内で status bar が `mds typescript | source` を示す。
- [ ] Markdown preview command が `mds-markdown` editor で利用できる。

## Embedded Language Delegation

- [ ] hover が TypeScript symbol 情報を返す。
- [ ] definition が Markdown 正本または参照先へ移動する。
- [ ] references が bridge 経由の結果だけを返し、Markdown heuristic duplicate を混在させない。
- [ ] rename が Markdown code fence へ remap された edit を適用する。
- [ ] formatting が code fence 範囲へ remap された edit を適用する。
- [ ] edit-backed code action が command-only action を漏らさず、edit だけ Markdown 側へ反映する。

## Diagnostic Mirror

- [ ] generated file 側 diagnostic が Markdown 正本へ戻る。
- [ ] embedded shadow diagnostic が Markdown 正本へ戻る。
- [ ] code block 移動後、old shadow URI 由来の mirrored diagnostic が消え、新しい位置だけ残る。
- [ ] remap 不能 diagnostic は Markdown 上へ guessed 表示しない。

## Evidence To Record

- [ ] 実行 commit または diff base
- [ ] VS Code version
- [ ] 実行した command
- [ ] 各 scenario の pass / fail と観測メモ
