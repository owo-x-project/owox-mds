# foo.source-map test

## Purpose

[[foo.source-map]] の generated output を検証する。

## Covers

- [[foo.source-map]]

## Cases

- `two()` は `2` を返す。

## Test

```ts
import { describe, expect, it } from "vitest";
import { two } from "../../src/foo/source-map";

describe("source-map", () => {
  it("returns incremented value", () => {
    expect(two()).toBe(2);
  });
});
```