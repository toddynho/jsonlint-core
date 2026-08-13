//! WASM-facing C ABI. No wasm-bindgen: tiny binary, zero deps, no install scripts.
//! Protocol: JS writes UTF-8 into a buffer from `jc_alloc`, calls `jc_validate`,
//! reads back a length-prefixed JSON diagnostics report, then frees both buffers.
use crate::{validate, LineIndex, Mode, ParseOptions, Severity};

#[no_mangle]
pub extern "C" fn jc_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr` must come from `jc_alloc(len)` or a jc_validate result with its prefixed length.
#[no_mangle]
pub unsafe extern "C" fn jc_dealloc(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(Vec::from_raw_parts(ptr, 0, len));
    }
}

/// Validate `len` bytes at `ptr`. mode: 0 = strict, 1 = jsonc.
/// Returns a pointer to [u32 LE byte-length][JSON report bytes]. Never null.
///
/// # Safety
/// `ptr..ptr+len` must be readable.
#[no_mangle]
pub unsafe extern "C" fn jc_validate(ptr: *const u8, len: usize, mode: u32) -> *mut u8 {
    let src = std::slice::from_raw_parts(ptr, len);
    let opts = ParseOptions {
        mode: if mode == 1 { Mode::Jsonc } else { Mode::Strict },
        ..Default::default()
    };
    let result = validate(src, &opts);
    let report = report_json(src, &result.diagnostics, result.ok());
    pack(report)
}

fn pack(s: String) -> *mut u8 {
    let bytes = s.into_bytes();
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
    let ptr = out.as_mut_ptr();
    std::mem::forget(out);
    ptr
}

/// Hand-rolled JSON serialization of the diagnostics report (zero deps).
pub fn report_json(src: &[u8], diags: &[crate::Diagnostic], ok: bool) -> String {
    let idx = LineIndex::new(src);
    let mut out = String::with_capacity(128 + diags.len() * 160);
    out.push_str("{\"ok\":");
    out.push_str(if ok { "true" } else { "false" });
    out.push_str(",\"diagnostics\":[");
    for (i, d) in diags.iter().enumerate() {
        if i > 0 { out.push(','); }
        let (line, col) = idx.line_col(d.span.start);
        out.push_str("{\"code\":\"");
        out.push_str(d.code);
        out.push_str("\",\"message\":");
        esc(&mut out, &d.message);
        out.push_str(",\"severity\":\"");
        out.push_str(match d.severity { Severity::Error => "error", Severity::Warning => "warning" });
        out.push_str("\",\"start\":");
        out.push_str(&d.span.start.to_string());
        out.push_str(",\"end\":");
        out.push_str(&d.span.end.to_string());
        out.push_str(",\"line\":");
        out.push_str(&line.to_string());
        out.push_str(",\"column\":");
        out.push_str(&col.to_string());
        if let Some(h) = &d.hint {
            out.push_str(",\"hint\":");
            esc(&mut out, h);
        }
        if let Some(r) = &d.related {
            let (rl, rc) = idx.line_col(r.start);
            out.push_str(",\"related\":{\"start\":");
            out.push_str(&r.start.to_string());
            out.push_str(",\"end\":");
            out.push_str(&r.end.to_string());
            out.push_str(",\"line\":");
            out.push_str(&rl.to_string());
            out.push_str(",\"column\":");
            out.push_str(&rc.to_string());
            out.push('}');
        }
        out.push('}');
    }
    out.push_str("]}");
    out
}

fn esc(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParseOptions;

    #[test]
    fn report_is_valid_json_and_roundtrips() {
        let src = br#"{"a": 'bad', "b": True}"#;
        let r = validate(src, &ParseOptions::default());
        let report = report_json(src, &r.diagnostics, r.ok());
        // our own parser should accept our own report
        let (v, check) = crate::parse(report.as_bytes(), &ParseOptions::default());
        assert!(check.ok(), "report not valid JSON: {}", report);
        match v.unwrap() {
            crate::Value::Object(pairs) => {
                assert_eq!(pairs[0].0, "ok");
                assert_eq!(pairs[0].1, crate::Value::Bool(false));
            }
            _ => panic!(),
        }
    }
}
