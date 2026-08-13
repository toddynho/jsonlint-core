# Hardening report

How this engine was hardened before its first release, and what that process found.
Everything below is reproducible: the corpora fetch in `scripts/fetch-corpora.sh`,
the gauntlets in `packages/core/test/harden.js` and `crates/json-core/examples/harden.rs`,
and CI runs all of it on every commit.

## Sources

- Complete issue histories (411 issues, 1,389 comments) of six predecessor and
  adjacent projects — see [ISSUE_AUDIT.md](./ISSUE_AUDIT.md)
- [JSONTestSuite](https://github.com/nst/JSONTestSuite) (283 verdict files)
- The historical test corpora of zaach/jsonlint and @prantlf/jsonlint (vendored,
  MIT, in `crates/json-core/corpus`)
- The official [json5-tests](https://github.com/json5/json5-tests) corpus
  (120 files, extension-as-oracle)
- Test cases extracted from Seldaek/jsonlint (Composer's linter) and
  microsoft/node-jsonc-parser
- Fuzzing: 30k inputs (JS) + 100k inputs (Rust), deterministic mutation + random
- Differential testing: 436 JSON snippets extracted from real issue threads, our
  verdict vs `JSON.parse`

## Bugs found and fixed by this process

1. **Lexer stack overflow** (both engines): the unknown-character path recursed
   per byte; 1MB of pasted junk meant a 1M-deep recursion. Now iterative — 1MB of
   junk yields 100 capped diagnostics in ~120ms.
2. **Invalid UTF-8 laundering** (both engines; the bug class that bit Composer,
   Seldaek#52): Latin-1 bytes were silently replaced with U+FFFD and validated OK.
   Now E027, pointing at the first invalid byte, with overlong and
   surrogate-encoding rejection; the rest of the document is still linted.
3. **Lone-surrogate over-rejection** (both engines, json5#192): `JSON.parse`
   accepts `"\uDEAD"`; we errored. Now W017 warning. The JS engine preserves the
   surrogate exactly like `JSON.parse`; the Rust engine substitutes U+FFFD (Rust
   strings cannot hold unpaired surrogates — a documented divergence). Malformed
   `\u` hex remains a hard error. This was the only verdict disagreement across
   all 436 differential probes.
4. **`TextDecoder` BOM stripping** (JS): byte-input BOM detection was a silent
   no-op until `ignoreBOM: true`.
5. **Wrong-byte blame in UTF-8 errors** (JS): the first fix located the
   continuation byte, not the invalid lead byte; replaced with a proper scanner.
6. **Windows-path silent corruption** (from circlecell#7): `"C:\temp"` is valid
   JSON in which `\t` silently becomes a TAB. W051 warns, escape-aware, with no
   false positive on properly doubled paths.

## Verification matrix (all green, both engines where applicable)

- JSONTestSuite: 95/95 must-accept, 188/188 must-reject
- Legacy corpora: zaach 32+3, prantlf 33+16 (100%)
- json5-tests strict oracle: 121/121 JS, 111/111 Rust (one corpus file exempted:
  `JSON.parse` itself rejects it; our contract is `JSON.parse` parity)
- json5-tests jsonc oracle (comment/trailing-comma files): 9/9
- node-jsonc-parser invalid/valid case lists: 13/13, 8/8
- Pathological: 100k unbalanced brackets, 500k commas, 5k members with duplicate
  detection, 5MB single-token string, 200k NUL bytes, 1MB junk — bounded time,
  no crash, diagnostics capped at 100
- Fuzz: 130k inputs across both engines, zero violations, including the
  invariant "strict-mode accept implies `JSON.parse` accept"
- Unit/regression: 35 Rust tests, 40 JS checks + 27 hardening checks, with
  issue-derived tests named for their sources

## Known limitations

- Duplicate-key detection compares raw key tokens; escaped-equivalent keys
  (`"a"` vs `"\u0061"`) are not flagged.
- Rust engine substitutes U+FFFD for lone surrogates (see #3 above); the JS
  engine matches `JSON.parse` exactly.
- JSON5 mode is not yet implemented; `jsonc` covers comments and trailing commas.
