//! Standalone preview renderer for the visual iteration loop:
//! builds a theme from an image (same pipeline as the API) and writes
//! preview.png so the agent can screenshot & critique.

use std::collections::BTreeMap;

use themezzz::colors::{extract_palette, resolve_theme, HashMapSections, SectionChoice};
use themezzz::preview::{render_preview, PreviewArgs};
use themezzz::state::{EditorState, COLOR_SECTIONS};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let img_path = args.get(1).map(|s| s.as_str()).unwrap_or("testimg.jpg");
    let out_dir = args.get(2).map(|s| s.as_str()).unwrap_or("/tmp/pv");
    let mode = args.get(3).map(|s| s.as_str()).unwrap_or("dark");
    let blur = args.get(4).map(|s| s == "blur").unwrap_or(false);

    let raw = std::fs::read(img_path).expect("input image");
    let palette = extract_palette(&raw, 6);
    println!("palette: {:?}", palette);

    // Fresh section choices, exactly like a brand-new editor session.
    let mut sections: HashMapSections = COLOR_SECTIONS
        .iter()
        .map(|k| (k.to_string(), SectionChoice::default()))
        .collect();
    for (k, v) in sections.iter_mut() {
        v.alpha = if k == "in" {
            30
        } else if k == "out" {
            70
        } else {
            0
        };
    }
    let st = EditorState {
        palette,
        mode: mode.to_string(),
        sections,
        active: "bg".into(),
        wall_mode: "image".into(),
        wall_idx: -1,
        wall_custom: None,
        wall_blur: blur,
        wall_bytes: None,
        msg_id: None,
        mode_img: "photo".into(),
    };
    let res = resolve_theme(&st.palette, &st.sections, &st.mode);
    let alphas: BTreeMap<String, u32> = st
        .sections
        .iter()
        .map(|(k, v)| (k.clone(), v.alpha))
        .collect();

    std::fs::create_dir_all(out_dir).unwrap();
    let png = render_preview(&PreviewArgs {
        colors: &res,
        alphas: &alphas,
        wall_bytes: Some(raw.as_slice()),
        wall_flat: None,
        blur,
    });
    std::fs::write(format!("{out_dir}/preview.png"), &png).unwrap();
    println!("wrote {out_dir}/preview.png ({} bytes)", png.len());
}
