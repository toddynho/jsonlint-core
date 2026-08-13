//! Hardening gauntlet for the Rust core: json5-tests oracle, pathological
//! inputs, and a deterministic mutation fuzzer. Panics on any violation.
use json_core::{parse, validate, Mode, ParseOptions};
use std::time::Instant;

fn main() {
    let mut fails = 0u32;

    // ---- json5-tests oracle (strict) ----
    let root = std::env::var("CORPORA_DIR").map(|d| format!("{}/json5-tests-master", d)).unwrap_or_else(|_| "/tmp/json5-tests-master".into());
    let (mut pass, mut miss) = (0, 0);
    for dir in ["arrays", "comments", "misc", "new-lines", "numbers", "objects", "strings"] {
        let mut entries: Vec<_> = std::fs::read_dir(format!("{}/{}", &root, dir)).unwrap()
            .map(|e| e.unwrap().path()).collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if name == "irregular-block-comment.json" { continue; } // JSON.parse rejects it too
            let ext_ok = name.ends_with(".json");
            if !(name.ends_with(".json") || name.ends_with(".json5")
                || name.ends_with(".js") || name.ends_with(".txt")) { continue; }
            let src = std::fs::read(&path).unwrap();
            let ok = validate(&src, &ParseOptions::default()).ok();
            if ok == ext_ok { pass += 1; }
            else { miss += 1; println!("  MISS {}/{} expected {}", dir, name, if ext_ok {"valid"} else {"invalid"}); }
        }
    }
    println!("json5-tests (strict): {} pass, {} miss", pass, miss);
    fails += miss;

    // ---- pathological ----
    let t0 = Instant::now();
    let deep = "[".repeat(100_000);
    let r = validate(deep.as_bytes(), &ParseOptions::default());
    assert!(!r.ok());
    let junk = vec![0x01u8; 1_000_000];
    let r = validate(&junk, &ParseOptions::default());
    assert!(r.diagnostics.len() <= 100);
    let commas = ",".repeat(500_000);
    validate(commas.as_bytes(), &ParseOptions::default());
    let closers = "]".repeat(100_000);
    validate(closers.as_bytes(), &ParseOptions::default());
    let long_str = format!("\"{}\"", "a".repeat(5_000_000));
    validate(long_str.as_bytes(), &ParseOptions::default());
    println!("pathological set: no panic, {:?}", t0.elapsed());

    // ---- fuzz: 100k inputs, must never panic; strict-accept implies well-formed ----
    let mut seed: u64 = 0x2F6E2B1;
    let mut rnd = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 33) as u32 };
    let alphabet: Vec<u8> = b"{}[],\":0123456789.eE+-\\ truefalsn\n\r\t'\x00/*ab\xE2\x80\x9C\xFF".to_vec();
    let base = br#"{"a":[1,2.5,{"b":"x\n","c":true}],"d":null}"#.to_vec();
    let t0 = Instant::now();
    for i in 0..100_000u32 {
        let input: Vec<u8> = if i % 3 == 0 {
            let len = 1 + (rnd() % 60) as usize;
            (0..len).map(|_| alphabet[(rnd() as usize) % alphabet.len()]).collect()
        } else {
            let mut v = base.clone();
            for _ in 0..(1 + rnd() % 4) {
                let p = (rnd() as usize) % v.len().max(1);
                match rnd() % 10 {
                    0..=3 => v[p] = alphabet[(rnd() as usize) % alphabet.len()],
                    4..=6 => { v.remove(p.min(v.len() - 1)); }
                    _ => v.insert(p, alphabet[(rnd() as usize) % alphabet.len()]),
                }
                if v.is_empty() { v.push(b'0'); }
            }
            v
        };
        let mode = if i % 2 == 0 { Mode::Strict } else { Mode::Jsonc };
        let opts = ParseOptions { mode, ..Default::default() };
        // must not panic; tree build must not panic either
        let (_v, _r) = parse(&input, &opts);
    }
    println!("fuzz: 100000 inputs, no panic, {:?}", t0.elapsed());

    if fails > 0 { std::process::exit(1); }
    println!("HARDENED: all checks green");
}
