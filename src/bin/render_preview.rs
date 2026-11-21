//! Render example previews (dark & light, wallpaper & flat) to PNG for the
//! repo's screenshots and for visual verification.

use std::collections::BTreeMap;
use themezzz::colors::{resolve_theme, resolve_wall, HashMapSections, SectionChoice};
use themezzz::preview::{render_preview, PreviewArgs};
use themezzz::state::COLOR_SECTIONS;
fn sections_default() -> HashMapSections {
    COLOR_SECTIONS.iter().map(|k| (k.to_string(), SectionChoice::default())).collect()
}

fn main() {
    // A warm, colorful palette like a real photo would give.
    let pal = vec![
        "#1f2a44".to_string(), "#e2574c".to_string(), "#3d9970".to_string(),
        "#f0c674".to_string(), "#6b8cae".to_string(), "#b5838d".to_string(),
    ];
    let alphas: BTreeMap<String, u32> = [
        ("bar", 20u32), ("in", 10), ("out", 10), ("text", 0), ("accent", 0), ("reply", 0),
    ].iter().map(|(k, v)| (k.to_string(), *v)).collect();

    let mut out_dir = std::path::PathBuf::from("assets");
    std::fs::create_dir_all(&out_dir).unwrap();

    for mode in ["dark", "light"] {
        let secs = sections_default();
        let res = resolve_theme(&pal, &secs, mode);
        let wall_flat = Some(resolve_wall(&pal, -1, None, mode));
        let png = render_preview(&PreviewArgs {
            colors: &res, alphas: &alphas, wall_bytes: None, wall_flat, blur: false,
        });
        let p = out_dir.join(format!("preview_{}.png", mode));
        std::fs::write(&p, &png).unwrap();
        println!("wrote {} ({} bytes)", p.display(), png.len());
    }
    println!("OK");
}
