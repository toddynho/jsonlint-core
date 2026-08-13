// @jsonlint/core — pure-JS engine. Same diagnostic codes and behavior as the
// Rust core (json-core); WASM/native builds slot in behind this API later.
// Zero dependencies. No install scripts. Offsets are UTF-16 code units.

const MAX_DIAGNOSTICS = 100;

// token kinds
const T_LBRACE = 1, T_RBRACE = 2, T_LBRACKET = 3, T_RBRACKET = 4, T_COLON = 5,
      T_COMMA = 6, T_STR = 7, T_NUM = 8, T_TRUE = 9, T_FALSE = 10, T_NULL = 11,
      T_IDENT = 12, T_EOF = 13;

class Ctx {
  constructor(src, opts, wantValue) {
    this.src = src;
    this.n = src.length;
    this.pos = 0;
    this.mode = opts.mode ?? "strict";
    this.maxDepth = opts.maxDepth ?? 512;
    this.dupKeys = opts.duplicateKeys ?? "warn";
    this.protoKeys = opts.protoKeys ?? "safe";
    this.wantValue = wantValue;
    this.diagnostics = [];
    // current token
    this.t = T_EOF; this.tv = null; this.ts = 0; this.te = 0;
  }

  diag(code, message, start, end, hint, severity = "error", related) {
    if (this.diagnostics.length < MAX_DIAGNOSTICS) {
      const d = { code, message, severity, start, end };
      if (hint) d.hint = hint;
      if (related) d.related = related;
      this.diagnostics.push(d);
    }
  }

  // ------------------------------------------------------------------ lexer

  skipTrivia() {
    const s = this.src;
    for (;;) {
      const c = s.charCodeAt(this.pos);
      if (c === 32 || c === 9 || c === 13 || c === 10) { this.pos++; continue; }
      if (c === 47 /* / */) {
        const start = this.pos, c2 = s.charCodeAt(this.pos + 1);
        if (c2 === 47) {
          while (this.pos < this.n && s.charCodeAt(this.pos) !== 10) this.pos++;
        } else if (c2 === 42 /* * */) {
          this.pos += 2;
          let closed = false;
          while (this.pos < this.n) {
            if (s.charCodeAt(this.pos) === 42 && s.charCodeAt(this.pos + 1) === 47) {
              this.pos += 2; closed = true; break;
            }
            this.pos++;
          }
          if (!closed) this.diag("E020", "unterminated block comment", start, this.pos);
        } else break;
        if (this.mode === "strict") {
          this.diag("E021", "comments are not allowed in strict JSON", start, this.pos,
            'comments are valid in JSONC — enable mode: "jsonc" if this is a config file');
        }
        continue;
      }
      break;
    }
  }

  next() {
    for (;;) {
    this.skipTrivia();
    const s = this.src, start = this.pos;
    this.ts = start;
    if (start >= this.n) { this.t = T_EOF; this.te = start; return; }
    const c = s.charCodeAt(start);
    switch (c) {
      case 123: this.pos++; this.t = T_LBRACE; break;
      case 125: this.pos++; this.t = T_RBRACE; break;
      case 91:  this.pos++; this.t = T_LBRACKET; break;
      case 93:  this.pos++; this.t = T_RBRACKET; break;
      case 58:  this.pos++; this.t = T_COLON; break;
      case 44:  this.pos++; this.t = T_COMMA; break;
      case 34:  this.lexString(34); break;
      case 39:
        this.diag("E010", "strings must use double quotes", start, start + 1,
          "single-quoted strings are valid in JSON5 but not JSON/JSONC");
        this.lexString(39);
        break;
      case 0x201C: case 0x201D: case 0x2018: case 0x2019: {
        this.diag("E011", "smart quote found where a string was expected", start, start + 1,
          'this looks like text pasted from a word processor — replace \u201C \u201D with straight quotes "');
        this.pos++;
        const sstart = this.pos;
        while (this.pos < this.n) {
          const q = s.charCodeAt(this.pos);
          if (q === 0x201C || q === 0x201D || q === 0x2018 || q === 0x2019) break;
          if (q === 44 || q === 58 || q === 125 || q === 93 || q === 10) break;
          this.pos++;
        }
        this.tv = s.slice(sstart, this.pos);
        const qq = s.charCodeAt(this.pos);
        if (qq === 0x201C || qq === 0x201D || qq === 0x2018 || qq === 0x2019) this.pos++;
        this.t = T_STR;
        break;
      }
      default:
        if (c === 45 || (c >= 48 && c <= 57)) { this.lexNumber(); }
        else if (c === 43 || c === 46) {
          this.diag("E012", `numbers cannot start with '${s[start]}'`, start, start + 1,
            "valid in JSON5, not JSON/JSONC");
          this.pos++;
          this.lexNumber();
        }
        else if ((c >= 65 && c <= 90) || (c >= 97 && c <= 122) || c === 95 || c === 36) { this.lexIdent(); }
        else {
          this.pos++;
          this.diag("E001", `unexpected character (0x${c.toString(16).toUpperCase().padStart(2, "0")})`, start, this.pos);
          continue;
        }
    }
    this.te = this.pos;
    return;
    }
  }

  lexString(quote) {
    const s = this.src, open = this.pos;
    this.pos++;
    let out = this.wantValue ? "" : null;
    let runStart = this.pos;
    let sawControlEscape = false;
    for (;;) {
      if (this.pos >= this.n) {
        this.diag("E013", "unterminated string", open, this.pos);
        break;
      }
      const c = s.charCodeAt(this.pos);
      if (c === quote) {
        if (out !== null) out += s.slice(runStart, this.pos);
        this.pos++;
        const raw = s.slice(open, this.pos);
        if (sawControlEscape && /^.[A-Za-z]:/.test(raw)) {
          this.diag("W051", "control escape in what looks like a Windows path", open, this.pos,
            "\\t here is a TAB character, not a backslash + t — double every backslash: C:\\\\temp",
            "warning");
        }
        break;
      }
      if (c === 10) {
        this.diag("E013", "unterminated string (newline reached)", open, this.pos,
          "did you forget a closing quote?");
        if (out !== null) out += s.slice(runStart, this.pos);
        break;
      }
      if (c === 92 /* \ */) {
        if (out !== null) out += s.slice(runStart, this.pos);
        this.pos++;
        const e = s.charCodeAt(this.pos);
        this.pos++;
        switch (e) {
          case 34: if (out !== null) out += '"'; break;
          case 39: if (out !== null) out += "'"; break;
          case 92: if (out !== null) out += "\\"; break;
          case 47: if (out !== null) out += "/"; break;
          case 98: sawControlEscape = true; if (out !== null) out += "\b"; break;
          case 102: sawControlEscape = true; if (out !== null) out += "\f"; break;
          case 110: sawControlEscape = true; if (out !== null) out += "\n"; break;
          case 114: sawControlEscape = true; if (out !== null) out += "\r"; break;
          case 116: sawControlEscape = true; if (out !== null) out += "\t"; break;
          case 117: { // \u
            const cp = this.hex4();
            if (cp === null) {
              this.diag("E014", "invalid \\u escape", this.pos - 2, this.pos,
                "expected four hex digits, e.g. \\u00e9");
            } else if (cp >= 0xD800 && cp <= 0xDBFF) {
              if (s.charCodeAt(this.pos) === 92 && s.charCodeAt(this.pos + 1) === 117) {
                this.pos += 2;
                const lo = this.hex4();
                if (lo === null) {
                  this.diag("E014", "invalid \\u escape", this.pos - 2, this.pos,
                    "expected four hex digits, e.g. \\u00e9");
                  if (out !== null) out += String.fromCharCode(cp);
                } else if (lo >= 0xDC00 && lo <= 0xDFFF) {
                  if (out !== null) out += String.fromCharCode(cp, lo);
                } else {
                  // both preserved: JSON.parse accepts lone surrogates
                  this.surrogateDiag();
                  if (out !== null) out += String.fromCharCode(cp, lo);
                }
              } else {
                this.surrogateDiag();
                if (out !== null) out += String.fromCharCode(cp);
              }
            } else if (cp >= 0xDC00 && cp <= 0xDFFF) {
              this.surrogateDiag();
              if (out !== null) out += String.fromCharCode(cp);
            } else {
              if (out !== null) out += String.fromCharCode(cp);
            }
            break;
          }
          default: {
            const winPath = /^.[A-Za-z]:\\/.test(s.slice(open, open + 4));
            this.diag("E015",
              `invalid escape sequence '\\${Number.isNaN(e) ? "?" : String.fromCharCode(e)}'`,
              this.pos - 2, this.pos,
              winPath ? "this looks like a Windows path — double every backslash: C:\\\\Users\\\\..." : undefined);
          }
        }
        runStart = this.pos;
        continue;
      }
      if (c < 0x20) {
        if (out !== null) out += s.slice(runStart, this.pos);
        this.controlCharDiag(c);
        this.pos++;
        runStart = this.pos;
        continue;
      }
      this.pos++;
    }
    this.tv = out === null ? "" : out;
    this.t = T_STR;
  }

  controlCharDiag(c) {
    const [name, esc] =
      c === 9 ? ["tab character", "\\t"] :
      c === 11 ? ["vertical tab", "\\u000b"] :
      c === 12 ? ["form feed", "\\f"] : ["control character", "a \\u0000-style escape"];
    this.diag("E016", `unescaped ${name} in string (0x${c.toString(16).toUpperCase().padStart(2, "0")})`,
      this.pos, this.pos + 1, `replace it with ${esc}`);
  }

  surrogateDiag() {
    this.diag("W017", "lone surrogate in \\u escape", Math.max(0, this.pos - 6), this.pos,
      "JSON.parse accepts this, but the string will contain an unpaired surrogate — many systems will corrupt or reject it",
      "warning");
  }

  hex4() {
    if (this.pos + 4 > this.n) return null;
    const h = this.src.slice(this.pos, this.pos + 4);
    if (!/^[0-9a-fA-F]{4}$/.test(h)) return null;
    this.pos += 4;
    return parseInt(h, 16);
  }

  lexNumber() {
    const s = this.src, start = this.pos;
    if (s.charCodeAt(this.pos) === 45) this.pos++;
    const intStart = this.pos;
    while (this.pos < this.n) { const d = s.charCodeAt(this.pos); if (d < 48 || d > 57) break; this.pos++; }
    const intLen = this.pos - intStart;
    if (intLen > 1 && s.charCodeAt(intStart) === 48) {
      this.diag("E018", "numbers may not have leading zeros", start, this.pos);
    }
    if (intLen === 0) this.diag("E019", "expected digits in number", start, this.pos + 1);
    let isInt = true;
    if (s.charCodeAt(this.pos) === 46) {
      isInt = false;
      this.pos++;
      const fs = this.pos;
      while (this.pos < this.n) { const d = s.charCodeAt(this.pos); if (d < 48 || d > 57) break; this.pos++; }
      if (this.pos === fs) this.diag("E019", "expected digit after decimal point", start, this.pos);
    }
    const ec = s.charCodeAt(this.pos);
    if (ec === 101 || ec === 69) {
      isInt = false;
      this.pos++;
      const sc = s.charCodeAt(this.pos);
      if (sc === 43 || sc === 45) this.pos++;
      const es = this.pos;
      while (this.pos < this.n) { const d = s.charCodeAt(this.pos); if (d < 48 || d > 57) break; this.pos++; }
      if (this.pos === es) this.diag("E019", "expected digit in exponent", start, this.pos);
    }
    const text = s.slice(start, this.pos);
    const num = Number(text);
    if (isInt && intLen > 15) {
      try {
        const big = BigInt(text);
        const abs = big < 0n ? -big : big;
        if (abs > 9007199254740992n && BigInt(num) !== big) {
          this.diag("W050",
            `integer exceeds JavaScript's safe range and loses precision (${text} becomes ${num.toFixed(0)})`,
            start, this.pos,
            "store large IDs as strings, or parse with a lossless-number option", "warning");
        }
      } catch { /* not a plain integer */ }
    }
    this.tv = num;
    this.t = T_NUM;
  }

  lexIdent() {
    const s = this.src, start = this.pos;
    while (this.pos < this.n) {
      const c = s.charCodeAt(this.pos);
      if ((c >= 65 && c <= 90) || (c >= 97 && c <= 122) || (c >= 48 && c <= 57) || c === 95 || c === 36) this.pos++;
      else break;
    }
    const w = s.slice(start, this.pos);
    switch (w) {
      case "true": this.t = T_TRUE; break;
      case "false": this.t = T_FALSE; break;
      case "null": this.t = T_NULL; break;
      case "True": case "False":
        this.diag("E030", `'${w}' is not valid JSON`, start, this.pos,
          `this looks like a Python literal — use '${w.toLowerCase()}'`);
        this.t = w === "True" ? T_TRUE : T_FALSE;
        break;
      case "None":
        this.diag("E030", "'None' is not valid JSON", start, this.pos,
          "this looks like a Python literal — use 'null'");
        this.t = T_NULL;
        break;
      case "undefined":
        this.diag("E030", "'undefined' is not valid JSON", start, this.pos, "use 'null'");
        this.t = T_NULL;
        break;
      case "NaN": case "Infinity":
        this.diag("E031", `'${w}' is not valid JSON`, start, this.pos,
          "valid in JSON5 only; JSON has no representation for it");
        this.tv = w === "NaN" ? NaN : Infinity;
        this.t = T_NUM;
        break;
      default:
        this.tv = w;
        this.t = T_IDENT;
    }
  }

  // ----------------------------------------------------------------- parser

  value(depth) {
    if (depth >= this.maxDepth) {
      this.diag("E040", "maximum nesting depth exceeded", this.ts, this.te);
      this.resync();
      return null;
    }
    switch (this.t) {
      case T_LBRACE: return this.object(depth + 1);
      case T_LBRACKET: return this.array(depth + 1);
      case T_STR: { const v = this.tv; this.next(); return v; }
      case T_NUM: { const v = this.tv; this.next(); return v; }
      case T_TRUE: this.next(); return true;
      case T_FALSE: this.next(); return false;
      case T_NULL: this.next(); return null;
      case T_IDENT: {
        this.diag("E003", `unexpected identifier '${this.tv}'`, this.ts, this.te,
          "bare words are not values — did you mean a quoted string?");
        const v = this.tv; this.next(); return v;
      }
      case T_EOF:
        this.diag("E004", "unexpected end of input, expected a value", this.ts, this.te);
        return null;
      default:
        this.diag("E005", `expected a value, found ${tokName(this.t)}`, this.ts, this.te);
        this.next();
        return null;
    }
  }

  object(depth) {
    const obj = this.wantValue ? {} : null;
    const seen = this.dupKeys === "allow" ? null : new Map();
    this.next(); // {
    let first = true;
    for (;;) {
      if (this.t === T_RBRACE) { this.next(); break; }
      if (this.t === T_EOF) { this.diag("E006", "unclosed object, expected '}'", this.ts, this.te); break; }
      if (this.t === T_COMMA && first) {
        this.diag("E007", "leading comma in object", this.ts, this.te);
        this.next();
        continue;
      }
      if (!first) {
        if (this.t === T_COMMA) {
          this.next();
          if (this.t === T_RBRACE) {
            if (this.mode === "strict") {
              this.diag("E008", "trailing comma in object", this.ts, this.te,
                'trailing commas are valid in JSONC — enable mode: "jsonc" if this is a config file');
            }
            this.next();
            break;
          }
        } else if (this.t === T_RBRACE) { this.next(); break; }
        else {
          this.diag("E009", `expected ',' or '}' between object members, found ${tokName(this.t)}`,
            this.ts, this.te, "did you forget a comma after the previous value?");
        }
      }
      first = false;
      // key
      let key;
      if (this.t === T_STR) {
        key = this.tv;
        this.checkDup(seen);
        this.next();
      } else if (this.t === T_IDENT) {
        key = this.tv;
        this.diag("E022", `object keys must be quoted: '${key}'`, this.ts, this.te,
          `write "${key}" — unquoted keys are valid in JSON5 only`);
        this.checkDup(seen);
        this.next();
      } else if (this.t === T_NUM) {
        key = String(this.tv);
        this.diag("E022", "object keys must be quoted strings", this.ts, this.te,
          "numbers cannot be keys in JSON");
        this.next();
      } else {
        this.diag("E023", `expected object key, found ${tokName(this.t)}`, this.ts, this.te);
        this.resyncMember();
        continue;
      }
      // colon
      if (this.t === T_COLON) this.next();
      else this.diag("E024", `expected ':' after object key, found ${tokName(this.t)}`, this.ts, this.te);
      const v = this.value(depth);
      if (obj !== null) {
        if (key === "__proto__") {
          if (this.protoKeys !== "allow") {
            Object.defineProperty(obj, key, { value: v, enumerable: true, writable: true, configurable: true });
          } else obj[key] = v;
        } else obj[key] = v;
      }
    }
    return obj;
  }

  checkDup(seen) {
    if (!seen) return;
    const raw = this.src.slice(this.ts, this.te);
    const prev = seen.get(raw);
    if (prev !== undefined) {
      this.diag("W060", `duplicate object key ${raw[0] === '"' ? raw : JSON.stringify(raw)}`,
        this.ts, this.te,
        "the last occurrence wins; earlier values are silently discarded",
        this.dupKeys === "error" ? "error" : "warning",
        { start: prev[0], end: prev[1] });
    } else {
      seen.set(raw, [this.ts, this.te]);
    }
  }

  array(depth) {
    const arr = this.wantValue ? [] : null;
    this.next(); // [
    let first = true;
    for (;;) {
      if (this.t === T_RBRACKET) { this.next(); break; }
      if (this.t === T_EOF) { this.diag("E006", "unclosed array, expected ']'", this.ts, this.te); break; }
      if (!first) {
        if (this.t === T_COMMA) {
          this.next();
          if (this.t === T_RBRACKET) {
            if (this.mode === "strict") {
              this.diag("E008", "trailing comma in array", this.ts, this.te,
                "trailing commas are valid in JSONC");
            }
            this.next();
            break;
          }
        } else if (this.t === T_RBRACKET) { this.next(); break; }
        else {
          this.diag("E009", `expected ',' or ']' between array elements, found ${tokName(this.t)}`,
            this.ts, this.te, "did you forget a comma after the previous value?");
        }
      }
      first = false;
      const v = this.value(depth);
      if (arr !== null) arr.push(v);
    }
    return arr;
  }

  resync() {
    let depth = 0;
    for (;;) {
      switch (this.t) {
        case T_EOF: return;
        case T_LBRACE: case T_LBRACKET: depth++; this.next(); break;
        case T_RBRACE: case T_RBRACKET:
          if (depth === 0) return;
          depth--; this.next(); break;
        case T_COMMA:
          if (depth === 0) return;
          this.next(); break;
        default: this.next();
      }
    }
  }

  resyncMember() {
    for (;;) {
      if (this.t === T_EOF || this.t === T_RBRACE || this.t === T_COMMA) break;
      this.next();
    }
    if (this.t === T_COMMA) this.next();
  }
}

function tokName(t) {
  return ["", "'{'", "'}'", "'['", "']'", "':'", "','", "a string", "a number",
          "a boolean", "a boolean", "'null'", "an identifier", "end of input"][t];
}

// ----------------------------------------------------------- input handling

function decodeInput(input, diags) {
  if (typeof input === "string") {
    return input;
  }
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  // UTF-16/32 detection (prantlf#15 class): one clear error beats a garbage cascade
  const utf16 =
    (bytes[0] === 0xFF && bytes[1] === 0xFE) || (bytes[0] === 0xFE && bytes[1] === 0xFF) ||
    (bytes.length >= 4 && bytes[0] !== 0 && bytes[1] === 0 && bytes[3] === 0) ||
    (bytes.length >= 4 && bytes[0] === 0 && bytes[2] === 0);
  if (utf16) {
    diags.push({
      code: "E026", severity: "error",
      message: "input appears to be UTF-16/UTF-32 encoded, not UTF-8",
      start: 0, end: Math.min(4, bytes.length),
      hint: "common with files saved by Windows tools (PowerShell, Visual Studio) — re-save as UTF-8",
    });
    return null;
  }
  try {
    return new TextDecoder("utf-8", { ignoreBOM: true, fatal: true }).decode(bytes);
  } catch {
    // Locate the first invalid byte (Seldaek#52 class: pasted Latin-1)
    const at = firstInvalidUtf8(bytes);
    diags.push({
      code: "E027", severity: "error",
      message: `invalid UTF-8 byte (0x${bytes[at].toString(16).toUpperCase().padStart(2, "0")}) in input`,
      start: at, end: at + 1,
      hint: "this often means Latin-1/Windows-1252 text was pasted in — re-encode the input as UTF-8",
    });
    // Continue with a lossy decode so the rest still gets linted
    const text = new TextDecoder("utf-8", { ignoreBOM: true }).decode(bytes);
    return text;
  }
}

/** Index of the first byte that starts an invalid UTF-8 sequence. */
function firstInvalidUtf8(b) {
  let i = 0;
  while (i < b.length) {
    const c = b[i];
    let need;
    if (c < 0x80) { i++; continue; }
    else if (c >= 0xC2 && c <= 0xDF) need = 1;
    else if (c >= 0xE0 && c <= 0xEF) need = 2;
    else if (c >= 0xF0 && c <= 0xF4) need = 3;
    else return i; // 0x80-0xC1 continuation/overlong lead, 0xF5+ out of range
    if (i + need >= b.length + 1 && i + need > b.length - 1 + 1) { /* fallthrough */ }
    for (let k = 1; k <= need; k++) {
      const cc = b[i + k];
      if (cc === undefined || (cc & 0xC0) !== 0x80) return i;
      // reject overlong/surrogate/out-of-range second bytes
      if (k === 1) {
        if (c === 0xE0 && cc < 0xA0) return i;
        if (c === 0xED && cc > 0x9F) return i; // UTF-16 surrogates
        if (c === 0xF0 && cc < 0x90) return i;
        if (c === 0xF4 && cc > 0x8F) return i;
      }
    }
    i += need + 1;
  }
  return 0;
}

function run(input, opts, wantValue) {
  const preDiags = [];
  let src = decodeInput(input, preDiags);
  if (src === null) {
    return { value: undefined, diagnostics: preDiags };
  }
  let bomOffset = 0;
  if (src.charCodeAt(0) === 0xFEFF) {
    src = src.slice(1);
    bomOffset = 1;
    const strict = (opts.mode ?? "strict") === "strict";
    preDiags.push({
      code: strict ? "E025" : "W001",
      severity: strict ? "error" : "warning",
      message: "leading UTF-8 byte-order mark (BOM)",
      start: 0, end: 1,
      hint: "some Windows editors add this — save as UTF-8 without BOM, or parse in jsonc mode",
    });
  }
  const ctx = new Ctx(src, opts, wantValue);
  ctx.next();
  const value = ctx.value(0);
  if (ctx.t !== T_EOF) {
    const concat = ctx.t === T_LBRACE || ctx.t === T_LBRACKET;
    ctx.diag("E002", "unexpected content after top-level value", ctx.ts, ctx.te,
      concat ? "looks like two JSON documents concatenated — did you mean an array, or NDJSON?" : undefined);
  }
  let diagnostics = preDiags.concat(ctx.diagnostics);
  if (bomOffset) {
    for (const d of diagnostics) {
      if (d.code !== "E025" && d.code !== "W001") {
        d.start += bomOffset; d.end += bomOffset;
        if (d.related) { d.related.start += bomOffset; d.related.end += bomOffset; }
      }
    }
  }
  diagnostics.sort((a, b) => a.start - b.start);
  diagnostics.length = Math.min(diagnostics.length, MAX_DIAGNOSTICS);
  return { value, diagnostics };
}

function isOk(diagnostics) {
  return diagnostics.every(d => d.severity !== "error");
}

/** Compute 1-based line/column for a UTF-16 offset (handles \n, \r\n, \r). */
export function lineColumn(src, offset) {
  let line = 1, col = 1;
  for (let i = 0; i < offset && i < src.length; i++) {
    const c = src.charCodeAt(i);
    if (c === 10) { line++; col = 1; }
    else if (c === 13) {
      if (src.charCodeAt(i + 1) === 10) i++;
      line++; col = 1;
      // note: when \r\n was skipped, loop's i++ lands past the pair
    } else col++;
  }
  return { line, column: col };
}

/** Validate only. Fastest path: strings are scanned, never materialized. */
export function validate(input, options = {}) {
  const { diagnostics } = run(input, options, false);
  return { ok: isOk(diagnostics), diagnostics };
}

/** Parse, never throw. Returns value (possibly recovered), ok, diagnostics. */
export function tryParse(input, options = {}) {
  const { value, diagnostics } = run(input, options, true);
  return { ok: isOk(diagnostics), value, diagnostics };
}

/** JSON.parse-compatible: returns the value or throws on the first error. */
export function parse(input, options = {}) {
  const { value, diagnostics } = run(input, options, true);
  const err = diagnostics.find(d => d.severity === "error");
  if (err) {
    const src = typeof input === "string" ? input : "";
    const { line, column } = lineColumn(src, err.start);
    const e = new SyntaxError(`${err.message} (line ${line}, column ${column})`);
    e.code = err.code;
    e.diagnostics = diagnostics;
    throw e;
  }
  if (options.reviver) return applyReviver(value, options.reviver);
  return value;
}

function applyReviver(holder, reviver) {
  function walk(holder, key) {
    const val = holder[key];
    if (val && typeof val === "object") {
      for (const k of Object.keys(val)) {
        const v = walk(val, k);
        if (v === undefined) delete val[k];
        else val[k] = v;
      }
    }
    return reviver.call(holder, key, val);
  }
  return walk({ "": holder }, "");
}
