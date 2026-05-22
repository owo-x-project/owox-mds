# Bar

## Purpose

Fixture.

## Contract

- Preserve fixture behavior.

## Source

```ts
export type Bar = Util;
```

```ts
export const bar: Bar = util;
```

```ts
expect(bar).toBe("ok");
```