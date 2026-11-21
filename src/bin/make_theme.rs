//! Offline .attheme generator (replaces the old smoke_test.py):
//! builds a theme from a built-in palette and writes it to stdout or a file.
//!
//! Usage: cargo run --bin make_theme [-- out.theme.attheme]

use std::collections::BTreeMap;
use themezzz::colors::{build_attheme, resolve_theme, HashMapSections, SectionChoice};
use themezzz::state::COLOR_SECTIONS;

fn main() {
    let pal = vec![
        "#1f2a44".to_string(),
        "#e2574c".to_string(),
        "#3d9970".to_string(),
        "#f0c674".to_string(),
        "#6b8cae".to_string(),
        "#b5838d".to_string(),
    ];
    let secs: HashMapSections = COLOR_SECTIONS
        .iter()
        .map(|k| (k.to_string(), SectionChoice::default()))
        .collect();
    let alphas: BTreeMap<String, u32> = [
        ("bar", 20u32),
        ("in", 10),
        ("out", 10),
        ("text", 0),
        ("accent", 0),
        ("reply", 0),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect();

    let res = resolve_theme(&pal, &secs, "dark");
    let theme = build_attheme(&res, &alphas, None, None, false);
    let n_keys = theme.iter().filter(|&&b| b == b'\n').count();

    let mut args = std::env::args().skip(1);
    if let Some(path) = args.next() {
        std::fs::write(&path, &theme).unwrap();
        println!("wrote {} ({} keys, {} bytes)", path, n_keys, theme.len());
    } else {
        println!("keys: {}, bytes: {}", n_keys, theme.len());
        std::io::Write::write_all(&mut std::io::stdout(), &theme).unwrap();
    }
}
