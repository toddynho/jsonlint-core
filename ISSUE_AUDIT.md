# Issue audit

Before the first release, we pulled the complete issue history — open and closed,
including comment threads — of the six most relevant predecessor and adjacent
projects, and mapped every parser-relevant finding to this engine:

| Repo | Issues | Comments |
|---|---|---|
| zaach/jsonlint (the original, 2011–) | 93 | 223 |
| prantlf/jsonlint (maintained fork) | 30 | 72 |
| Seldaek/jsonlint (PHP port used by Composer) | 26 | 68 |
| circlecell/jsonlint (jsonlint.com) | 12 | 10 |
| json5/json5 | 197 | 910 |
| microsoft/node-jsonc-parser | 53 | 106 |
| **Total** | **411 issues** | **1,389 comments** |

436 concrete JSON snippets were extracted from issue bodies and comments and run
as a differential probe against `JSON.parse`. One disagreement was found — a bug
in this engine (lone-surrogate over-rejection), fixed before release.

## Resolved in this engine (regression-tested, tests named for their source)

| Source | Problem | Resolution |
|---|---|---|
| zaach#13, #85 (most-upvoted issue in repo history, open since 2011) | duplicate keys pass silently | W060 warn/error policy + first-occurrence location |
| circlecell#12 | duplicate-key false positive across sibling objects | per-object scoping, span-based comparison |
| circlecell#1 | duplicates raised a hard error; spec allows them | warning by default, matching ECMA-404 |
| zaach#63 | `1111111111258928239` silently becomes `...232` | W050 shows the exact corrupted value |
| circlecell#7 | Windows path escaping confusion | W051: `"C:\temp"` is valid JSON where `\t` becomes TAB — warned; invalid escapes in path-like strings get a "double the backslashes" hint |
| Seldaek#52, #15 | invalid UTF-8 (pasted Latin-1) accepted | E027 pointing at the first invalid byte; overlong/surrogate encodings rejected |
| prantlf#15 | valid-looking file fails at line 1 col 1 (UTF-16 file) | E026: one clear "re-save as UTF-8" diagnostic instead of a garbage cascade |
| Seldaek#54 | bad backslash deep in a string: wrong location, unrelated message | E015 names the escape; span points at it exactly |
| Seldaek#59 | `\u0022` decoded incorrectly | correct decode, regression-tested |
| zaach#24 | tab in string: cryptic jison error | E016 names the character and the fix |
| zaach#142 | invalid escape: wrong location | exact span |
| zaach#65 | `"[1,2,3]"` string misparsed as array | correct lexing |
| json5#192 | lone surrogates | accepted with W017 warning — `JSON.parse` parity (this was the one differential-probe disagreement) |
| json5#295 (CVE-2022-46175 class) | `__proto__` prototype pollution | blocked by design; `constructor`/`hasOwnProperty` keys safe (span-based, not JS-object-based, duplicate detection) |
| prantlf#1 | CRLF files: wrong line numbers | `\n`, `\r\n`, `\r` all handled |
| prantlf (fork raison d'être) | BOM handling | E025 strict (JSON.parse parity) / W001 tolerated in jsonc |
| zaach#119, #46, #50 | single quotes, comments | E010 with JSON5 hint; jsonc mode |
| Seldaek#31 | top-level scalars | valid per RFC 8259 |
| circlecell#11 | tab indentation | valid whitespace, regression-tested |

## Validates the roadmap (not yet built)

| Source | Feature |
|---|---|
| zaach#89, #96 | streaming for GB-scale files; NDJSON |
| zaach#37, #90 | formatter must round-trip escapes correctly (their pretty-printer corrupts `\\n` and `\/`) |
| jsonc#26, json5 corpus | JSON5 mode (single quotes, unquoted keys, line continuations) |
| zaach#62, #36, prantlf#39 | JSON Schema validation with error locations |
| zaach#77, #122, prantlf#20 | CLI: globs, exit codes, multi-file |

## User-misconception issues (turned into hints and docs, not code changes)

zaach#47 (raw unicode in strings is valid), zaach#113 (`/` needn't be escaped),
Seldaek#31 (scalars are valid documents) — the error-code documentation on
jsonlint.com explains these inline rather than "fixing" correct behavior.
