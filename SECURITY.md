# Security

## Supply-chain posture

- **Zero runtime dependencies.** The published package contains 4 files of
  auditable source. Nothing is compiled or executed at install time — there
  are no install scripts and never will be.
- Releases are published from GitHub Actions with **npm provenance** (OIDC
  trusted publishing); every version links back to the exact commit and
  workflow run that produced it. Provenance is not a silver bullet — read the
  source; it's short.
- One documented exception: **v0.1.0 was bootstrap-published manually with
  2FA**, because npm requires a package to exist before a trusted publisher
  can be configured. Immediately after, the trusted publisher was linked and
  token-based publishing disallowed; every version after 0.1.0 is CI-only.
- npm org (@jsonlint) requires 2FA; no tokens, no publishes from developer
  machines beyond the bootstrap above.

## Parser hardening

- No `eval`, `Function`, or regex-based parsing (no ReDoS surface).
- `__proto__` never pollutes prototypes; depth-limited (default 512);
  diagnostics capped at 100; fuzzed (130k inputs across JS and Rust engines)
  and differential-tested against JSON.parse. See HARDENING.md.

## Reporting

Email security@jsonlint.com. We aim to respond within 72 hours.
