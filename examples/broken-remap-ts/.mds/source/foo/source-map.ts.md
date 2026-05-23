# Source map

## Purpose

Fixture.

## Contract

- Preserve source map spans.

## Source

```ts
export const one = 1;
```

```ts
export function two(): number {
  return one + 1;
}
```

```ts
export const twoResult = two();
export const confirmed = twoResult === 2;
export type TwoResult = typeof twoResult;
```