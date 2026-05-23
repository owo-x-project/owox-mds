# Source Overview

## Purpose

`examples/broken-remap-ts` を diagnostic remap regression fixture として保つ。

## Architecture

`.mds/source/` と `.mds/test/` の Markdown を正本とし、`src/` と `tests/` に TypeScript を生成する。`foo/bar.ts.md` は core の remap success / failure / no-op regression、`foo/source-map.ts.md` は LSP / VS Code の generated と embedded remap regression に使う。

### Package Summary

| Name | Version |
| --- | --- |
| broken-remap-ts | 0.1.0 |

### Dependencies

| Name | Version | Summary |
| --- | --- | --- |

### Dev Dependencies

| Name | Version | Summary |
| --- | --- | --- |
| prettier | 3.3.3 |  |
| typescript | 5.3.3 |  |
| vitest | 2.1.8 |  |

## Rules

- Keep remap success / failure / no-op regression separate from `examples/minimal-ts`.
- Keep generated `src/` and `tests/` output synchronized with the Markdown source of truth.
