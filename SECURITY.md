# Security

## Supply-chain posture

- **Zero runtime dependencies.** The published package contains 4 files of
  auditable source. Nothing is compiled or executed at install time — there
  are no install scripts and never will be.
- Releases are published from GitHub Actions with **npm provenance** (OIDC
  trusted publishing); every version links back to the exact commit and
  workflow run that produced it. Provenance is not a silver bullet — read the
  source; it's short.
- npm org (@jsonlint) requires 2FA; no publish from developer machines.

## Parser hardening

- No `eval`, `Function`, or regex-based parsing (no ReDoS surface).
- `__proto__` never pollutes prototypes; depth-limited (default 512);
  diagnostics capped at 100; fuzzed (130k inputs across JS and Rust engines)
  and differential-tested against JSON.parse. See HARDENING.md.

## Reporting

Email security@jsonlint.com. We aim to respond within 72 hours.
