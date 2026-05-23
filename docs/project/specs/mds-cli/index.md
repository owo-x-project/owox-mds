# mds-cli Specs

## 役割

このディレクトリは `mds-cli` 固有の詳細仕様を置く。

## 対象

- command surface
- 引数解釈
- interactive init
- self-update
- CLI 出力整形

## 命名規則

- `SPEC-cli-<short-title>.md`

## 参照

- `SPEC-cli-command-surface-and-execution.md`: command surface、option 制約、stdout/stderr/exit code の仕様
- `SPEC-cli-descriptor-authoring-workflows.md`: `mds init descriptor` と `mds descriptor` による descriptor 作成、検証、説明、schema 出力の仕様
- `SPEC-cli-init-and-new-workflows.md`: wizard、non-interactive init、template 起票の仕様
- `SPEC-cli-init-wizard-screen-flow.md`: init wizard の画面順、分岐、各画面責務の仕様
- `SPEC-cli-doctor-and-update.md`: doctor と GitHub 前提 update の仕様
