# app.greet test

## Purpose

[[greet]] の挙動を検証する。

## Covers

- [[greet]]

## Cases

- 通常名では `"Hello, World!"` を返す。
- `name` が未指定のときは `"Hello, Anonymous!"` を返す。

## Test

```ts
import { describe, expect, it } from "vitest";
import { greet } from "../src/greet";

describe("greet", () => {
  it("returns greeting with name", () => {
    expect(greet({ name: "World" })).toBe("Hello, World!");
  });

  it("uses Anonymous when name is missing", () => {
    expect(greet({})).toBe("Hello, Anonymous!");
  });
});
```
