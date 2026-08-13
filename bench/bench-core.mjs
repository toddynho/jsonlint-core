// @jsonlint/core (pure-JS) vs incumbents, same methodology as bench.js
import fs from "node:fs";
import { createRequire } from "node:module";
import { validate, tryParse } from "../packages/core/src/index.js";
const require = createRequire(import.meta.url);
const jsonlint = require("jsonlint");
const JSON5 = require("json5");
const jsonc = require("jsonc-parser");

const docs = {
  "100kb": fs.readFileSync("fixtures/100kb.json", "utf8"),
  "5mb": fs.readFileSync("fixtures/5mb.json", "utf8"),
};

function bench(fn, input, ms) {
  for (let i = 0; i < 3; i++) fn(input);
  let iters = 0;
  const t0 = process.hrtime.bigint();
  let el = 0n;
  const budget = BigInt(ms) * 1000000n;
  while (el < budget) { fn(input); iters++; el = process.hrtime.bigint() - t0; }
  return Number(el) / 1e6 / iters;
}

for (const [size, doc] of Object.entries(docs)) {
  const mb = Buffer.byteLength(doc) / 1048576;
  const budget = size === "5mb" ? 3000 : 1500;
  console.log(`\n=== ${size} ===`);
  const rows = {
    "JSON.parse": () => JSON.parse(doc),
    "@jsonlint/core validate": () => validate(doc),
    "@jsonlint/core tryParse": () => tryParse(doc),
    "jsonc-parser": () => jsonc.parse(doc),
    "json5": () => JSON5.parse(doc),
    "jsonlint": () => jsonlint.parse(doc),
  };
  const base = {};
  for (const [name, fn] of Object.entries(rows)) {
    const msOp = bench(fn, doc, budget);
    if (name === "jsonlint") base.lint = msOp;
    console.log(name.padEnd(26), msOp.toFixed(3).padStart(9), "ms", (mb / (msOp / 1000)).toFixed(1).padStart(8), "MB/s");
  }
}

// error path: broken 100kb
const good = docs["100kb"];
const i = Math.floor(good.length * 0.8);
const bad = good.slice(0, i) + "}{" + good.slice(i);
console.log("\n=== broken 100kb (diagnostics path) ===");
for (const [name, fn] of Object.entries({
  "@jsonlint/core validate": () => validate(bad),
  "jsonlint (throws)": () => { try { jsonlint.parse(bad); } catch {} },
})) {
  console.log(name.padEnd(26), bench(fn, bad, 1500).toFixed(3).padStart(9), "ms");
}
const r = validate(bad);
console.log("diagnostics found:", r.diagnostics.length, "| first:", r.diagnostics[0].code, r.diagnostics[0].message.slice(0, 60));
