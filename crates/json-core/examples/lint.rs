//! Pretty-printing linter demo: `cargo run --example lint -- file.json [--jsonc]`
use json_core::{validate, LineIndex, Mode, ParseOptions, Severity};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: lint <file> [--jsonc]");
    let mode = if args.iter().any(|a| a == "--jsonc") { Mode::Jsonc } else { Mode::Strict };
    let src = std::fs::read(path).expect("read failed");

    let t0 = std::time::Instant::now();
    let result = validate(&src, &ParseOptions { mode, ..Default::default() });
    let elapsed = t0.elapsed();

    let idx = LineIndex::new(&src);
    let lines: Vec<&[u8]> = src.split(|b| *b == b'\n').collect();

    if result.diagnostics.is_empty() {
        println!("✓ valid ({} bytes in {:?})", src.len(), elapsed);
        return;
    }

    for d in &result.diagnostics {
        let (line, col) = idx.line_col(d.span.start);
        let sev = match d.severity { Severity::Error => "error", Severity::Warning => "warning" };
        println!("{}[{}]: {} (line {}, col {})", sev, d.code, d.message, line, col);
        if let Some(text) = lines.get(line - 1) {
            let text = String::from_utf8_lossy(text);
            let display: String = text.trim_end_matches('\r').chars().take(120).collect();
            println!("  {}", display);
            let caret_pos = col.saturating_sub(1).min(119);
            println!("  {}^", " ".repeat(caret_pos));
        }
        if let Some(r) = &d.related {
            let (rl, rc) = idx.line_col(r.start);
            println!("  first occurrence at line {}, col {}", rl, rc);
        }
        if let Some(h) = &d.hint {
            println!("  hint: {}", h);
        }
        println!();
    }
    let errs = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).count();
    println!("✗ {} error(s), {} warning(s) — {} bytes in {:?}",
        errs, result.diagnostics.len() - errs, src.len(), elapsed);
    std::process::exit(1);
}
