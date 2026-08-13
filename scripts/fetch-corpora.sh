#!/bin/bash
# Fetch external conformance corpora into $CORPORA_DIR (default /tmp).
# Vendored corpora (zaach/prantlf legacy) live in crates/json-core/corpus already.
set -e
DIR="${CORPORA_DIR:-/tmp}"
mkdir -p "$DIR/mine"
curl -sL https://codeload.github.com/nst/JSONTestSuite/zip/refs/heads/master -o "$DIR/jts.zip"
unzip -qo "$DIR/jts.zip" -d "$DIR"
curl -sL https://codeload.github.com/json5/json5-tests/zip/refs/heads/master -o "$DIR/j5t.zip"
unzip -qo "$DIR/j5t.zip" -d "$DIR"
(cd "$DIR/mine" && curl -sL https://codeload.github.com/zaach/jsonlint/zip/refs/heads/master -o z.zip && unzip -qo z.zip && mv jsonlint-master zaach)
(cd "$DIR/mine" && curl -sL https://codeload.github.com/prantlf/jsonlint/zip/refs/heads/master -o p.zip && unzip -qo p.zip && mv jsonlint-master prantlf)
echo "corpora ready in $DIR"
