# jsonlint.com integration

## Rollout plan

1. **Shadow (week 1):** existing validator stays authoritative. Add
   `shadowCompare(text, legacyOk)` after each validation. Divergence beacons
   (verdict + first error code + length only — never the document) land at
   /api/shadow-divergence. Expect near-zero; every hit is either a legacy bug
   we fixed intentionally (check ISSUE_AUDIT.md) or a real gap to fix pre-flip.
2. **Flip:** render `lint()` diagnostics. Users go from one error to all
   errors + hints + duplicate-key/precision/path warnings in a single paste.
3. **Powered-by:** add "Validated by @jsonlint/core — npm install @jsonlint/core"
   with a link to the GitHub repo. This is the distribution flywheel.
4. **Error-code pages:** each diagnostic links to jsonlint.com/errors/{CODE}
   (e.g. /errors/W060). One page per code: what it means, example, fix.
   ~25 pages of high-intent SEO content that only this site can rank for,
   and every npm user of the package sees those codes too.

## Notes

- `mode: "jsonc"` toggle maps to a "allow comments & trailing commas" checkbox.
- The legacy duplicate-key false-positive (circlecell#12) and tab-indent issue
  (#11) are covered by regression tests in packages/core.
- Warnings (W050 precision, W051 windows paths, W060 duplicates, W017 lone
  surrogates) should render visually distinct from errors — the document is
  still valid; that nuance is the feature.
