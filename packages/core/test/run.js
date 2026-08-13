// Test runner for @jsonlint/core: JSONTestSuite conformance, legacy corpora,
// and issue-derived regressions. Run: node test/run.js
import { readdirSync, readFileSync } from "node:fs";
import { validate, tryParse, parse } from "../src/index.js";

let pass = 0, fail = 0;
function check(name, cond, detail = "") {
  if (cond) pass++;
  else { fail++; console.log(`  FAIL: ${name} ${detail}`); }
}

// --- JSONTestSuite ---
const C = process.env.CORPORA_DIR ?? "/tmp";
const jts = `${C}/JSONTestSuite-master/test_parsing`;
let y = [0, 0], n = [0, 0];
for (const f of readdirSync(jts).sort()) {
  if (!f.endsWith(".json")) continue;
  const bytes = readFileSync(`${jts}/${f}`);
  let ok;
  try { ok = validate(new Uint8Array(bytes)).ok; } catch { ok = false; }
  if (f.startsWith("y_")) { ok ? y[0]++ : (y[1]++, console.log("  REJECTED valid: " + f)); }
  else if (f.startsWith("n_")) { !ok ? n[0]++ : (n[1]++, console.log("  ACCEPTED invalid: " + f)); }
}
console.log(`JSONTestSuite: y ${y[0]} pass ${y[1]} fail | n ${n[0]} pass ${n[1]} fail`);
check("JSONTestSuite clean", y[1] + n[1] === 0);

// --- legacy corpora ---
for (const [dir, expectFail] of [
  [`${C}/mine/zaach/test/fails`, true], [`${C}/mine/zaach/test/passes`, false],
  [`${C}/mine/prantlf/test/fails`, true], [`${C}/mine/prantlf/test/passes`, false],
]) {
  let p = 0, misses = [];
  for (const f of readdirSync(dir).sort()) {
    if (!f.endsWith(".json")) continue;
    const ok = validate(new Uint8Array(readFileSync(`${dir}/${f}`))).ok;
    if (expectFail !== ok) p++; else misses.push(f);
  }
  console.log(`${dir.split("/").slice(-2).join("/")}: ${p} pass, ${misses.length} miss ${misses.join(",")}`);
  check(dir, misses.length === 0);
}

// --- issue regressions (same cases as the Rust suite) ---
{
  const r = validate('{"a":1,"a":2}');
  check("dup key warns, ok", r.ok && r.diagnostics.some(d => d.code === "W060"));
  const d = r.diagnostics.find(d => d.code === "W060");
  check("dup key related span", d.related && d.related.start === 1 && d.related.end === 4);
  check("dup key error policy", !validate('{"a":1,"a":2}', { duplicateKeys: "error" }).ok);
  check("dup scoped per object", validate('{"x":{"a":1},"y":{"a":2}}').diagnostics.every(d => d.code !== "W060"));
  const r50 = validate('{"fooId":1111111111258928239}');
  check("zaach#63 precision", r50.diagnostics.some(d => d.code === "W050" && d.message.includes("1111111111258928239")));
  check("safe int silent", validate('{"id":9007199254740991}').diagnostics.every(d => d.code !== "W050"));
  check("zaach#65 bracket string", validate('{"pos": "[106.675,525.792]"}').ok);
  const r15 = validate('{"": "foobar\\?"}');
  const e15 = r15.diagnostics.find(d => d.code === "E015");
  check("zaach#142 escape named+located", e15 && e15.message.includes("\\?") && e15.start === 12);
  const r16 = validate('{"action": "log\tin"}');
  check("zaach#24 tab named", r16.diagnostics.some(d => d.code === "E016" && d.message.includes("tab")));
  const utf16 = validate(new Uint8Array([0xFF, 0xFE, 0x7B, 0, 0x7D, 0]));
  check("prantlf#15 utf16", !utf16.ok && utf16.diagnostics.some(d => d.code === "E026") && utf16.diagnostics.length === 1);
  const bomDoc = new Uint8Array([0xEF, 0xBB, 0xBF, ...new TextEncoder().encode('{"a":1}')]);
  check("BOM strict error", !validate(bomDoc).ok && validate(bomDoc).diagnostics.some(d => d.code === "E025"));
  check("BOM jsonc warn", validate(bomDoc, { mode: "jsonc" }).ok);
  const proto = tryParse('{"__proto__": {"polluted": true}}');
  check("proto pollution blocked", proto.ok && ({}).polluted === undefined && proto.value.__proto__.polluted === true);
  const dupSafe = validate('{"constructor":1,"hasOwnProperty":2}');
  check("prantlf#23 no false dup", dupSafe.diagnostics.every(d => d.code !== "W060"));
  check("real ctor dup caught", validate('{"constructor":1,"constructor":2}').diagnostics.some(d => d.code === "W060"));
  const multi = validate(`{"a": 1, "b": 'x', "c": True,}`);
  const codes = multi.diagnostics.map(d => d.code);
  check("all errors one pass", codes.includes("E010") && codes.includes("E030") && codes.includes("E008"));
  const rec = tryParse('{"a": 1 "b": 2}');
  check("recovery builds both members", !rec.ok && rec.value.a === 1 && rec.value.b === 2);
  check("jsonc mode", validate('{\n// c\n"a": [1,2,],\n}', { mode: "jsonc" }).ok);
  const cat = validate('{"a":1}{"b":2}');
  check("concat hint", cat.diagnostics.some(d => d.code === "E002" && (d.hint || "").includes("NDJSON")));
  check("parse compat", JSON.stringify(parse('{"a":[1,2.5,-3e2],"b":null}')) === JSON.stringify(JSON.parse('{"a":[1,2.5,-3e2],"b":null}')));
  check("reviver", parse('{"a": 2}', { reviver: (k, v) => typeof v === "number" ? v * 10 : v }).a === 20);
  let threw = false;
  try { parse('{"a":}'); } catch (e) { threw = e instanceof SyntaxError && e.code; }
  check("parse throws SyntaxError with code", !!threw);
  check("smart quotes recovered", validate('{\u201Ca\u201D: 1}').diagnostics.some(d => d.code === "E011"));
  check("depth limit", !validate("[".repeat(600) + "]".repeat(600)).ok);
}

// --- issue-dump round (from the authenticated 6-repo sweep) ---
{
  const r = validate('"\\uDEAD"');
  check("json5#192 lone surrogate parity", r.ok && r.diagnostics.some(d => d.code === "W017"));
  check("lone surrogate value preserved", tryParse('"\\uDEAD"').value === "\uDEAD");
  check("malformed hex still error", !validate('"\\uD800\\u"').ok);
  check("circlecell#12 sibling objects", validate('[{"name":"a","img":"x"},{"name":"b","img":"y"}]').diagnostics.every(d => d.code !== "W060"));
  check("circlecell#11 tab indents", validate("{\n\t\"a\": 1\n}").ok);
  const single = validate('{"p": "C:\\temp"}');
  check("circlecell#7 silent path corruption warned", single.ok && single.diagnostics.some(d => d.code === "W051"));
  check("doubled path clean", validate('{"p": "C:\\\\temp\\\\"}').diagnostics.length === 0);
  const e15 = validate('{"p": "C:\\Users"}').diagnostics.find(d => d.code === "E015");
  check("windows hint on bad escape", e15 && /Windows path/.test(e15.hint || ""));
  const s54 = '{"k": "TYPO3\\PharStreamWrapper\\Exception"}';
  const d54 = validate(s54).diagnostics.find(d => d.code === "E015");
  check("seldaek#54 exact location", d54 && s54.slice(d54.start, d54.end) === "\\P");
  check("seldaek#59 \\u0022 decode", tryParse('"Argument \\u0022input\\u0022"').value === 'Argument "input"');
  check("seldaek#31 scalar top-level valid", validate("42").ok && validate('"str"').ok);
}

console.log(`\n${pass} passed, ${fail} failed`);
process.exit(fail ? 1 : 0);

