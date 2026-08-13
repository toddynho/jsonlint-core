// Hardening gauntlet for @jsonlint/core
import { readdirSync, readFileSync } from "node:fs";
import { validate, tryParse, parse } from "../src/index.js";

let pass = 0, fail = 0;
const failures = [];
function check(name, cond, detail = "") {
  if (cond) pass++;
  else { fail++; failures.push(`${name} ${detail}`); }
}
function safeValidate(input, opts) {
  try { return { threw: false, r: validate(input, opts) }; }
  catch (e) { return { threw: true, e }; }
}

// ---- 1. json5-tests corpus (extension oracle, strict mode) ----
const C2 = process.env.CORPORA_DIR ?? "/tmp";
const root = `${C2}/json5-tests-master`;
let j5 = { pass: 0, miss: [] };
for (const dir of ["arrays", "comments", "misc", "new-lines", "numbers", "objects", "strings"]) {
  for (const f of readdirSync(`${root}/${dir}`)) {
    const bytes = new Uint8Array(readFileSync(`${root}/${dir}/${f}`));
    const { threw, r } = safeValidate(bytes);
    if (threw) { j5.miss.push(`${dir}/${f} THREW`); continue; }
    // json5-tests labels this .json but JSON.parse itself rejects it (trailing
    // comment after top-level value); our contract is JSON.parse parity.
    if (f === "irregular-block-comment.json") continue;
    const expectOk = f.endsWith(".json"); // .json5/.js/.txt are all strict-invalid
    if (r.ok === expectOk) j5.pass++;
    else j5.miss.push(`${dir}/${f} expected ${expectOk ? "valid" : "invalid"}`);
  }
}
console.log(`json5-tests (strict oracle): ${j5.pass} pass, ${j5.miss.length} miss`);
j5.miss.forEach(m => console.log("  MISS:", m));
check("json5-tests strict", j5.miss.length === 0);

// jsonc bonus oracle: comment + trailing-comma cases must pass in jsonc mode
let jc = { pass: 0, miss: [] };
for (const dir of ["comments", "arrays", "objects"]) {
  for (const f of readdirSync(`${root}/${dir}`)) {
    if (!f.endsWith(".json5")) continue;
    if (!/comment|trailing/.test(f)) continue;
    const bytes = new Uint8Array(readFileSync(`${root}/${dir}/${f}`));
    const { threw, r } = safeValidate(bytes, { mode: "jsonc" });
    if (!threw && r.ok) jc.pass++; else jc.miss.push(`${dir}/${f}`);
  }
}
console.log(`json5-tests (jsonc oracle): ${jc.pass} pass, ${jc.miss.length} miss`, jc.miss.join(", "));

// ---- 2. jsonc-parser invalid cases (jsonc mode) ----
const jsoncInvalid = [
  "{,}", "{ \"foo\": true, \"foo\": false }", "{ \"bar\": 8 \"xoo\": \"foo\" }",
  "{ ,\"bar\": 8 }", "{ ,\"bar\": 8, \"foo\" }", "{ \"bar\": 8, \"foo\": }",
  "{ 8, \"foo\": 9 }", "[,]", "[ 1 2, 3 ]", "[ ,1, 2, 3 ]", "[ ,1, 2, 3, ]",
  "", "1,1",
];
let ji = 0, jm = [];
for (const c of jsoncInvalid) {
  const { threw, r } = safeValidate(c, { mode: "jsonc" });
  // dup-keys case is warn-by-default; use error policy so it counts as invalid
  const r2 = c.includes('"foo": true, "foo"')
    ? validate(c, { mode: "jsonc", duplicateKeys: "error" }) : (threw ? null : r);
  if (r2 && !r2.ok) ji++; else jm.push(JSON.stringify(c.slice(0, 30)));
}
console.log(`jsonc-parser invalid cases: ${ji}/${jsoncInvalid.length}`, jm.join(" "));
check("jsonc invalid cases", jm.length === 0);

// jsonc-parser valid-in-jsonc cases
const jsoncValid = [
  "// comment\n{}", "/* block */ []", '{ "a": 1, /* mid */ "b": 2 }',
  '[ 1, 2, ]', '{ "hello": [], }', '{"a"://line\n1}',
  '["a", "sdf//sd"]', '[/*"a",*/ "sdfsd"]', // comments-in-strings & strings-in-comments
];
let jv = 0, jvm = [];
for (const c of jsoncValid) {
  const { threw, r } = safeValidate(c, { mode: "jsonc" });
  if (!threw && r.ok) jv++; else jvm.push(JSON.stringify(c.slice(0, 30)));
}
console.log(`jsonc-parser valid cases: ${jv}/${jsoncValid.length}`, jvm.join(" "));
check("jsonc valid cases", jvm.length === 0);

// ---- 3. Seldaek / Composer real-world cases ----
{
  check("empty input errors cleanly", (() => {
    const r = validate("");
    return !r.ok && r.diagnostics.length === 1 && r.diagnostics[0].code === "E004";
  })());
  check("whitespace-only input", !validate("   \n\t ").ok);
  check("bare word ABCD", !validate("ABCD").ok);
  check("unterminated at EOF {\"", !validate('{"').ok);
  check("empty-string key valid", validate('{"":"foo"}').ok);
  check("duplicate empty keys caught", validate('{"":1,"":2}').diagnostics.some(d => d.code === "W060"));
  check("\\u1f47d is 4-hex + literal d", tryParse('"\\u1f47d"').value === "\u1f47" + "d");
  check("raw emoji valid", validate('"👻"').ok && tryParse('"👻"').value === "👻");
  check("escaped solidus", tryParse('"http:\\/\\/foo.com"').value === "http://foo.com");
  check("double backslash", tryParse('"zo\\\\mg"').value === "zo\\mg");
  check("\\z invalid escape", !validate('{"foo": "bar\\z"}').ok);
  check("string then junk after close", !validate('{"bar": "foo}').ok); // quote swallows brace: unterminated
}

// ---- 4. Pathological inputs (no crash, bounded time, capped diagnostics) ----
{
  const t = (fn) => { const t0 = Date.now(); fn(); return Date.now() - t0; };
  const deep = "[".repeat(100000);
  check("100k open brackets bounded", t(() => {
    const r = validate(deep);
    check("  ...and errors", !r.ok);
  }) < 10000);
  check("100k closers no crash", t(() => validate("]".repeat(100000))) < 10000);
  const commas = ",".repeat(500000);
  check("500k commas bounded", t(() => validate(commas)) < 15000);
  const manyDup = "{" + Array.from({length: 5000}, (_, i) => `"k${i % 10}":1`).join(",") + "}";
  check("5k members w/ dup detection bounded", t(() => validate(manyDup)) < 15000);
  const longStr = '"' + "a".repeat(5_000_000) + '"';
  check("5MB single string bounded", t(() => validate(longStr)) < 8000);
  const nulls = "\u0000".repeat(200000);
  check("200k NULs no crash", t(() => validate(nulls)) < 10000);
  check("lone minus", !validate("-").ok);
  check("just a dot", !validate(".").ok);
  check("1e999 -> Infinity like JSON.parse", parse("1e999") === Infinity);
  check("diagnostics hard cap", validate(",".repeat(10000)).diagnostics.length <= 100);
}

// ---- 5. Fuzz: random + mutation, engine must never throw or hang ----
{
  let seed = 0x2F6E2B1;
  const rnd = () => (seed = (seed * 1103515245 + 12345) & 0x7FFFFFFF) / 0x7FFFFFFF;
  const alphabet = '{}[]",:0123456789.eE+-\\ truefalsn\n\r\t\'\u0000\uD800\uFFFD/*ab';
  const base = '{"a":[1,2.5,{"b":"x\\n","c":true}],"d":null}';
  let threwCount = 0;
  const t0 = Date.now();
  for (let i = 0; i < 30000; i++) {
    let input;
    if (i % 3 === 0) {
      // pure random
      let len = 1 + Math.floor(rnd() * 60);
      input = Array.from({length: len}, () => alphabet[Math.floor(rnd() * alphabet.length)]).join("");
    } else {
      // mutate valid doc
      const chars = base.split("");
      const edits = 1 + Math.floor(rnd() * 4);
      for (let e = 0; e < edits; e++) {
        const p = Math.floor(rnd() * chars.length);
        const op = rnd();
        if (op < 0.4) chars[p] = alphabet[Math.floor(rnd() * alphabet.length)];
        else if (op < 0.7) chars.splice(p, 1);
        else chars.splice(p, 0, alphabet[Math.floor(rnd() * alphabet.length)]);
      }
      input = chars.join("");
    }
    try {
      const r = tryParse(input, { mode: i % 2 ? "jsonc" : "strict" });
      // invariant: ok implies JSON.parse also accepts (strict mode, no warnings-only docs)
      if (i % 2 === 0 && r.ok) {
        try { JSON.parse(input); } catch {
          // Acceptable divergences: none expected in strict — record
          threwCount++; failures.push("strict-accepts-but-JSON.parse-rejects: " + JSON.stringify(input.slice(0, 60)));
        }
      }
    } catch (e) {
      threwCount++;
      failures.push("ENGINE THREW on: " + JSON.stringify(input.slice(0, 60)) + " :: " + e.message);
      if (threwCount > 5) break;
    }
  }
  console.log(`fuzz: 30000 inputs in ${Date.now() - t0}ms, ${threwCount} violations`);
  check("fuzz clean", threwCount === 0);
}

console.log(`\n=== ${pass} passed, ${fail} failed ===`);
failures.slice(0, 15).forEach(f => console.log("  " + f));
process.exit(fail ? 1 : 0);
