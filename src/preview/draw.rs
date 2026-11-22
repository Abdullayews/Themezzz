//! Anti-aliased 2D drawing primitives for the preview renderer.
//!
//! Every primitive composites with true source-over blending (translucent
//! bubbles and bars show the wallpaper through, exactly like in Telegram) and
//! uses analytic coverage, so edges stay smooth at any scale.

use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

use crate::colors::Rgb;

/// RGBA colour wrapper (straight alpha).
#[derive(Clone, Copy)]
pub(crate) struct C(pub(crate) Rgba<u8>);

impl From<(Rgb, u8)> for C {
    fn from((c, a): (Rgb, u8)) -> Self {
        C(Rgba([c.r, c.g, c.b, a]))
    }
}

impl C {
    /// Same colour with its alpha raised to at least `min` (keeps small
    /// glyphs readable at any user transparency).
    pub(crate) fn at_least(self, min: u8) -> C {
        let mut p = self.0;
        p[3] = p[3].max(min);
        C(p)
    }

    /// Same colour with its alpha multiplied by `f` (0..1).
    pub(crate) fn faded(self, f: f64) -> C {
        let mut p = self.0;
        p[3] = (f64::from(p[3]) * f.clamp(0.0, 1.0)).round() as u8;
        C(p)
    }
}

#[inline]
fn over(dst: &mut Rgba<u8>, src: &Rgba<u8>, cov: f32) {
    let sa = f32::from(src[3]) / 255.0 * cov;
    if sa <= 0.0 {
        return;
    }
    let da = f32::from(dst[3]) / 255.0;
    let oa = sa + da * (1.0 - sa);
    if oa <= 0.0 {
        return;
    }
    for c in 0..3 {
        let v = (f32::from(src[c]) * sa + f32::from(dst[c]) * da * (1.0 - sa)) / oa;
        dst[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Polylines produced by [`PathBuilder`] (one per sub-path).
pub(crate) type Polys = Vec<Vec<(f64, f64)>>;

/// Minimal path builder that flattens curves to polylines.
#[derive(Default)]
pub(crate) struct PathBuilder {
    done: Polys,
    cur: Vec<(f64, f64)>,
}

impl PathBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn move_to(&mut self, x: f64, y: f64) {
        self.flush();
        self.cur.push((x, y));
    }

    pub(crate) fn line_to(&mut self, x: f64, y: f64) {
        self.cur.push((x, y));
    }

    pub(crate) fn cubic_to(&mut self, c1: (f64, f64), c2: (f64, f64), p: (f64, f64)) {
        let Some(&p0) = self.cur.last() else {
            self.cur.push(p);
            return;
        };
        let len = dist(p0, c1) + dist(c1, c2) + dist(c2, p);
        let steps = (len.sqrt() * 1.6).ceil().clamp(6.0, 48.0) as usize;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let mt = 1.0 - t;
            let x = mt * mt * mt * p0.0
                + 3.0 * mt * mt * t * c1.0
                + 3.0 * mt * t * t * c2.0
                + t * t * t * p.0;
            let y = mt * mt * mt * p0.1
                + 3.0 * mt * mt * t * c1.1
                + 3.0 * mt * t * t * c2.1
                + t * t * t * p.1;
            self.cur.push((x, y));
        }
    }

    pub(crate) fn quad_to(&mut self, c: (f64, f64), p: (f64, f64)) {
        let Some(&p0) = self.cur.last() else {
            self.cur.push(p);
            return;
        };
        self.cubic_to(
            (
                p0.0 + 2.0 / 3.0 * (c.0 - p0.0),
                p0.1 + 2.0 / 3.0 * (c.1 - p0.1),
            ),
            (p.0 + 2.0 / 3.0 * (c.0 - p.0), p.1 + 2.0 / 3.0 * (c.1 - p.1)),
            p,
        );
    }

    /// Close the current sub-path (the fill rule closes implicitly).
    pub(crate) fn close(&mut self) {
        self.flush();
    }

    fn flush(&mut self) {
        if self.cur.len() > 1 {
            self.done.push(std::mem::take(&mut self.cur));
        } else {
            self.cur.clear();
        }
    }

    pub(crate) fn finish(mut self) -> Polys {
        self.flush();
        self.done
    }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Parse SVG path data (M/L/H/V/C/S/Q/Z, absolute and relative) into
/// polylines. `map` converts a point from the icon's own space to pixels.
pub(crate) fn svg_path(data: &str, map: impl Fn(f64, f64) -> (f64, f64)) -> Polys {
    let b: Vec<char> = data.chars().collect();
    let mut i = 0usize;
    let num = |i: &mut usize| -> Option<f64> {
        while *i < b.len() && (b[*i] == ' ' || b[*i] == ',' || b[*i] == '\n') {
            *i += 1;
        }
        let start = *i;
        if *i < b.len() && (b[*i] == '-' || b[*i] == '+') {
            *i += 1;
        }
        let mut dot = false;
        while *i < b.len() && (b[*i].is_ascii_digit() || (b[*i] == '.' && !dot)) {
            if b[*i] == '.' {
                dot = true;
            }
            *i += 1;
        }
        if *i == start {
            return None;
        }
        b[start..*i].iter().collect::<String>().parse().ok()
    };
    let mut pb = PathBuilder::new();
    let (mut cx, mut cy) = (0.0f64, 0.0f64);
    let (mut sx, mut sy) = (0.0f64, 0.0f64);
    let mut last_c2: Option<(f64, f64)> = None;
    let mut cmd = ' ';
    while i < b.len() {
        while i < b.len() && (b[i] == ' ' || b[i] == ',' || b[i] == '\n') {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i].is_ascii_alphabetic() {
            cmd = b[i];
            i += 1;
            if cmd == 'z' || cmd == 'Z' {
                pb.line_to(map(sx, sy).0, map(sx, sy).1);
                pb.close();
                cx = sx;
                cy = sy;
                last_c2 = None;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };
        let mut keep_c2 = false;
        match cmd.to_ascii_uppercase() {
            'M' => {
                let (Some(x), Some(y)) = (num(&mut i), num(&mut i)) else {
                    break;
                };
                cx = ox + x;
                cy = oy + y;
                sx = cx;
                sy = cy;
                let p = map(cx, cy);
                pb.move_to(p.0, p.1);
                // further coordinate pairs after M are implicit L
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => {
                let (Some(x), Some(y)) = (num(&mut i), num(&mut i)) else {
                    break;
                };
                cx = ox + x;
                cy = oy + y;
                let p = map(cx, cy);
                pb.line_to(p.0, p.1);
            }
            'H' => {
                let Some(x) = num(&mut i) else { break };
                cx = ox + x;
                let p = map(cx, cy);
                pb.line_to(p.0, p.1);
            }
            'V' => {
                let Some(y) = num(&mut i) else { break };
                cy = oy + y;
                let p = map(cx, cy);
                pb.line_to(p.0, p.1);
            }
            'C' => {
                let mut v = [0.0f64; 6];
                for item in &mut v {
                    let Some(n) = num(&mut i) else {
                        return pb.finish();
                    };
                    *item = n;
                }
                let c1 = (ox + v[0], oy + v[1]);
                let c2 = (ox + v[2], oy + v[3]);
                let p = (ox + v[4], oy + v[5]);
                pb.cubic_to(map(c1.0, c1.1), map(c2.0, c2.1), map(p.0, p.1));
                last_c2 = Some(c2);
                keep_c2 = true;
                cx = p.0;
                cy = p.1;
            }
            'S' => {
                let mut v = [0.0f64; 4];
                for item in &mut v {
                    let Some(n) = num(&mut i) else {
                        return pb.finish();
                    };
                    *item = n;
                }
                let c1 = match last_c2 {
                    Some(l) => (2.0 * cx - l.0, 2.0 * cy - l.1),
                    None => (cx, cy),
                };
                let c2 = (ox + v[0], oy + v[1]);
                let p = (ox + v[2], oy + v[3]);
                pb.cubic_to(map(c1.0, c1.1), map(c2.0, c2.1), map(p.0, p.1));
                last_c2 = Some(c2);
                keep_c2 = true;
                cx = p.0;
                cy = p.1;
            }
            'Q' => {
                let mut v = [0.0f64; 4];
                for item in &mut v {
                    let Some(n) = num(&mut i) else {
                        return pb.finish();
                    };
                    *item = n;
                }
                let c = (ox + v[0], oy + v[1]);
                let p = (ox + v[2], oy + v[3]);
                pb.quad_to(map(c.0, c.1), map(p.0, p.1));
                cx = p.0;
                cy = p.1;
            }
            _ => break,
        }
        if !keep_c2 {
            last_c2 = None;
        }
    }
    pb.finish()
}

/// Drawing surface. The field is public inside the crate on purpose:
/// `store::render_thumb` builds one directly.
pub(crate) struct D<'a> {
    pub(crate) img: &'a mut RgbaImage,
}

impl D<'_> {
    /// Source-over blend of one pixel with fractional coverage.
    #[inline]
    pub(crate) fn blend(&mut self, x: i64, y: i64, col: C, cov: f32) {
        if x < 0 || y < 0 || cov <= 0.0 {
            return;
        }
        if let Some(p) = self.img.get_pixel_mut_checked(x as u32, y as u32) {
            over(p, &col.0, cov.min(1.0));
        }
    }

    /// Axis-aligned rectangle with exact fractional edge coverage.
    pub(crate) fn rectf(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, col: C) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        for y in y0.floor() as i64..y1.ceil() as i64 {
            let cy = ((y + 1) as f64).min(y1) - (y as f64).max(y0);
            for x in x0.floor() as i64..x1.ceil() as i64 {
                let cx = ((x + 1) as f64).min(x1) - (x as f64).max(x0);
                self.blend(x, y, col, (cx * cy) as f32);
            }
        }
    }

    /// Fill a region described by a signed distance function (negative inside).
    fn sdf_fill(
        &mut self,
        bx0: f64,
        by0: f64,
        bx1: f64,
        by1: f64,
        col: C,
        f: impl Fn(f64, f64) -> f64,
    ) {
        for y in (by0.floor() as i64 - 1)..=(by1.ceil() as i64 + 1) {
            for x in (bx0.floor() as i64 - 1)..=(bx1.ceil() as i64 + 1) {
                let d = f(x as f64 + 0.5, y as f64 + 0.5);
                let cov = (0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend(x, y, col, cov as f32);
                }
            }
        }
    }

    pub(crate) fn ellipse(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, col: C) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
        let m = rx.min(ry);
        self.sdf_fill(x0, y0, x1, y1, col, |px, py| {
            let (nx, ny) = ((px - cx) / rx, (py - cy) / ry);
            ((nx * nx + ny * ny).sqrt() - 1.0) * m
        });
    }

    pub(crate) fn circle(&mut self, cx: f64, cy: f64, r: f64, col: C) {
        self.ellipse(cx - r, cy - r, cx + r, cy + r, col);
    }

    /// Filled rounded rectangle.
    pub(crate) fn round_rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, radius: f64, col: C) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (hx, hy) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
        let r = radius.min(hx).min(hy).max(0.0);
        self.sdf_fill(x0, y0, x1, y1, col, |px, py| {
            let (qx, qy) = ((px - cx).abs() - (hx - r), (py - cy).abs() - (hy - r));
            qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
        });
    }

    /// Fill polylines with the non-zero rule and exact horizontal / 4x
    /// vertical coverage.
    pub(crate) fn fill_polys(&mut self, polys: &[Vec<(f64, f64)>], col: C) {
        const SUB: usize = 4;
        let pts = polys.iter().flatten();
        let (mut minx, mut maxx, mut miny, mut maxy) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in pts {
            minx = minx.min(p.0);
            maxx = maxx.max(p.0);
            miny = miny.min(p.1);
            maxy = maxy.max(p.1);
        }
        if minx > maxx {
            return;
        }
        let (ix0, ix1) = (minx.floor() as i64, maxx.ceil() as i64);
        let w = (ix1 - ix0 + 1) as usize;
        let mut acc = vec![0f32; w];
        let mut xs: Vec<(f64, i32)> = Vec::new();
        for y in miny.floor() as i64..=maxy.ceil() as i64 {
            acc.iter_mut().for_each(|v| *v = 0.0);
            for s in 0..SUB {
                let sy = y as f64 + (s as f64 + 0.5) / SUB as f64;
                xs.clear();
                for poly in polys {
                    let n = poly.len();
                    for i in 0..n {
                        let (xa, ya) = poly[i];
                        let (xb, yb) = poly[(i + 1) % n];
                        if (ya <= sy && yb > sy) || (yb <= sy && ya > sy) {
                            let t = (sy - ya) / (yb - ya);
                            xs.push((xa + t * (xb - xa), if yb > ya { 1 } else { -1 }));
                        }
                    }
                }
                xs.sort_by(|a, b| a.0.total_cmp(&b.0));
                let (mut wind, mut start) = (0i32, 0.0f64);
                for &(x, dir) in &xs {
                    let prev = wind;
                    wind += dir;
                    if prev == 0 && wind != 0 {
                        start = x;
                    } else if prev != 0 && wind == 0 {
                        add_span(&mut acc, ix0, start, x, 1.0 / SUB as f32);
                    }
                }
            }
            for (i, v) in acc.iter().enumerate() {
                if *v > 0.0 {
                    self.blend(ix0 + i as i64, y, col, *v);
                }
            }
        }
    }

    /// Material-style icon: `data` lives in a 24x24 box, drawn with its
    /// centre at (cx, cy) and edge length `size` pixels.
    pub(crate) fn icon(&mut self, data: &str, cx: f64, cy: f64, size: f64, col: C) {
        let k = size / 24.0;
        let polys = svg_path(data, |x, y| (cx + (x - 12.0) * k, cy + (y - 12.0) * k));
        self.fill_polys(&polys, col);
    }

    /// Vertical two-colour gradient disc (Telegram avatars).
    pub(crate) fn circle_vgrad(&mut self, cx: f64, cy: f64, r: f64, top: Rgb, bottom: Rgb, a: u8) {
        for y in (cy - r - 1.0).floor() as i64..=(cy + r + 1.0).ceil() as i64 {
            let t = ((y as f64 + 0.5 - (cy - r)) / (2.0 * r)).clamp(0.0, 1.0);
            let col = crate::colors::mix(top, bottom, t);
            for x in (cx - r - 1.0).floor() as i64..=(cx + r + 1.0).ceil() as i64 {
                let d = (x as f64 + 0.5 - cx).hypot(y as f64 + 0.5 - cy) - r;
                let cov = (0.5 - d).clamp(0.0, 1.0);
                self.blend(x, y, (col, a).into(), cov as f32);
            }
        }
    }
}

fn add_span(acc: &mut [f32], ix0: i64, xa: f64, xb: f64, wgt: f32) {
    if xb <= xa {
        return;
    }
    let last = acc.len() as i64 - 1;
    let ia = xa.floor() as i64;
    let ib = xb.floor() as i64;
    let at = |i: i64| (i - ix0).clamp(0, last) as usize;
    if ia == ib {
        acc[at(ia)] += (xb - xa) as f32 * wgt;
        return;
    }
    acc[at(ia)] += ((ia + 1) as f64 - xa) as f32 * wgt;
    for k in ia + 1..ib {
        acc[at(k)] += wgt;
    }
    acc[at(ib)] += (xb - ib as f64) as f32 * wgt;
}

// ---------- Image helpers (kept from the original renderer) ----------

/// Source-over composite of `src` onto `img` at (dx, dy).
pub(crate) fn paste_rgba(img: &mut RgbaImage, src: &RgbaImage, dx: i64, dy: i64) {
    for (x, y, p) in src.enumerate_pixels() {
        let (tx, ty) = (dx + i64::from(x), dy + i64::from(y));
        if tx < 0 || ty < 0 {
            continue;
        }
        if let Some(dst) = img.get_pixel_mut_checked(tx as u32, ty as u32) {
            over(dst, p, 1.0);
        }
    }
}

/// Center-crop image to fully cover w x h maintaining aspect ratio.
pub(crate) fn cover_rgba(img: &DynamicImage, w: u32, h: u32) -> RgbaImage {
    let (iw, ih) = img.dimensions();
    let scale = (f64::from(w) / f64::from(iw).max(1.0)).max(f64::from(h) / f64::from(ih).max(1.0));
    let (nw, nh) = (
        (f64::from(iw) * scale) as u32 + 1,
        (f64::from(ih) * scale) as u32 + 1,
    );
    let resized = img.resize_exact(nw.max(1), nh.max(1), image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let (x, y) = ((nw - w) / 2, (nh - h) / 2);
    image::imageops::crop_imm(
        &rgba,
        x.min(nw.saturating_sub(w)),
        y.min(nh.saturating_sub(h)),
        w.min(nw),
        h.min(nh),
    )
    .to_image()
}

/// Public alias of the internal Gaussian blur for thumbnails.
pub(crate) fn blur_rgba(img: &RgbaImage, radius: f64) -> RgbaImage {
    blur_fast(img, radius)
}

/// Gaussian blur that works on a downscaled copy for large radii: visually
/// identical for soft blurs, but orders of magnitude cheaper (matters on the
/// free Render plan).
pub(crate) fn blur_fast(img: &RgbaImage, sigma: f64) -> RgbaImage {
    let f = (sigma / 3.0).floor().max(1.0) as u32;
    if f <= 1 {
        return gaussian_blur_rgba(img, sigma);
    }
    let (w, h) = img.dimensions();
    let small = image::imageops::resize(
        img,
        (w / f).max(1),
        (h / f).max(1),
        image::imageops::FilterType::Triangle,
    );
    let blurred = gaussian_blur_rgba(&small, sigma / f64::from(f));
    image::imageops::resize(&blurred, w, h, image::imageops::FilterType::Triangle)
}

/// Gaussian blur on RGBA (std dev = radius), separable.
pub(crate) fn gaussian_blur_rgba(img: &RgbaImage, radius: f64) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return img.clone();
    }
    let sigma = radius.max(0.1);
    let k_len = (sigma * 3.0).ceil() as usize * 2 + 1;
    let mid = k_len / 2;
    let mut kernel = vec![0f64; k_len];
    let mut sum = 0f64;
    for (i, k) in kernel.iter_mut().enumerate() {
        let d = i as f64 - mid as f64;
        let v = (-0.5 * d * d / (sigma * sigma)).exp();
        *k = v;
        sum += v;
    }
    for k in kernel.iter_mut() {
        *k /= sum;
    }
    let sample = |im: &RgbaImage, x: i64, y: i64| -> [f64; 4] {
        let (cx, cy) = (x.clamp(0, i64::from(w) - 1), y.clamp(0, i64::from(h) - 1));
        let p = im.get_pixel(cx as u32, cy as u32);
        [
            f64::from(p[0]),
            f64::from(p[1]),
            f64::from(p[2]),
            f64::from(p[3]),
        ]
    };
    let mut tmp = RgbaImage::new(w, h);
    for y in 0..i64::from(h) {
        for x in 0..i64::from(w) {
            let mut acc = [0f64; 4];
            for (i, k) in kernel.iter().enumerate() {
                let s = sample(img, x + i as i64 - mid as i64, y);
                for c in 0..4 {
                    acc[c] += s[c] * k;
                }
            }
            tmp.put_pixel(
                x as u32,
                y as u32,
                Rgba(acc.map(|v| v.round().clamp(0.0, 255.0) as u8)),
            );
        }
    }
    let mut out = RgbaImage::new(w, h);
    for y in 0..i64::from(h) {
        for x in 0..i64::from(w) {
            let mut acc = [0f64; 4];
            for (i, k) in kernel.iter().enumerate() {
                let s = sample(&tmp, x, y + i as i64 - mid as i64);
                for c in 0..4 {
                    acc[c] += s[c] * k;
                }
            }
            out.put_pixel(
                x as u32,
                y as u32,
                Rgba(acc.map(|v| v.round().clamp(0.0, 255.0) as u8)),
            );
        }
    }
    out
}

/// Multiply the alpha channel by an anti-aliased rounded-rect mask.
pub(crate) fn round_corners(img: &mut RgbaImage, radius: f64) {
    let (w, h) = (f64::from(img.width()), f64::from(img.height()));
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = radius.min(cx).min(cy);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let (qx, qy) = (
            (f64::from(x) + 0.5 - cx).abs() - (cx - r),
            (f64::from(y) + 0.5 - cy).abs() - (cy - r),
        );
        let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
        let cov = (0.5 - d).clamp(0.0, 1.0);
        p[3] = (f64::from(p[3]) * cov).round() as u8;
    }
}
