# app.greet

## Purpose

名前を受け取り、挨拶メッセージを返す。

## Contract

- 入力は `GreetOptions`。
- 出力は `string`。
- 例外は投げない。
- `name` が未指定または空白のみの場合は `Anonymous` を使う。

## API

`greet` は外部公開 API で、`GreetOptions` は挨拶生成に必要な入力だけを持つ。

## Source

##### GreetOptions

`GreetOptions` は挨拶生成に必要な入力です。

```ts
export interface GreetOptions {
  name?: string;
}
```

##### greet

`greet` は入力を正規化して挨拶文字列を返します。

```ts
export function greet(options: GreetOptions): string {
  const name = options.name?.trim() || "Anonymous";
  return `Hello, ${name}!`;
}
```

## Cases

- `name: "World"` を渡すと `"Hello, World!"` を返す。
- `name` が未指定または空白のみのときは `"Hello, Anonymous!"` を返す。
