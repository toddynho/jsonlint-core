# @jsonlint/core

[![CI](https://github.com/toddynho/jsonlint-core/actions/workflows/ci.yml/badge.svg)](https://github.com/toddynho/jsonlint-core/actions/workflows/ci.yml)

The JSON engine behind [jsonlint.com](https://jsonlint.com) — the validator developers have pasted broken JSON into since 2009.

**Every error in one pass. Errors that tell you how to fix them. 25–40x faster than the parser it replaces. Zero dependencies, no install scripts, published with provenance.**

```
npm install @jsonlint/core
```

```js
import { validate } from "@jsonlint/core";

const { ok, diagnostics } = validate(text);
```

```
error[E030]: 'True' is not valid JSON (line 4, col 13)
    "active": True,
              ^
  hint: this looks like a Python literal — use 'true'

warning[W050]: integer exceeds JavaScript's safe range and loses precision
               (1786623058123456789 becomes 1786623058123456768) (line 2, col 14)
  hint: store large IDs as strings, or parse with a lossless-number option

warning[W060]: duplicate object key "name" (line 7, col 3)
  first occurrence at line 3, col 3
```

## Why this exists

`JSON.parse` is fast but tells you almost nothing when input is broken. The legacy `jsonlint` package has better errors but stops at the first one, is 65x slower than native, and hasn't been maintained in a decade. This engine is the successor: it passes the complete historical test suites of both `zaach/jsonlint` and `@prantlf/jsonlint`, plus JSONTestSuite (283/283), plus regression tests for the parser-relevant issues ever filed against six predecessor and adjacent projects — we read all 411 of them, comment threads included ([ISSUE_AUDIT.md](./ISSUE_AUDIT.md), [HARDENING.md](./HARDENING.md)).

## What it catches that others don't

- **Duplicate keys**, with the location of the first occurrence — the most-requested jsonlint feature since 2011
- **Integer precision loss** — the exact corrupted value, before it silently eats your IDs
- **UTF-16-encoded files** — one clear "re-save as UTF-8" instead of a garbage cascade
- **Invalid UTF-8 bytes** — pointed at exactly (pasted Latin-1, the bug class that bit Composer)
- **Silent Windows-path corruption** — `"C:\temp"` is *valid* JSON where `\t` becomes a TAB
- **Lone surrogates, BOMs, smart quotes, Python literals, concatenated documents** — accepted or rejected exactly like `JSON.parse`, but explained

## API

- `validate(input, opts)` → `{ ok, diagnostics }` — allocation-light, fastest
- `tryParse(input, opts)` → `{ ok, value, diagnostics }` — never throws; value recovered even from broken input
- `parse(input, opts)` → drop-in `JSON.parse` (reviver included, throws `SyntaxError`)
- Input: `string` | `Uint8Array` | `ArrayBuffer` · Modes: `strict` (RFC 8259) | `jsonc`
- Options: `duplicateKeys: "warn" | "error" | "allow"`, `protoKeys`, `maxDepth`

Every diagnostic: stable `code`, `message`, `start`/`end` offsets, `hint`, optional `related` location. Full types shipped.

## Performance

Honest numbers (see `bench/`, reproducible): pure-JS engine, 5MB document —

| | time | vs legacy jsonlint |
|---|---|---|
| `JSON.parse` (native, no diagnostics) | 27ms | — |
| **@jsonlint/core validate** | **45ms** | **40x** |
| **@jsonlint/core tryParse** | **106ms** | **17x** |
| jsonc-parser | 121ms | 15x |
| json5 | 768ms | 2.3x |
| jsonlint | 1,801ms | 1x |

We do not claim to beat `JSON.parse` at plain parsing. A Rust core (this repo, `crates/json-core`, ~230MB/s) ships later behind the same API as WASM/native acceleration — pure upside, same behavior, verified by a shared corpus.

## Trust

Zero runtime dependencies. No install scripts. 4-file package you can read in ten minutes. Published exclusively from CI with npm provenance. Fuzzed, differential-tested against `JSON.parse` (436 real-world probes, 1 disagreement — which was our bug, fixed). See [SECURITY.md](./SECURITY.md).

## About

Built and maintained by [Todd Garland](https://x.com/toddo) ([@toddo](https://x.com/toddo)),
who has run [jsonlint.com](https://jsonlint.com) for many years — this engine is
what the site itself uses. Engineered in collaboration with Claude Fable 5
(Anthropic): the issue mining, differential testing, and fuzzing methodology in
[HARDENING.md](./HARDENING.md) was designed and executed together, and every
claim above is backed by tests that run in public CI — trust the corpus, not
the authors.

## Repo layout

- `packages/core` — the npm package (pure JS, zero deps)
- `crates/json-core` — Rust engine (same diagnostics, WASM/napi targets)
- `bench/` — reproducible benchmark harness
- `integration/jsonlint-com` — how jsonlint.com uses this

MIT
