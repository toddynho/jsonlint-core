//! Run the zaach + prantlf legacy test corpora against json-core.
//! fails/*.json must produce >=1 error; passes/*.json must be clean valid JSON.
use json_core::{validate, Mode, ParseOptions, Severity};

fn run_dir(dir: &str, expect_fail: bool, mode: Mode) -> (u32, u32, Vec<String>) {
    let (mut pass, mut fail) = (0, 0);
    let mut misses = Vec::new();
    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.map(|e| e.unwrap().path()).collect(),
        Err(_) => return (0, 0, misses),
    };
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".json") { continue; }
        let src = std::fs::read(&path).unwrap();
        let r = validate(&src, &ParseOptions { mode, ..Default::default() });
        let has_error = r.diagnostics.iter().any(|d| d.severity == Severity::Error);
        let correct = if expect_fail { has_error } else { !has_error };
        if correct { pass += 1; } else {
            fail += 1;
            misses.push(format!("{} ({})", name,
                String::from_utf8_lossy(&src[..src.len().min(60)]).replace('\n', " ")));
        }
    }
    (pass, fail, misses)
}

fn main() {
    for (label, dir, expect_fail) in [
        ("zaach fails", "corpus/zaach-fails", true),
        ("zaach passes", "corpus/zaach-passes", false),
        ("prantlf fails", "corpus/prantlf-fails", true),
        ("prantlf passes", "corpus/prantlf-passes", false),
    ] {
        let (pass, fail, misses) = run_dir(dir, expect_fail, Mode::Strict);
        println!("{:<16} {:>3} pass, {:>2} miss", label, pass, fail);
        for m in &misses { println!("    MISS: {}", m); }
    }
}
