# json-core

Diagnostics-grade JSON engine. Zero dependencies. One Rust core, targets napi (Node), WASM (browser / jsonlint.com), and a pure-JS fallback (planned).

## Status: Milestone 1 complete

- Lexer + recovering recursive-descent parser, `strict` and `jsonc` modes
- **Error recovery**: reports every error in one pass (not just the first), each with stable code, byte span, line/col, and contextual hint
- Hint table for real-world mistakes: Python literals (`True`/`None`), single quotes, smart quotes from copy-paste, unquoted keys, trailing commas ("valid in JSONC — enable it"), concatenated documents ("did you mean NDJSON?")
- **Sink architecture**: parser is generic over output — `TreeSink` (DOM), `NullSink` (validation, allocation-free string scanning), tape/event sinks slot in next
- Security: depth limits (default 512), diagnostic cap; proto-key policy lands in the JS binding layer
- WASM C ABI (`src/wasm_api.rs`) + JS loader (`js/loader.js`), no wasm-bindgen — tiny binary, no install scripts

## Results (container-grade hardware, rustc 1.75)

| Workload | json-core | Incumbent | Speedup |
|---|---|---|---|
| Validate 25 MB | ~238 MB/s | jsonlint 2.5 MB/s | ~95x |
| Diagnose broken 100 KB | 0.39 ms | jsonlint 29.6 ms | ~75x |
| JSONTestSuite | 95/95 y_, 188/188 n_ | — | full conformance |

(Validation is allocation-free and outruns `JSON.parse` (~165 MB/s here), but that comparison is validate-vs-parse; keep claims honest.)

## Build & test

```sh
cargo test --release
cargo run --release --example lint -- broken.json          # pretty diagnostics
cargo run --release --example lint -- config.json --jsonc
cargo run --release --example conformance -- JSONTestSuite/test_parsing
```

## WASM build (for jsonlint.com)

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
# optional: wasm-opt -Oz target/wasm32-unknown-unknown/release/json_core.wasm -o json_core.wasm
```

Then from JS:

```js
import { init, validate } from "./js/loader.js";
await init(fetch("/json_core.wasm"));
const { ok, diagnostics } = validate(text, { mode: "jsonc" });
// diagnostics: [{ code, message, severity, start, end, line, column, hint? }]
```

## Issue-mined features (from 15 years of jsonlint GitHub history)

| Community issue | Status |
|---|---|
| zaach#13 / #85: duplicate keys pass silently (open since 2011, most-upvoted ever) | W060 warning by default, `DupKeys::Error` policy, reports first-occurrence location (the jsonlint-pos feature) |
| prantlf fork: leading UTF-8 BOM breaks parsing | W001: skipped with warning + hint |
| json5#101: line numbers wrong with \r\n / \r newlines | LineIndex handles \n, \r\n, \r |
| Large integer IDs silently corrupt through doubles | W050 warning showing the exact corrupted value |
| codenothing#4: literal line breaks in strings not caught | E013/E016 |
| zaach#89: 1GB files exhaust memory | streaming sink (milestone 5) |
| Error position ≠ mistake position ("check the line above") | recovery hints point at the previous member |

## Legacy corpus compatibility

Runs the historical test suites of both predecessor projects (`examples/legacy.rs`):
zaach/jsonlint 32/32 fails + 3/3 passes; @prantlf/jsonlint 33/33 fails + 16/16 passes,
including the BOM-rejection default and the `hasOwnProperty`/`constructor` key probes.
Strict mode rejects a leading BOM exactly like JSON.parse (E025, with recovery);
jsonc mode tolerates it with W001.

## Next milestones

2. napi addon + npm package skeleton (`@jsonlint/core`), `parse`/`tryParse`/`validate`
3. jsonlint.com swap-in (WASM validate behind existing UI)
4. json5 mode + pure-JS fallback
5. Streaming (event sink) + NDJSON fast path
6. Lazy tape mode
7. Public benchmark repo + launch

## Corpus attribution

The vendored corpora in `corpus/` are the historical test suites of
[zaach/jsonlint](https://github.com/zaach/jsonlint) and
[@prantlf/jsonlint](https://github.com/prantlf/jsonlint), both MIT-licensed.
Audit and hardening docs live at the repository root.
