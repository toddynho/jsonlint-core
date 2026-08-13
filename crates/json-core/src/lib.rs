//! json-core: diagnostics-grade JSON parser with error recovery.
//! Zero dependencies. Dialects: strict (RFC 8259) and JSONC. JSON5: milestone 4.

pub mod wasm_api;

// ---------------------------------------------------------------- diagnostics

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
    pub severity: Severity,
    /// Secondary location, e.g. the first occurrence of a duplicated key.
    pub related: Option<Span>,
}

pub const MAX_DIAGNOSTICS: usize = 100;

/// Maps byte offsets to 1-based (line, column). Built from newline offsets.
pub struct LineIndex {
    newlines: Vec<usize>,
}

impl LineIndex {
    pub fn new(src: &[u8]) -> Self {
        let mut newlines = Vec::new();
        let mut i = 0;
        while i < src.len() {
            match src[i] {
                b'\n' => newlines.push(i),
                b'\r' => {
                    if src.get(i + 1) == Some(&b'\n') { i += 1; }
                    newlines.push(i);
                }
                _ => {}
            }
            i += 1;
        }
        LineIndex { newlines }
    }

    pub fn line_col(&self, pos: usize) -> (usize, usize) {
        let line = self.newlines.partition_point(|&n| n < pos);
        let line_start = if line == 0 { 0 } else { self.newlines[line - 1] + 1 };
        (line + 1, pos - line_start + 1)
    }
}

// ---------------------------------------------------------------------- value

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    /// Insertion-ordered; duplicate policy applied by the parser.
    Object(Vec<(String, Value)>),
}

// ----------------------------------------------------------------------- sink

/// The parser is generic over its output. One parser, many products.
pub trait Sink {
    /// If false, the lexer validates strings but skips materializing their contents.
    fn wants_strings(&self) -> bool { true }
    fn null(&mut self) {}
    fn boolean(&mut self, _b: bool) {}
    fn number(&mut self, _n: f64) {}
    fn string(&mut self, _s: &str) {}
    fn begin_object(&mut self) {}
    fn key(&mut self, _k: &str) {}
    fn end_object(&mut self) {}
    fn begin_array(&mut self) {}
    fn end_array(&mut self) {}
}

/// Validation only: does nothing, as fast as possible. Powers jsonlint.com.
pub struct NullSink;
impl Sink for NullSink {
    fn wants_strings(&self) -> bool { false }
}

/// Builds a Value tree (DOM mode).
pub struct TreeSink {
    stack: Vec<Frame>,
    pub root: Option<Value>,
}

enum Frame {
    Array(Vec<Value>),
    Object(Vec<(String, Value)>, Option<String>),
}

impl TreeSink {
    pub fn new() -> Self {
        TreeSink { stack: Vec::new(), root: None }
    }

    fn push(&mut self, v: Value) {
        match self.stack.last_mut() {
            None => self.root = Some(v),
            Some(Frame::Array(items)) => items.push(v),
            Some(Frame::Object(pairs, pending)) => {
                let k = pending.take().unwrap_or_default();
                pairs.push((k, v));
            }
        }
    }
}

impl Sink for TreeSink {
    fn null(&mut self) { self.push(Value::Null); }
    fn boolean(&mut self, b: bool) { self.push(Value::Bool(b)); }
    fn number(&mut self, n: f64) { self.push(Value::Number(n)); }
    fn string(&mut self, s: &str) { self.push(Value::String(s.to_string())); }
    fn begin_array(&mut self) { self.stack.push(Frame::Array(Vec::new())); }
    fn end_array(&mut self) {
        if let Some(Frame::Array(items)) = self.stack.pop() {
            self.push(Value::Array(items));
        }
    }
    fn begin_object(&mut self) { self.stack.push(Frame::Object(Vec::new(), None)); }
    fn key(&mut self, k: &str) {
        if let Some(Frame::Object(_, pending)) = self.stack.last_mut() {
            *pending = Some(k.to_string());
        }
    }
    fn end_object(&mut self) {
        if let Some(Frame::Object(pairs, _)) = self.stack.pop() {
            self.push(Value::Object(pairs));
        }
    }
}

// ---------------------------------------------------------------------- lexer

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Colon,
    Comma,
    Str(String),
    Num(f64),
    True,
    False,
    Null,
    /// Bare word: unquoted key, Python literal, NaN, etc. Kept for hints/recovery.
    Ident(String),
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Strict,
    Jsonc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DupKeys {
    Allow,
    Warn,
    Error,
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    mode: Mode,
    pub keep_strings: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a [u8], mode: Mode) -> Self {
        let mut lx = Lexer { src, pos: 0, mode, keep_strings: true, diagnostics: Vec::new() };
        // UTF-16/32 input produces useless garbage-cascade errors downstream;
        // detect it up front and emit one clear diagnostic instead (prantlf#15 class).
        let utf16 = src.starts_with(&[0xFF, 0xFE]) || src.starts_with(&[0xFE, 0xFF])
            || (src.len() >= 4 && src[0] != 0 && src[1] == 0 && src[3] == 0)  // LE, no BOM
            || (src.len() >= 4 && src[0] == 0 && src[2] == 0);                 // BE, no BOM
        if utf16 {
            lx.pos = src.len(); // suppress the garbage cascade
            lx.diag("E026", "input appears to be UTF-16/UTF-32 encoded, not UTF-8".into(),
                Span { start: 0, end: src.len().min(4) },
                Some("common with files saved by Windows tools (PowerShell, Visual Studio) — re-save as UTF-8".into()),
                Severity::Error);
            return lx;
        }
        if src.starts_with(&[0xEF, 0xBB, 0xBF]) {
            lx.pos = 3;
            let (code, sev) = if mode == Mode::Strict {
                ("E025", Severity::Error) // JSON.parse rejects BOM; so do we in strict
            } else {
                ("W001", Severity::Warning)
            };
            lx.diag(code, "leading UTF-8 byte-order mark (BOM)".into(),
                Span { start: 0, end: 3 },
                Some("some Windows editors add this — save as UTF-8 without BOM, or parse in jsonc mode".into()),
                sev);
        }
        lx
    }

    fn diag(&mut self, code: &'static str, msg: String, span: Span, hint: Option<String>, sev: Severity) {
        if self.diagnostics.len() < MAX_DIAGNOSTICS {
            self.diagnostics.push(Diagnostic { code, message: msg, span, hint, severity: sev, related: None });
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n') => self.pos += 1,
                Some(b'/') => {
                    let start = self.pos;
                    match self.src.get(self.pos + 1) {
                        Some(b'/') => {
                            while let Some(b) = self.peek() {
                                if b == b'\n' { break; }
                                self.pos += 1;
                            }
                            self.comment_diag(start);
                        }
                        Some(b'*') => {
                            self.pos += 2;
                            let mut closed = false;
                            while self.pos < self.src.len() {
                                if self.src[self.pos] == b'*' && self.src.get(self.pos + 1) == Some(&b'/') {
                                    self.pos += 2;
                                    closed = true;
                                    break;
                                }
                                self.pos += 1;
                            }
                            if !closed {
                                self.diag("E020", "unterminated block comment".into(),
                                    Span { start, end: self.pos }, None, Severity::Error);
                            }
                            self.comment_diag(start);
                        }
                        _ => break,
                    }
                }
                _ => break,
            }
        }
    }

    fn comment_diag(&mut self, start: usize) {
        if self.mode == Mode::Strict {
            self.diag("E021", "comments are not allowed in strict JSON".into(),
                Span { start, end: self.pos },
                Some("comments are valid in JSONC — enable mode: \"jsonc\" if this is a config file".into()),
                Severity::Error);
        }
    }

    pub fn next_token(&mut self) -> Token {
        loop {
        self.skip_trivia();
        let start = self.pos;
        let b = match self.peek() {
            None => return Token { tok: Tok::Eof, span: Span { start, end: start } },
            Some(b) => b,
        };
        let tok = match b {
            b'{' => { self.pos += 1; Tok::LBrace }
            b'}' => { self.pos += 1; Tok::RBrace }
            b'[' => { self.pos += 1; Tok::LBracket }
            b']' => { self.pos += 1; Tok::RBracket }
            b':' => { self.pos += 1; Tok::Colon }
            b',' => { self.pos += 1; Tok::Comma }
            b'"' => self.lex_string(b'"'),
            b'\'' => {
                self.diag("E010", "strings must use double quotes".into(),
                    Span { start, end: start + 1 },
                    Some("single-quoted strings are valid in JSON5 but not JSON/JSONC".into()),
                    Severity::Error);
                self.lex_string(b'\'')
            }
            0xE2 => {
                // Smart quotes from copy-paste: U+201C/201D (E2 80 9C/9D), U+2018/2019 (E2 80 98/99)
                let b2 = self.src.get(self.pos + 1).copied();
                let b3 = self.src.get(self.pos + 2).copied();
                if b2 == Some(0x80) && matches!(b3, Some(0x98) | Some(0x99) | Some(0x9C) | Some(0x9D)) {
                    self.diag("E011", "smart quote found where a string was expected".into(),
                        Span { start, end: start + 3 },
                        Some("this looks like text pasted from a word processor — replace “ ” with straight quotes \"".into()),
                        Severity::Error);
                    self.pos += 3;
                    // Recover: lex until the matching closing smart quote or a structural byte.
                    let s_start = self.pos;
                    while let Some(c) = self.peek() {
                        if c == 0xE2 && self.src.get(self.pos + 1) == Some(&0x80)
                            && matches!(self.src.get(self.pos + 2), Some(0x98) | Some(0x99) | Some(0x9C) | Some(0x9D)) {
                            let text = String::from_utf8_lossy(&self.src[s_start..self.pos]).into_owned();
                            self.pos += 3;
                            return Token { tok: Tok::Str(text), span: Span { start, end: self.pos } };
                        }
                        if matches!(c, b',' | b':' | b'}' | b']' | b'\n') { break; }
                        self.pos += 1;
                    }
                    let text = String::from_utf8_lossy(&self.src[s_start..self.pos]).into_owned();
                    Tok::Str(text)
                } else {
                    self.pos += 1;
                    self.diag("E001", "unexpected character".into(),
                        Span { start, end: self.pos }, None, Severity::Error);
                    continue;
                }
            }
            b'-' | b'0'..=b'9' => self.lex_number(),
            b'+' | b'.' => {
                self.diag("E012", format!("numbers cannot start with '{}'", b as char),
                    Span { start, end: start + 1 },
                    Some("valid in JSON5, not JSON/JSONC".into()), Severity::Error);
                self.pos += 1;
                self.lex_number()
            }
            b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'$' => self.lex_ident(),
            _ => {
                self.pos += 1;
                self.diag("E001", format!("unexpected character (0x{:02X})", b),
                    Span { start, end: self.pos }, None, Severity::Error);
                continue;
            }
        };
        return Token { tok, span: Span { start, end: self.pos } };
        }
    }

    fn lex_string(&mut self, quote: u8) -> Tok {
        if !self.keep_strings {
            return self.scan_string(quote);
        }
        let open = self.pos;
        self.pos += 1; // opening quote
        let mut out = String::new();
        let mut saw_control_escape = false;
        loop {
            let b = match self.peek() {
                None => {
                    self.diag("E013", "unterminated string".into(),
                        Span { start: open, end: self.pos }, None, Severity::Error);
                    return Tok::Str(out);
                }
                Some(b) => b,
            };
            match b {
                q if q == quote => {
                    self.pos += 1;
                    if saw_control_escape { self.win_path_check(open); }
                    return Tok::Str(out);
                }

                b'\n' => {
                    self.diag("E013", "unterminated string (newline reached)".into(),
                        Span { start: open, end: self.pos },
                        Some("did you forget a closing quote?".into()), Severity::Error);
                    return Tok::Str(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let esc = self.peek();
                    self.pos += 1;
                    match esc {
                        Some(b'"') => out.push('"'),
                        Some(b'\'') => out.push('\''),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'b') => { saw_control_escape = true; out.push('\u{0008}') }
                        Some(b'f') => { saw_control_escape = true; out.push('\u{000C}') }
                        Some(b'n') => { saw_control_escape = true; out.push('\n') }
                        Some(b'r') => { saw_control_escape = true; out.push('\r') }
                        Some(b't') => { saw_control_escape = true; out.push('\t') }
                        Some(b'u') => {
                            let cp = self.lex_hex4();
                            match cp {
                                Some(hi @ 0xD800..=0xDBFF) => {
                                    // expect low surrogate
                                    if self.peek() == Some(b'\\') && self.src.get(self.pos + 1) == Some(&b'u') {
                                        self.pos += 2;
                                        match self.lex_hex4() {
                                            Some(lo @ 0xDC00..=0xDFFF) => {
                                                let c = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                                                out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
                                            }
                                            Some(other) => {
                                                // two lone surrogates or surrogate+scalar: JSON.parse
                                                // accepts; Rust strings can't hold lone surrogates,
                                                // so we substitute U+FFFD (documented divergence).
                                                self.surrogate_diag();
                                                out.push('\u{FFFD}');
                                                out.push(char::from_u32(other).unwrap_or('\u{FFFD}'));
                                            }
                                            None => {
                                                self.diag("E014", "invalid \\u escape".into(),
                                                    Span { start: self.pos.saturating_sub(2), end: self.pos },
                                                    Some("expected four hex digits, e.g. \\u00e9".into()), Severity::Error);
                                                out.push('\u{FFFD}');
                                            }
                                        }
                                    } else {
                                        self.surrogate_diag();
                                        out.push('\u{FFFD}');
                                    }
                                }
                                Some(cp) => {
                                    if (0xDC00..=0xDFFF).contains(&cp) {
                                        self.surrogate_diag();
                                    }
                                    out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                                }
                                None => {
                                    self.diag("E014", "invalid \\u escape".into(),
                                        Span { start: self.pos.saturating_sub(2), end: self.pos },
                                        Some("expected four hex digits, e.g. \\u00e9".into()), Severity::Error);
                                }
                            }
                        }
                        other => {
                            let hint = if self.is_win_path_string(open) {
                                Some("this looks like a Windows path — double every backslash: C:\\\\Users\\\\...".into())
                            } else { None };
                            self.diag("E015",
                                format!("invalid escape sequence '\\{}'",
                                    other.map(|c| c as char).unwrap_or('?')),
                                Span { start: self.pos.saturating_sub(2), end: self.pos },
                                hint, Severity::Error);
                        }
                    }
                }
                b if b < 0x20 => {
                    self.control_char_diag(b);
                    self.pos += 1;
                }
                _ => {
                    // Copy a contiguous run of plain bytes in one shot (hot path).
                    let run_start = self.pos;
                    while self.pos < self.src.len() {
                        let c = self.src[self.pos];
                        if c == quote || c == b'\\' || c < 0x20 { break; }
                        self.pos += utf8_len(c);
                    }
                    if self.pos > self.src.len() { self.pos = self.src.len(); }
                    let run = &self.src[run_start..self.pos];
                    match std::str::from_utf8(run) {
                        Ok(t) => out.push_str(t),
                        Err(e) => {
                            self.utf8_diag(run_start + e.valid_up_to(), run[e.valid_up_to()]);
                            out.push_str(&String::from_utf8_lossy(run));
                        }
                    }
                }
            }
        }
    }

    /// Validation-only string scan: checks escapes and structure, allocates nothing.
    fn scan_string(&mut self, quote: u8) -> Tok {
        let open = self.pos;
        self.pos += 1;
        let mut saw_control_escape = false;
        loop {
            let b = match self.peek() {
                None => {
                    self.diag("E013", "unterminated string".into(),
                        Span { start: open, end: self.pos }, None, Severity::Error);
                    return Tok::Str(String::new());
                }
                Some(b) => b,
            };
            match b {
                q if q == quote => {
                    self.pos += 1;
                    if saw_control_escape { self.win_path_check(open); }
                    return Tok::Str(String::new());
                }
                b'\n' => {
                    self.diag("E013", "unterminated string (newline reached)".into(),
                        Span { start: open, end: self.pos },
                        Some("did you forget a closing quote?".into()), Severity::Error);
                    return Tok::Str(String::new());
                }
                b'\\' => {
                    self.pos += 1;
                    let esc = self.peek();
                    self.pos += 1;
                    match esc {
                        Some(b'"') | Some(b'\'') | Some(b'\\') | Some(b'/') => {}
                        Some(b'b') | Some(b'f') | Some(b'n') | Some(b'r') | Some(b't') => {
                            saw_control_escape = true;
                        }
                        Some(b'u') => {
                            match self.lex_hex4() {
                                Some(hi @ 0xD800..=0xDBFF) => {
                                    if self.peek() == Some(b'\\') && self.src.get(self.pos + 1) == Some(&b'u') {
                                        self.pos += 2;
                                        match self.lex_hex4() {
                                            Some(0xDC00..=0xDFFF) => {}
                                            Some(_) => { let _ = hi; self.surrogate_diag(); }
                                            None => {
                                                self.diag("E014", "invalid \\u escape".into(),
                                                    Span { start: self.pos.saturating_sub(2), end: self.pos },
                                                    Some("expected four hex digits, e.g. \\u00e9".into()), Severity::Error);
                                            }
                                        }
                                    } else {
                                        self.surrogate_diag();
                                    }
                                }
                                Some(cp) => {
                                    if (0xDC00..=0xDFFF).contains(&cp) { self.surrogate_diag(); }
                                }
                                None => {
                                    self.diag("E014", "invalid \\u escape".into(),
                                        Span { start: self.pos.saturating_sub(2), end: self.pos },
                                        Some("expected four hex digits, e.g. \\u00e9".into()), Severity::Error);
                                }
                            }
                        }
                        other => {
                            let hint = if self.is_win_path_string(open) {
                                Some("this looks like a Windows path — double every backslash: C:\\\\Users\\\\...".into())
                            } else { None };
                            self.diag("E015",
                                format!("invalid escape sequence '\\{}'",
                                    other.map(|c| c as char).unwrap_or('?')),
                                Span { start: self.pos.saturating_sub(2), end: self.pos },
                                hint, Severity::Error);
                        }
                    }
                }
                b if b < 0x20 => {
                    self.control_char_diag(b);
                    self.pos += 1;
                }
                _ => {
                    // burn through the plain run
                    let run_start = self.pos;
                    while self.pos < self.src.len() {
                        let c = self.src[self.pos];
                        if c == quote || c == b'\\' || c < 0x20 { break; }
                        self.pos += 1;
                    }
                    if let Err(e) = std::str::from_utf8(&self.src[run_start..self.pos]) {
                        self.utf8_diag(run_start + e.valid_up_to(), self.src[run_start + e.valid_up_to()]);
                    }
                }
            }
        }
    }

    /// Invalid UTF-8 inside a string (Seldaek#52 class: pasted Latin-1).
    fn utf8_diag(&mut self, at: usize, byte: u8) {
        self.diag("E027", format!("invalid UTF-8 byte (0x{:02X}) in string", byte),
            Span { start: at, end: at + 1 },
            Some("this often means Latin-1/Windows-1252 text was pasted in — re-encode the input as UTF-8".into()),
            Severity::Error);
    }

    /// circlecell#7 class: "C:\temp" is valid JSON where \t silently becomes TAB.
    fn win_path_check(&mut self, open: usize) {
        let s = self.src;
        if open + 3 < s.len() && s[open + 1].is_ascii_alphabetic() && s[open + 2] == b':' {
            self.diag("W051", "control escape in what looks like a Windows path".into(),
                Span { start: open, end: self.pos },
                Some("\\t here is a TAB character, not a backslash + t — double every backslash: C:\\\\temp".into()),
                Severity::Warning);
        }
    }

    fn is_win_path_string(&self, open: usize) -> bool {
        let s = self.src;
        open + 3 < s.len() && s[open + 1].is_ascii_alphabetic() && s[open + 2] == b':' && s[open + 3] == b'\\'
    }

    fn control_char_diag(&mut self, b: u8) {
        let (name, esc) = match b {
            b'\t' => ("tab character", "\\t"),
            0x0B => ("vertical tab", "\\u000b"),
            0x0C => ("form feed", "\\f"),
            _ => ("control character", "\\u0000-style escape"),
        };
        self.diag("E016", format!("unescaped {} in string (0x{:02X})", name, b),
            Span { start: self.pos, end: self.pos + 1 },
            Some(format!("replace it with {}", esc)), Severity::Error);
    }

    fn surrogate_diag(&mut self) {
        self.diag("W017", "lone surrogate in \\u escape".into(),
            Span { start: self.pos.saturating_sub(6), end: self.pos },
            Some("JSON.parse accepts this, but the string will contain an unpaired surrogate — many systems will corrupt or reject it".into()),
            Severity::Warning);
    }

    fn lex_hex4(&mut self) -> Option<u32> {
        if self.pos + 4 > self.src.len() { return None; }
        let s = std::str::from_utf8(&self.src[self.pos..self.pos + 4]).ok()?;
        let v = u32::from_str_radix(s, 16).ok()?;
        self.pos += 4;
        Some(v)
    }

    fn lex_number(&mut self) -> Tok {
        let start = self.pos;
        if self.peek() == Some(b'-') { self.pos += 1; }
        // integer part; catch leading zeros
        let int_start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) { self.pos += 1; }
        let int_len = self.pos - int_start;
        if int_len > 1 && self.src[int_start] == b'0' {
            self.diag("E018", "numbers may not have leading zeros".into(),
                Span { start, end: self.pos }, None, Severity::Error);
        }
        if int_len == 0 {
            self.diag("E019", "expected digits in number".into(),
                Span { start, end: self.pos + 1 }, None, Severity::Error);
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            let frac_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) { self.pos += 1; }
            if self.pos == frac_start {
                self.diag("E019", "expected digit after decimal point".into(),
                    Span { start, end: self.pos }, None, Severity::Error);
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) { self.pos += 1; }
            let exp_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) { self.pos += 1; }
            if self.pos == exp_start {
                self.diag("E019", "expected digit in exponent".into(),
                    Span { start, end: self.pos }, None, Severity::Error);
            }
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let n = text.parse::<f64>().unwrap_or(f64::NAN);
        // Integer wider than 2^53: silently corrupts through IEEE-754 doubles
        // (the classic "pasted a tweet/Discord ID" bug).
        if int_len > 15 && !text.contains('.') && !text.contains('e') && !text.contains('E') {
            if let Ok(i) = text.parse::<i128>() {
                if i.unsigned_abs() > (1u128 << 53) && (n as i128) != i {
                    self.diag("W050",
                        format!("integer exceeds JavaScript's safe range and loses precision ({} becomes {:.0})", text, n),
                        Span { start, end: self.pos },
                        Some("store large IDs as strings, or parse with a lossless-number option".into()),
                        Severity::Warning);
                }
            }
        }
        Tok::Num(n)
    }

    fn lex_ident(&mut self) -> Tok {
        let start = self.pos;
        while matches!(self.peek(), Some(b'A'..=b'Z') | Some(b'a'..=b'z') | Some(b'0'..=b'9') | Some(b'_') | Some(b'$')) {
            self.pos += 1;
        }
        let word = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("");
        match word {
            "true" => Tok::True,
            "false" => Tok::False,
            "null" => Tok::Null,
            "True" | "False" => {
                self.diag("E030", format!("'{}' is not valid JSON", word),
                    Span { start, end: self.pos },
                    Some(format!("this looks like a Python literal — use '{}'", word.to_lowercase())),
                    Severity::Error);
                if word == "True" { Tok::True } else { Tok::False }
            }
            "None" => {
                self.diag("E030", "'None' is not valid JSON".into(),
                    Span { start, end: self.pos },
                    Some("this looks like a Python literal — use 'null'".into()), Severity::Error);
                Tok::Null
            }
            "NaN" | "Infinity" => {
                self.diag("E031", format!("'{}' is not valid JSON", word),
                    Span { start, end: self.pos },
                    Some("valid in JSON5 only; JSON has no representation for it".into()),
                    Severity::Error);
                Tok::Num(if word == "NaN" { f64::NAN } else { f64::INFINITY })
            }
            "undefined" => {
                self.diag("E030", "'undefined' is not valid JSON".into(),
                    Span { start, end: self.pos },
                    Some("use 'null'".into()), Severity::Error);
                Tok::Null
            }
            _ => Tok::Ident(word.to_string()),
        }
    }
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 { 1 } else if b < 0xE0 { 2 } else if b < 0xF0 { 3 } else { 4 }
}

// --------------------------------------------------------------------- parser

pub struct ParseOptions {
    pub mode: Mode,
    pub max_depth: usize,
    pub duplicate_keys: DupKeys,
}

impl Default for ParseOptions {
    fn default() -> Self {
        ParseOptions { mode: Mode::Strict, max_depth: 512, duplicate_keys: DupKeys::Warn }
    }
}

pub struct ParseResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl ParseResult {
    pub fn ok(&self) -> bool {
        self.diagnostics.iter().all(|d| d.severity != Severity::Error)
    }
}

pub fn parse_into<S: Sink>(src: &[u8], opts: &ParseOptions, sink: &mut S) -> ParseResult {
    let mut p = Parser {
        lexer: {
            let mut lx = Lexer::new(src, opts.mode);
            lx.keep_strings = sink.wants_strings();
            lx
        },
        tok: Token { tok: Tok::Eof, span: Span { start: 0, end: 0 } },
        opts,
        depth: 0,
        extra: Vec::new(),
    };
    p.bump();
    p.value(sink);
    // Anything after the top-level value?
    if p.tok.tok != Tok::Eof {
        let hint = if matches!(p.tok.tok, Tok::LBrace | Tok::LBracket) {
            Some("looks like two JSON documents concatenated — did you mean an array, or NDJSON?".into())
        } else {
            None
        };
        let span = p.tok.span;
        p.diag("E002", "unexpected content after top-level value".into(), span, hint);
    }
    let mut diagnostics = p.lexer.diagnostics;
    diagnostics.append(&mut p.extra);
    diagnostics.sort_by_key(|d| d.span.start);
    diagnostics.truncate(MAX_DIAGNOSTICS);
    ParseResult { diagnostics }
}

/// Convenience: full DOM parse.
pub fn parse(src: &[u8], opts: &ParseOptions) -> (Option<Value>, ParseResult) {
    let mut sink = TreeSink::new();
    let res = parse_into(src, opts, &mut sink);
    (sink.root, res)
}

/// Convenience: validation only (fastest path).
pub fn validate(src: &[u8], opts: &ParseOptions) -> ParseResult {
    parse_into(src, opts, &mut NullSink)
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    tok: Token,
    opts: &'a ParseOptions,
    depth: usize,
    extra: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn bump(&mut self) {
        self.tok = self.lexer.next_token();
    }

    fn diag(&mut self, code: &'static str, msg: String, span: Span, hint: Option<String>) {
        if self.extra.len() < MAX_DIAGNOSTICS {
            self.extra.push(Diagnostic { code, message: msg, span, hint, severity: Severity::Error, related: None });
        }
    }

    fn value<S: Sink>(&mut self, sink: &mut S) {
        if self.depth >= self.opts.max_depth {
            let span = self.tok.span;
            self.diag("E040", "maximum nesting depth exceeded".into(), span, None);
            self.resync();
            sink.null();
            return;
        }
        match self.tok.tok.clone() {
            Tok::LBrace => self.object(sink),
            Tok::LBracket => self.array(sink),
            Tok::Str(s) => { sink.string(&s); self.bump(); }
            Tok::Num(n) => { sink.number(n); self.bump(); }
            Tok::True => { sink.boolean(true); self.bump(); }
            Tok::False => { sink.boolean(false); self.bump(); }
            Tok::Null => { sink.null(); self.bump(); }
            Tok::Ident(w) => {
                let span = self.tok.span;
                self.diag("E003", format!("unexpected identifier '{}'", w), span,
                    Some("bare words are not values — did you mean a quoted string?".into()));
                sink.string(&w);
                self.bump();
            }
            Tok::Eof => {
                let span = self.tok.span;
                self.diag("E004", "unexpected end of input, expected a value".into(), span, None);
                sink.null();
            }
            other => {
                let span = self.tok.span;
                self.diag("E005", format!("expected a value, found {}", tok_name(&other)), span, None);
                sink.null();
                self.bump();
            }
        }
    }

    fn object<S: Sink>(&mut self, sink: &mut S) {
        sink.begin_object();
        self.depth += 1;
        self.bump(); // {
        let mut first = true;
        let mut seen_keys: Vec<Span> = Vec::new();
        loop {
            match self.tok.tok.clone() {
                Tok::RBrace => {
                    self.bump();
                    break;
                }
                Tok::Eof => {
                    let span = self.tok.span;
                    self.diag("E006", "unclosed object, expected '}'".into(), span, None);
                    break;
                }
                Tok::Comma if first => {
                    let span = self.tok.span;
                    self.diag("E007", "leading comma in object".into(), span, None);
                    self.bump();
                    continue;
                }
                _ => {}
            }
            if !first {
                if self.tok.tok == Tok::Comma {
                    self.bump();
                    if self.tok.tok == Tok::RBrace {
                        if self.opts.mode == Mode::Strict {
                            let span = self.tok.span;
                            self.diag("E008", "trailing comma in object".into(), span,
                                Some("trailing commas are valid in JSONC — enable mode: \"jsonc\" if this is a config file".into()));
                        }
                        self.bump();
                        break;
                    }
                } else if self.tok.tok == Tok::RBrace {
                    self.bump();
                    break;
                } else {
                    let span = self.tok.span;
                    let found = tok_name(&self.tok.tok);
                    self.diag("E009", format!("expected ',' or '}}' between object members, found {}", found), span,
                        Some("did you forget a comma after the previous value?".into()));
                    // fall through and try to parse a member anyway (recovery)
                }
            }
            first = false;
            // key
            match self.tok.tok.clone() {
                Tok::Str(k) => {
                    let kspan = self.tok.span;
                    self.check_duplicate(&kspan, &mut seen_keys);
                    sink.key(&k);
                    self.bump();
                }
                Tok::Ident(k) => {
                    let span = self.tok.span;
                    self.diag("E022", format!("object keys must be quoted: '{}'", k), span,
                        Some(format!("write \"{}\" — unquoted keys are valid in JSON5 only", k)));
                    sink.key(&k);
                    self.bump();
                }
                Tok::Num(n) => {
                    let span = self.tok.span;
                    self.diag("E022", "object keys must be quoted strings".into(), span,
                        Some("numbers cannot be keys in JSON".into()));
                    sink.key(&format!("{}", n));
                    self.bump();
                }
                _ => {
                    let span = self.tok.span;
                    self.diag("E023", format!("expected object key, found {}", tok_name(&self.tok.tok)), span, None);
                    self.resync_member();
                    if self.tok.tok == Tok::RBrace { continue; }
                    if self.tok.tok == Tok::Eof { continue; }
                    continue;
                }
            }
            // colon
            if self.tok.tok == Tok::Colon {
                self.bump();
            } else {
                let span = self.tok.span;
                self.diag("E024", format!("expected ':' after object key, found {}", tok_name(&self.tok.tok)), span, None);
                // recovery: if next token starts a value, assume the colon was forgotten
            }
            self.value(sink);
        }
        self.depth -= 1;
        sink.end_object();
    }

    fn array<S: Sink>(&mut self, sink: &mut S) {
        sink.begin_array();
        self.depth += 1;
        self.bump(); // [
        let mut first = true;
        loop {
            match self.tok.tok {
                Tok::RBracket => {
                    self.bump();
                    break;
                }
                Tok::Eof => {
                    let span = self.tok.span;
                    self.diag("E006", "unclosed array, expected ']'".into(), span, None);
                    break;
                }
                _ => {}
            }
            if !first {
                if self.tok.tok == Tok::Comma {
                    self.bump();
                    if self.tok.tok == Tok::RBracket {
                        if self.opts.mode == Mode::Strict {
                            let span = self.tok.span;
                            self.diag("E008", "trailing comma in array".into(), span,
                                Some("trailing commas are valid in JSONC".into()));
                        }
                        self.bump();
                        break;
                    }
                } else if self.tok.tok == Tok::RBracket {
                    self.bump();
                    break;
                } else {
                    let span = self.tok.span;
                    self.diag("E009", format!("expected ',' or ']' between array elements, found {}", tok_name(&self.tok.tok)), span,
                        Some("did you forget a comma after the previous value?".into()));
                }
            }
            first = false;
            self.value(sink);
        }
        self.depth -= 1;
        sink.end_array();
    }

    /// Duplicate-key check by raw byte comparison of the key token (quotes included).
    /// Escaped-equivalent keys ("a" vs "\u0061") are not detected in v1.
    fn check_duplicate(&mut self, kspan: &Span, seen: &mut Vec<Span>) {
        if self.opts.duplicate_keys == DupKeys::Allow {
            return;
        }
        let src = self.lexer.src;
        let key_bytes = &src[kspan.start..kspan.end.min(src.len())];
        if let Some(prev) = seen.iter().find(|p| {
            &src[p.start..p.end.min(src.len())] == key_bytes
        }) {
            let sev = if self.opts.duplicate_keys == DupKeys::Error { Severity::Error } else { Severity::Warning };
            let name = String::from_utf8_lossy(key_bytes).into_owned();
            if self.extra.len() < MAX_DIAGNOSTICS {
                self.extra.push(Diagnostic {
                    code: "W060",
                    message: format!("duplicate object key {}", name),
                    span: *kspan,
                    hint: Some("the last occurrence wins; earlier values are silently discarded".into()),
                    severity: sev,
                    related: Some(*prev),
                });
            }
        } else {
            seen.push(*kspan);
        }
    }

    /// Skip tokens until a plausible synchronization point at this nesting level.
    fn resync(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.tok.tok {
                Tok::Eof => break,
                Tok::LBrace | Tok::LBracket => { depth += 1; self.bump(); }
                Tok::RBrace | Tok::RBracket => {
                    if depth == 0 { break; }
                    depth -= 1;
                    self.bump();
                }
                Tok::Comma if depth == 0 => break,
                _ => self.bump(),
            }
        }
    }

    fn resync_member(&mut self) {
        loop {
            match self.tok.tok {
                Tok::Eof | Tok::RBrace | Tok::Comma => break,
                _ => self.bump(),
            }
        }
        if self.tok.tok == Tok::Comma { self.bump(); }
    }
}

fn tok_name(t: &Tok) -> String {
    match t {
        Tok::LBrace => "'{'".into(),
        Tok::RBrace => "'}'".into(),
        Tok::LBracket => "'['".into(),
        Tok::RBracket => "']'".into(),
        Tok::Colon => "':'".into(),
        Tok::Comma => "','".into(),
        Tok::Str(_) => "a string".into(),
        Tok::Num(_) => "a number".into(),
        Tok::True | Tok::False => "a boolean".into(),
        Tok::Null => "'null'".into(),
        Tok::Ident(w) => format!("'{}'", w),
        Tok::Eof => "end of input".into(),
    }
}

// ---------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> (Option<Value>, ParseResult) {
        parse(s.as_bytes(), &ParseOptions::default())
    }

    #[test]
    fn happy_path() {
        let (v, r) = p(r#"{"a": [1, 2.5, -3e2], "b": {"c": null, "d": true}, "e": "x\ny"}"#);
        assert!(r.ok(), "{:?}", r.diagnostics);
        match v.unwrap() {
            Value::Object(pairs) => assert_eq!(pairs.len(), 3),
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn unicode_escapes() {
        let (v, r) = p(r#""caf\u00e9 \ud83d\ude00""#);
        assert!(r.ok());
        assert_eq!(v.unwrap(), Value::String("café 😀".into()));
    }

    #[test]
    fn reports_all_errors_in_one_pass() {
        // three distinct problems: trailing comma, single quotes, Python literal
        let (_, r) = p(r#"{"a": 1, "b": 'x', "c": True,}"#);
        let codes: Vec<_> = r.diagnostics.iter().map(|d| d.code).collect();
        assert!(codes.contains(&"E010"), "{:?}", codes); // single quotes
        assert!(codes.contains(&"E030"), "{:?}", codes); // Python True
        assert!(codes.contains(&"E008"), "{:?}", codes); // trailing comma
    }

    #[test]
    fn recovery_still_builds_tree() {
        let (v, r) = p(r#"{"a": 1 "b": 2}"#); // missing comma
        assert!(!r.ok());
        match v.unwrap() {
            Value::Object(pairs) => assert_eq!(pairs.len(), 2), // both members recovered
            _ => panic!(),
        }
    }

    #[test]
    fn jsonc_mode_allows_comments_and_trailing_commas() {
        let opts = ParseOptions { mode: Mode::Jsonc, ..Default::default() };
        let (v, r) = parse(br#"{
            // config-style comment
            "a": 1, /* block */
            "b": [1, 2,],
        }"#, &opts);
        assert!(r.ok(), "{:?}", r.diagnostics);
        assert!(matches!(v, Some(Value::Object(_))));
    }

    #[test]
    fn strict_mode_rejects_comments_with_hint() {
        let (_, r) = p("// hello\n{}");
        assert_eq!(r.diagnostics[0].code, "E021");
        assert!(r.diagnostics[0].hint.as_ref().unwrap().contains("jsonc"));
    }

    #[test]
    fn concatenated_docs_hint() {
        let (_, r) = p(r#"{"a":1}{"b":2}"#);
        let d = r.diagnostics.iter().find(|d| d.code == "E002").unwrap();
        assert!(d.hint.as_ref().unwrap().contains("NDJSON"));
    }

    #[test]
    fn smart_quotes_recovered() {
        let (_, r) = p("{\u{201C}a\u{201D}: 1}");
        assert!(r.diagnostics.iter().any(|d| d.code == "E011"));
    }

    #[test]
    fn depth_limit() {
        let deep = "[".repeat(1000) + &"]".repeat(1000);
        let opts = ParseOptions { max_depth: 64, ..Default::default() };
        let (_, r) = parse(deep.as_bytes(), &opts);
        assert!(r.diagnostics.iter().any(|d| d.code == "E040"));
    }

    #[test]
    fn line_index() {
        let src = b"ab\ncd\nef";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_col(0), (1, 1));
        assert_eq!(idx.line_col(4), (2, 2));
        assert_eq!(idx.line_col(7), (3, 2));
    }

    #[test]
    fn validate_matches_parse_verdict() {
        for s in [r#"{"a":1}"#, r#"{"a":}"#, "[1,2,", "true", "nope"] {
            let full = p(s).1.ok();
            let fast = validate(s.as_bytes(), &ParseOptions::default()).ok();
            assert_eq!(full, fast, "mismatch on {}", s);
        }
    }
}

#[cfg(test)]
mod issue_mined_tests {
    use super::*;

    fn p(s: &str) -> ParseResult {
        validate(s.as_bytes(), &ParseOptions::default())
    }

    #[test]
    fn zaach_13_duplicate_keys_warned_with_first_location() {
        // zaach/jsonlint#13 (2011) and #85: {"a":1,"a":2} passes silently
        let r = p(r#"{"a":1,"a":2}"#);
        assert!(r.ok()); // warning by default, matches JSON.parse acceptance
        let d = r.diagnostics.iter().find(|d| d.code == "W060").expect("dup warning");
        assert_eq!(d.severity, Severity::Warning);
        let rel = d.related.expect("first-occurrence span");
        assert_eq!((rel.start, rel.end), (1, 4)); // first "a"
        assert_eq!((d.span.start, d.span.end), (7, 10)); // second "a"
    }

    #[test]
    fn duplicate_keys_error_policy() {
        let opts = ParseOptions { duplicate_keys: DupKeys::Error, ..Default::default() };
        let r = validate(br#"{"a":1,"a":2}"#, &opts);
        assert!(!r.ok());
    }

    #[test]
    fn duplicate_keys_scoped_per_object() {
        // same key in sibling objects is fine
        let r = p(r#"{"x":{"a":1},"y":{"a":2}}"#);
        assert!(r.diagnostics.iter().all(|d| d.code != "W060"));
    }

    #[test]
    fn prantlf_bom_strict_error_jsonc_warning() {
        let mut doc = vec![0xEF, 0xBB, 0xBF];
        doc.extend_from_slice(br#"{"a":1}"#);
        // strict: matches JSON.parse (rejects), but still validates the rest
        let r = validate(&doc, &ParseOptions::default());
        assert!(!r.ok());
        assert!(r.diagnostics.iter().any(|d| d.code == "E025"));
        assert_eq!(r.diagnostics.len(), 1); // recovery: no cascade
        // jsonc: tolerated with warning
        let opts = ParseOptions { mode: Mode::Jsonc, ..Default::default() };
        let r2 = validate(&doc, &opts);
        assert!(r2.ok());
        assert!(r2.diagnostics.iter().any(|d| d.code == "W001"));
    }

    #[test]
    fn json5_101_crlf_line_numbers() {
        // json5/json5#101: newline definition must include \r\n and \r
        let idx = LineIndex::new(b"a\r\nb\rc\nd");
        assert_eq!(idx.line_col(3), (2, 1)); // 'b'
        assert_eq!(idx.line_col(5), (3, 1)); // 'c'
        assert_eq!(idx.line_col(7), (4, 1)); // 'd'
    }

    #[test]
    fn tweet_id_precision_warning() {
        let r = p(r#"{"id": 1786623058123456789}"#);
        let d = r.diagnostics.iter().find(|d| d.code == "W050").expect("precision warning");
        assert!(d.severity == Severity::Warning);
        assert!(r.ok());
        // safe integers stay silent
        let r2 = p(r#"{"id": 9007199254740991}"#);
        assert!(r2.diagnostics.iter().all(|d| d.code != "W050"));
    }
}

#[cfg(test)]
mod full_issue_sweep_tests {
    use super::*;

    fn p(s: &str) -> ParseResult { validate(s.as_bytes(), &ParseOptions::default()) }

    #[test]
    fn zaach_63_exact_reported_value_warns() {
        let r = p(r#"{"fooId":1111111111258928239}"#);
        let d = r.diagnostics.iter().find(|d| d.code == "W050").unwrap();
        assert!(d.message.contains("1111111111258928239"));
    }

    #[test]
    fn zaach_65_bracket_chars_inside_string_are_fine() {
        assert!(p(r#"{"pos": "[106.675,525.792,47.7364]"}"#).ok());
    }

    #[test]
    fn zaach_142_invalid_escape_points_at_escape() {
        let src = r#"{"": "foobar\?"}"#;
        let r = p(src);
        let d = r.diagnostics.iter().find(|d| d.code == "E015").unwrap();
        assert!(d.message.contains("\\?"));
        assert_eq!(&src.as_bytes()[d.span.start..d.span.end], b"\\?");
    }

    #[test]
    fn zaach_24_tab_named_in_message() {
        let r = p("{\"action\": \"log\tin\"}");
        let d = r.diagnostics.iter().find(|d| d.code == "E016").unwrap();
        assert!(d.message.contains("tab"), "{}", d.message);
    }

    #[test]
    fn zaach_37_90_escape_decoding_correct() {
        // "\\n" is backslash + n (two chars), "\n" is a newline, "\/" is "/"
        let (v, r) = parse(br#"["\\n", "\n", "\/"]"#, &ParseOptions::default());
        assert!(r.ok());
        assert_eq!(v.unwrap(), Value::Array(vec![
            Value::String("\\n".into()),
            Value::String("\n".into()),
            Value::String("/".into()),
        ]));
    }

    #[test]
    fn zaach_47_113_raw_unicode_and_solidus_valid() {
        // raw non-ASCII and unescaped '/' are valid JSON; user misconception, not a bug
        assert!(p("{\"q\": \"\u{201C}curly\u{201D} and a / slash\"}").ok());
    }

    #[test]
    fn prantlf_15_utf16_detected_cleanly() {
        // UTF-16LE with BOM: FF FE then '{ ' as 16-bit units
        let doc: Vec<u8> = vec![0xFF, 0xFE, b'{', 0, b'}', 0];
        let r = validate(&doc, &ParseOptions::default());
        assert!(!r.ok());
        assert!(r.diagnostics.iter().any(|d| d.code == "E026"));
        // one clear error + EOF notice at most, not a garbage cascade
        assert!(r.diagnostics.len() <= 2, "{:?}", r.diagnostics);
        // UTF-16LE without BOM (null-interleaved ASCII)
        let doc2: Vec<u8> = vec![b'{', 0, b'}', 0];
        assert!(validate(&doc2, &ParseOptions::default())
            .diagnostics.iter().any(|d| d.code == "E026"));
    }

    #[test]
    fn prantlf_23_zaach_80_prototype_keys_safe() {
        let r = p(r#"{"constructor": 1, "hasOwnProperty": 2, "__proto__": 3}"#);
        // no false duplicate positives, parses clean
        assert!(r.diagnostics.iter().all(|d| d.code != "W060"));
        assert!(r.ok());
        // but real duplicates of those names ARE caught
        let r2 = p(r#"{"constructor": 1, "constructor": 2}"#);
        assert!(r2.diagnostics.iter().any(|d| d.code == "W060"));
    }
}

#[cfg(test)]
mod hardening_tests {
    use super::*;

    #[test]
    fn seldaek_52_latin1_rejected_at_first_byte() {
        let doc = [0x7Bu8, 0x22, 0x61, 0x22, 0x3A, 0x22, 0x63, 0x61, 0x66, 0xE9, 0x22, 0x7D];
        let r = validate(&doc, &ParseOptions::default());
        assert!(!r.ok());
        let d = r.diagnostics.iter().find(|d| d.code == "E027").unwrap();
        assert_eq!(d.span.start, 9);
    }

    #[test]
    fn junk_flood_no_stack_overflow() {
        let junk = vec![0x01u8; 1_000_000];
        let r = validate(&junk, &ParseOptions::default());
        assert!(r.diagnostics.len() <= MAX_DIAGNOSTICS);
    }

    #[test]
    fn empty_and_whitespace_single_clean_error() {
        for s in ["", "   \n\t "] {
            let r = validate(s.as_bytes(), &ParseOptions::default());
            assert!(!r.ok());
            assert_eq!(r.diagnostics.len(), 1);
            assert_eq!(r.diagnostics[0].code, "E004");
        }
    }

    #[test]
    fn seldaek_escaped_quote_apostrophe_decode() {
        let (v, r) = parse(br#""\u0022\u0027""#, &ParseOptions::default());
        assert!(r.ok());
        assert_eq!(v.unwrap(), Value::String("\"'".into()));
    }
}

#[cfg(test)]
mod issue_dump_tests {
    use super::*;

    fn p(s: &str) -> ParseResult { validate(s.as_bytes(), &ParseOptions::default()) }

    #[test]
    fn json5_192_lone_surrogate_json_parse_parity() {
        // JSON.parse accepts "\uDEAD"; we accept with W017 warning
        let r = p(r#""\uDEAD""#);
        assert!(r.ok(), "{:?}", r.diagnostics);
        assert!(r.diagnostics.iter().any(|d| d.code == "W017"));
        // high surrogate alone, and hi+hi, also accepted with warning
        assert!(p(r#""\uD800""#).ok());
        assert!(p(r#""\uD800\uD800""#).ok());
        // malformed hex after \u stays a hard error
        assert!(!p(r#""\uD800\u""#).ok());
    }

    #[test]
    fn circlecell_12_sibling_objects_no_false_duplicates() {
        let doc = r#"[
            {"name": "(HT)", "location_size": 6, "img": "a.png"},
            {"name": "(STT)", "location_size": 6, "img": "b.png"}
        ]"#;
        let r = p(doc);
        assert!(r.ok());
        assert!(r.diagnostics.iter().all(|d| d.code != "W060"));
    }

    #[test]
    fn circlecell_11_tab_indentation_valid() {
        assert!(p("{\n\t\"a\": 1,\n\t\"b\": [\n\t\t2\n\t]\n}").ok());
    }

    #[test]
    fn circlecell_7_windows_paths() {
        // properly doubled: valid, no warnings
        let r = p(r#"{"sourcePath": "C:\\temp\\"}"#);
        assert!(r.ok() && r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        // single backslash before t: valid JSON, silent TAB corruption -> W051
        let r2 = p("{\"p\": \"C:\\temp\"}");
        assert!(r2.ok());
        assert!(r2.diagnostics.iter().any(|d| d.code == "W051"));
        // invalid escape in a path gets the targeted hint
        let r3 = p(r#"{"p": "C:\Users"}"#);
        let d = r3.diagnostics.iter().find(|d| d.code == "E015").unwrap();
        assert!(d.hint.as_ref().unwrap().contains("Windows path"));
        // tab escape in a non-path string stays silent
        assert!(p(r#"{"cell": "a\tb"}"#).diagnostics.is_empty());
    }

    #[test]
    fn seldaek_54_bad_backslash_exact_location() {
        let doc = r#"{"k": "TYPO3\PharStreamWrapper\Exception"}"#;
        let r = p(doc);
        let d = r.diagnostics.iter().find(|d| d.code == "E015").unwrap();
        assert_eq!(&doc.as_bytes()[d.span.start..d.span.end], b"\\P");
    }
}
