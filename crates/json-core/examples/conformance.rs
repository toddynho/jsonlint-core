//! JSONTestSuite conformance: y_ files MUST be accepted, n_ MUST be rejected, i_ is free.
//! Run: cargo run --release --example conformance -- /tmp/JSONTestSuite-master/test_parsing
use json_core::{validate, ParseOptions};

fn main() {
    let dir = std::env::args().nth(1).expect("usage: conformance <dir>");
    let (mut y_pass, mut y_fail, mut n_pass, mut n_fail, mut i_count) = (0, 0, 0, 0, 0);
    let mut failures: Vec<String> = Vec::new();

    let mut entries: Vec<_> = std::fs::read_dir(&dir).unwrap()
        .map(|e| e.unwrap().path()).collect();
    entries.sort();

    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".json") { continue; }
        let src = match std::fs::read(&path) { Ok(s) => s, Err(_) => continue };
        let ok = validate(&src, &ParseOptions::default()).ok();
        if name.starts_with("y_") {
            if ok { y_pass += 1; } else { y_fail += 1; failures.push(format!("REJECTED valid:  {}", name)); }
        } else if name.starts_with("n_") {
            if !ok { n_pass += 1; } else { n_fail += 1; failures.push(format!("ACCEPTED invalid: {}", name)); }
        } else {
            i_count += 1; // implementation-defined, either verdict is conformant
        }
    }

    println!("y_ (must accept): {} pass, {} fail", y_pass, y_fail);
    println!("n_ (must reject): {} pass, {} fail", n_pass, n_fail);
    println!("i_ (either):      {} files", i_count);
    for f in failures.iter().take(40) { println!("  {}", f); }
    if y_fail + n_fail > 0 { std::process::exit(1); }
}
