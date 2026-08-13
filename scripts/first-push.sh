#!/bin/bash
# One-time: push this tree to a new GitHub repo you control.
# Usage: bash scripts/first-push.sh git@github.com:YOURORG/jsonlint-core.git
set -e
git init -b main
git add -A
git commit -m "jsonlint-core: diagnostics-grade JSON engine (JS + Rust)

Passes JSONTestSuite 283/283, legacy zaach+prantlf corpora 84/84,
json5-tests oracle, 130k-input fuzz, 436 differential probes vs JSON.parse.
Regression tests named for source issues across 6 predecessor repos.
See HARDENING.md and ISSUE_AUDIT.md."
git remote add origin "$1"
git push -u origin main
