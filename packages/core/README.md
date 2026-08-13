# @jsonlint/core

[![CI](https://github.com/toddynho/jsonlint-core/actions/workflows/ci.yml/badge.svg)](https://github.com/toddynho/jsonlint-core/actions/workflows/ci.yml)

The JSON engine behind [jsonlint.com](https://jsonlint.com). Every error in one pass — with locations, hints, and recovery — at 25-40x the speed of the legacy `jsonlint` package. Zero dependencies. No install scripts.

```js
import { validate, tryParse, parse } from "@jsonlint/core";

const { ok, diagnostics } = validate(text, { mode: "jsonc" });
// diagnostics: [{ code: "W060", message: 'duplicate object key "name"',
//                 severity: "warning", start: 41, end: 47,
//                 related: { start: 12, end: 18 },
//                 hint: "the last occurrence wins; ..." }]
```

## Why

- **Every error, one pass.** Recovery-based parsing reports all problems, not just the first.
- **Errors that teach.** `'True' is not valid JSON — this looks like a Python literal`. Smart quotes, trailing commas, unquoted keys, concatenated documents: named, located, and hinted.
- **Catches what parsers can't say.** Duplicate keys (with the first occurrence's location), integer precision loss (`1111111111258928239 becomes 1111111111258928300`), UTF-16-encoded files, BOMs.
- **Safe by default.** `__proto__` never pollutes; depth limits; duplicate-key policy.
- **JSON.parse compatible.** `parse()` matches native output (reviver included) and throws `SyntaxError`.
- **Fast.** Pure-JS today (validates faster than `JSON.parse` allocates); Rust/WASM core lands behind the same API.

## API

`validate(input, opts)` → `{ ok, diagnostics }` — fastest; powers jsonlint.com
`tryParse(input, opts)` → `{ ok, value, diagnostics }` — never throws; value recovered even on errors
`parse(input, opts)` → value or throws — drop-in for `JSON.parse`
`lineColumn(src, offset)` → `{ line, column }`

Input: `string`, `Uint8Array`, or `ArrayBuffer` (UTF-16/32 bytes detected and reported cleanly).
Options: `mode: "strict" | "jsonc"`, `duplicateKeys: "warn" | "error" | "allow"`, `protoKeys`, `maxDepth`, `reviver`.

## Conformance

JSONTestSuite 95/95 valid accepted, 188/188 invalid rejected; the complete historical
test corpora of zaach/jsonlint and @prantlf/jsonlint; regression tests named for the
GitHub issues they resolve (see the engine repo's ISSUE_AUDIT.md).

## About

By [Todd Garland](https://x.com/toddo), who has run jsonlint.com for many years.
Engineered in collaboration with Claude Fable 5 (Anthropic). Every claim here is
backed by tests running in public CI: [github.com/toddynho/jsonlint-core](https://github.com/toddynho/jsonlint-core)
— including the full audit of 411 issues across six predecessor projects
(ISSUE_AUDIT.md) and the hardening report (HARDENING.md).

MIT
