# Source Overview

## Purpose

`examples/minimal-ts` を v1 canonical success-path fixture として保ちます。

## Architecture

`.mds/source/` と `.mds/test/` の Markdown を正本とし、`src/` と `tests/` に TypeScript を生成します。package metadata は `mds package sync` で下記 managed section に同期します。

### Package Summary

| Name | Version |
| --- | --- |
| minimal-ts | 0.1.0 |

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

- Keep one implementation md per feature.
- Keep generated `src/` and `tests/` output synchronized with the Markdown source of truth.
