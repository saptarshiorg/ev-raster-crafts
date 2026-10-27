//! EV Raster Crafts by EVORA - native Paint-style editor (egui), Windows 11 Paint look.
#![cfg_attr(windows, windows_subsystem = "windows")]
use eframe::egui::{self, Color32, Key, Pos2, Rect, Vec2};

type P = (i32, i32);
const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLUE: Color32 = Color32::from_rgb(0, 120, 212);
const PALETTE: [u32; 20] = [
    0x000000, 0x7f7f7f, 0x880015, 0xed1c24, 0xff7f27, 0xfff200, 0x22b14c, 0x00a2e8, 0x3f48cc, 0xa349a4,
    0xffffff, 0xc3c3c3, 0xb97a57, 0xffaec9, 0xffc90e, 0xefe4b0, 0xb5e61d, 0x99d9ea, 0x7092be, 0xc8bfe7,
];

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Pencil, Brush, Calligraphy, Airbrush, Crayon, Marker, Eraser, Fill, Picker, Magnifier, Text,
    Line, Rect, RRect, Ellipse, Select,
}
const BRUSHES: [(Tool, &str); 5] = [
    (Tool::Brush, "Brush"), (Tool::Calligraphy, "Calligraphy pen"), (Tool::Airbrush, "Airbrush"),
    (Tool::Crayon, "Crayon"), (Tool::Marker, "Marker"),
];
fn is_brush(t: Tool) -> bool { BRUSHES.iter().any(|b| b.0 == t) }
fn is_freehand(t: Tool) -> bool {
    matches!(t, Tool::Pencil | Tool::Brush | Tool::Calligraphy | Tool::Crayon | Tool::Marker | Tool::Eraser)
}

#[derive(Clone)]
struct Img { w: i32, h: i32, d: Vec<u8> }

impl Img {
    fn new(w: i32, h: i32, c: [u8; 4]) -> Img {
        let mut d = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) { d.extend_from_slice(&c); }
        Img { w, h, d }
    }
    fn inb(&self, x: i32, y: i32) -> bool { x >= 0 && y >= 0 && x < self.w && y < self.h }
    fn get(&self, x: i32, y: i32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [self.d[i], self.d[i + 1], self.d[i + 2], self.d[i + 3]]
    }
    fn set(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if self.inb(x, y) { let i = ((y * self.w + x) * 4) as usize; self.d[i..i + 4].copy_from_slice(&c); }
    }
    fn disc(&mut self, cx: i32, cy: i32, r: i32, c: [u8; 4]) {
        for dy in -r..=r { for dx in -r..=r { if dx * dx + dy * dy <= r * r + r { self.set(cx + dx, cy + dy, c); } } }
    }
    fn square(&mut self, cx: i32, cy: i32, s: i32, c: [u8; 4]) {
        let half = s / 2;
        for dy in -half..(s - half) { for dx in -half..(s - half) { self.set(cx + dx, cy + dy, c); } }
    }
    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
        for yy in y..y + h { for xx in x..x + w { self.set(xx, yy, c); } }
    }
    fn crop(&self, x: i32, y: i32, w: i32, h: i32) -> Img {
        let mut n = Img::new(w, h, [0, 0, 0, 0]);
        for yy in 0..h { for xx in 0..w { if self.inb(x + xx, y + yy) { n.set(xx, yy, self.get(x + xx, y + yy)); } } }
        n
    }
    fn blit(&mut self, s: &Img, x: i32, y: i32) {
        for yy in 0..s.h { for xx in 0..s.w { self.set(x + xx, y + yy, s.get(xx, yy)); } }
    }
    fn flood(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if !self.inb(x, y) { return; }
        let t = self.get(x, y);
        if t == c { return; }
        let mut st = vec![(x, y)];
        while let Some((px, py)) = st.pop() {
            if !self.inb(px, py) || self.get(px, py) != t { continue; }
            self.set(px, py, c);
            st.push((px + 1, py)); st.push((px - 1, py)); st.push((px, py + 1)); st.push((px, py - 1));
        }
    }
    fn rot_cw(&self) -> Img {
        let mut n = Img::new(self.h, self.w, [0; 4]);
        for y in 0..self.h { for x in 0..self.w { n.set(self.h - 1 - y, x, self.get(x, y)); } }
        n
    }
    fn rot_ccw(&self) -> Img {
        let mut n = Img::new(self.h, self.w, [0; 4]);
        for y in 0..self.h { for x in 0..self.w { n.set(y, self.w - 1 - x, self.get(x, y)); } }
        n
    }
    fn flip_h(&self) -> Img {
        let mut n = Img::new(self.w, self.h, [0; 4]);
        for y in 0..self.h { for x in 0..self.w { n.set(self.w - 1 - x, y, self.get(x, y)); } }
        n
    }
    fn flip_v(&self) -> Img {
        let mut n = Img::new(self.w, self.h, [0; 4]);
        for y in 0..self.h { for x in 0..self.w { n.set(x, self.h - 1 - y, self.get(x, y)); } }
        n
    }
    fn inverted(&self) -> Img {
        let mut n = self.clone();
        for p in n.d.chunks_mut(4) { p[0] = 255 - p[0]; p[1] = 255 - p[1]; p[2] = 255 - p[2]; }
        n
    }
    fn resized(&self, w: i32, h: i32) -> Img {
        let mut n = Img::new(w, h, WHITE);
        for y in 0..h.min(self.h) { for x in 0..w.min(self.w) { n.set(x, y, self.get(x, y)); } }
        n
    }
    fn stretched(&self, w: i32, h: i32) -> Img {
        let mut n = Img::new(w, h, WHITE);
        for y in 0..h { for x in 0..w { n.set(x, y, self.get(x * self.w / w, y * self.h / h)); } }
        n
    }
}

fn bres(a: P, b: P) -> Vec<P> {
    let (mut x0, mut y0) = a;
    let (x1, y1) = b;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut e = dx + dy;
    let mut v = Vec::new();
    loop {
        v.push((x0, y0));
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * e;
        if e2 >= dy { e += dy; x0 += sx; }
        if e2 <= dx { e += dx; y0 += sy; }
    }
    v
}

fn draw_shape(img: &mut Img, tool: Tool, a: P, b: P, size: i32, stroke: [u8; 4], fill: [u8; 4], mode: u8, shift: bool) {
    let (mut x1, mut y1) = b;
    let (x0r, y0r) = a;
    if shift {
        if tool == Tool::Line {
            let ang = (((y1 - y0r) as f32).atan2((x1 - x0r) as f32) / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
            let len = (((x1 - x0r) as f32).hypot((y1 - y0r) as f32)).round();
            x1 = x0r + (ang.cos() * len).round() as i32;
            y1 = y0r + (ang.sin() * len).round() as i32;
        } else {
            let m = (x1 - x0r).abs().max((y1 - y0r).abs());
            x1 = x0r + if x1 < x0r { -m } else { m };
            y1 = y0r + if y1 < y0r { -m } else { m };
        }
    }
    if tool == Tool::Line {
        for (px, py) in bres((x0r, y0r), (x1, y1)) { img.disc(px, py, size / 2, stroke); }
        return;
    }
    let (x0, x1) = (x0r.min(x1), x0r.max(x1));
    let (y0, y1) = (y0r.min(y1), y0r.max(y1));
    let inside = |x: i32, y: i32, inset: i32| -> bool {
        let (ax, ay, bx, by) = (x0 + inset, y0 + inset, x1 - inset, y1 - inset);
        if bx < ax || by < ay { return false; }
        if tool == Tool::Ellipse {
            let rx = (bx - ax + 1) as f32 / 2.0;
            let ry = (by - ay + 1) as f32 / 2.0;
            let cx = ax as f32 + rx - 0.5;
            let cy = ay as f32 + ry - 0.5;
            let dx = (x as f32 - cx) / rx;
            let dy = (y as f32 - cy) / ry;
            dx * dx + dy * dy <= 1.0
        } else {
            let r = if tool == Tool::RRect { 20i32.min((bx - ax + 1) / 2).min((by - ay + 1) / 2).max(0) } else { 0 };
            let cx = x.max(ax + r).min(bx - r);
            let cy = y.max(ay + r).min(by - r);
            (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r
        }
    };
    for y in y0.max(0)..=y1.min(img.h - 1) {
        for x in x0.max(0)..=x1.min(img.w - 1) {
            if !inside(x, y, 0) { continue; }
            let edge = !inside(x, y, size);
            let col = match mode {
                0 => if edge { Some(stroke) } else { None },
                1 => Some(if edge { stroke } else { fill }),
                _ => Some(stroke),
            };
            if let Some(c) = col { img.set(x, y, c); }
        }
    }
}


fn draw_text(img: &mut Img, font: &[u8], x: i32, y: i32, px: f32, s: &str, c: [u8; 4]) {
    use ab_glyph::{point, Font, FontRef, ScaleFont};
    let f = match FontRef::try_from_slice(font) { Ok(f) => f, Err(_) => return };
    let sc = f.as_scaled(px);
    let mut cx = x as f32;
    let mut cy = y as f32 + sc.ascent();
    for ch in s.chars() {
        if ch == '\n' { cx = x as f32; cy += sc.height() + sc.line_gap(); continue; }
        let id = f.glyph_id(ch);
        let g = id.with_scale_and_position(px, point(cx, cy));
        cx += sc.h_advance(id);
        if let Some(o) = f.outline_glyph(g) {
            let b = o.px_bounds();
            o.draw(|gx, gy, cov| {
                let ix = b.min.x as i32 + gx as i32;
                let iy = b.min.y as i32 + gy as i32;
                if cov > 0.0 && img.inb(ix, iy) {
                    let d = img.get(ix, iy);
                    let a = cov.min(1.0);
                    let mix = |s: u8, t: u8| (s as f32 * a + t as f32 * (1.0 - a)) as u8;
                    img.set(ix, iy, [mix(c[0], d[0]), mix(c[1], d[1]), mix(c[2], d[2]), 255]);
                }
            });
        }
    }
}

struct Sel { x: i32, y: i32, w: i32, h: i32, float: Option<Img> }
struct Drag { start: P, last: P, right: bool, base: Option<Img>, mv: Option<P> }

struct App {
    img: Img, undo: Vec<Img>, redo: Vec<Img>, tool: Tool, prev_tool: Tool,
    primary: Color32, secondary: Color32, active: u8, size: i32, mode: u8, zoom_pct: f32,
    tex: Option<egui::TextureHandle>, drag: Option<Drag>, sel: Option<Sel>, clip: Option<Img>,
    cursor: Option<P>, dialog: Option<bool>, dw: i32, dh: i32, seed: u32, path: Option<std::path::PathBuf>,
    custom: [Option<Color32>; 10], text: Option<(P, String)>, font: Option<Vec<u8>>, last_title: String,
}

fn col(c: Color32) -> [u8; 4] { [c.r(), c.g(), c.b(), 255] }

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = Color32::from_rgb(249, 249, 249);
    v.window_fill = Color32::WHITE;
    v.selection.bg_fill = Color32::from_rgb(204, 228, 247);
    v.selection.stroke = egui::Stroke::new(1.0, BLUE);
    let r = egui::Rounding::same(4.0);
    v.widgets.noninteractive.rounding = r;
    v.widgets.inactive.rounding = r;
    v.widgets.hovered.rounding = r;
    v.widgets.active.rounding = r;
    v.widgets.open.rounding = r;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(229, 241, 251);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(180, 210, 240));
    ctx.set_visuals(v);
    ctx.style_mut(|s| s.spacing.item_spacing = Vec2::new(4.0, 4.0));
}

fn group(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        add(ui);
        ui.add_space(2.0);
        ui.label(egui::RichText::new(label).small().weak());
    });
}

fn swatch(ui: &mut egui::Ui, c: Option<Color32>, r: f32, ring: bool) -> egui::Response {
    let side = r * 2.0 + if ring { 8.0 } else { 3.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::click());
    let ctr = rect.center();
    let p = ui.painter();
    match c {
        Some(c) => { p.circle_filled(ctr, r, c); p.circle_stroke(ctr, r, egui::Stroke::new(1.0, Color32::from_gray(170))); }
        None => { p.circle_stroke(ctr, r, egui::Stroke::new(1.0, Color32::from_gray(190))); }
    }
    if ring { p.circle_stroke(ctr, r + 3.0, egui::Stroke::new(2.0, BLUE)); }
    resp
}

impl App {
    fn new() -> App {
        let font = egui::FontDefinitions::default().font_data.get("Ubuntu-Light").map(|d| d.font.to_vec());
        App {
            img: Img::new(800, 500, WHITE), undo: vec![], redo: vec![], tool: Tool::Pencil, prev_tool: Tool::Pencil,
            primary: Color32::BLACK, secondary: Color32::WHITE, active: 0, size: 3, mode: 0, zoom_pct: 100.0,
            tex: None, drag: None, sel: None, clip: None, cursor: None, dialog: None, dw: 100, dh: 100,
            seed: 2463534242, path: None, custom: [None; 10], text: None, font, last_title: String::new(),
        }
    }
    fn zoom(&self) -> f32 { self.zoom_pct / 100.0 }
    fn zoom_by(&mut self, d: i32) { self.zoom_pct = (self.zoom_pct * 1.25f32.powi(d)).clamp(10.0, 800.0); }
    fn active_color(&self) -> Color32 { if self.active == 0 { self.primary } else { self.secondary } }
    fn set_color(&mut self, c: Color32) { if self.active == 0 { self.primary = c } else { self.secondary = c } }
    fn rnd(&mut self) -> f32 {
        self.seed ^= self.seed << 13; self.seed ^= self.seed >> 17; self.seed ^= self.seed << 5;
        (self.seed % 10000) as f32 / 10000.0
    }
    fn push_undo(&mut self) {
        self.undo.push(self.img.clone());
        if self.undo.len() > 40 { self.undo.remove(0); }
        self.redo.clear();
    }
    fn do_undo(&mut self) {
        self.commit_sel();
        if let Some(p) = self.undo.pop() { let cur = std::mem::replace(&mut self.img, p); self.redo.push(cur); }
    }
    fn do_redo(&mut self) {
        self.commit_sel();
        if let Some(p) = self.redo.pop() { let cur = std::mem::replace(&mut self.img, p); self.undo.push(cur); }
    }
    fn apply<F: FnOnce(&Img) -> Img>(&mut self, f: F) {
        self.commit_sel();
        self.push_undo();
        let n = f(&self.img);
        self.img = n;
    }
    fn new_doc(&mut self) {
        self.img = Img::new(800, 500, WHITE); self.undo.clear(); self.redo.clear(); self.sel = None; self.path = None;
    }
    fn commit_sel(&mut self) {
        if let Some(s) = self.sel.take() { if let Some(f) = s.float { self.img.blit(&f, s.x, s.y); } }
    }
    fn lift(&mut self) {
        let need = matches!(&self.sel, Some(s) if s.float.is_none());
        if !need { return; }
        self.push_undo();
        let (x, y, w, h) = { let s = self.sel.as_ref().unwrap(); (s.x, s.y, s.w, s.h) };
        let f = self.img.crop(x, y, w, h);
        let bgc = col(self.secondary);
        self.img.fill_rect(x, y, w, h, bgc);
        if let Some(s) = self.sel.as_mut() { s.float = Some(f); }
    }
    fn copy_sel(&mut self) {
        if let Some(s) = &self.sel {
            self.clip = Some(match &s.float { Some(f) => f.clone(), None => self.img.crop(s.x, s.y, s.w, s.h) });
        }
    }
    fn del_sel(&mut self) {
        if self.sel.is_none() { return; }
        self.lift();
        self.sel = None;
    }
    fn paste(&mut self) {
        if let Some(c) = self.clip.clone() {
            self.commit_sel();
            self.push_undo();
            self.tool = Tool::Select;
            self.sel = Some(Sel { x: 0, y: 0, w: c.w, h: c.h, float: Some(c) });
        }
    }
    fn select_all(&mut self) {
        self.commit_sel();
        self.tool = Tool::Select;
        self.sel = Some(Sel { x: 0, y: 0, w: self.img.w, h: self.img.h, float: None });
    }
    fn crop_sel(&mut self) {
        let r = self.sel.as_ref().map(|s| (s.x, s.y, s.w, s.h));
        if let Some((x, y, w, h)) = r { self.commit_sel(); self.apply(|i| i.crop(x, y, w, h)); }
    }
    fn open_file(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif"]).pick_file() {
            if let Ok(im) = image::open(&p) {
                let rgba = im.to_rgba8();
                self.commit_sel();
                self.push_undo();
                self.img = Img { w: rgba.width() as i32, h: rgba.height() as i32, d: rgba.into_raw() };
                self.path = Some(p);
            }
        }
    }
    fn write_img(&self, p: &std::path::Path) {
        if let Some(buf) = image::RgbaImage::from_raw(self.img.w as u32, self.img.h as u32, self.img.d.clone()) {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
            let _ = if ext == "jpg" || ext == "jpeg" { image::DynamicImage::ImageRgba8(buf).to_rgb8().save(p) } else { buf.save(p) };
        }
    }
    fn save_file(&mut self, ask: bool) {
        self.commit_sel();
        let target = if ask || self.path.is_none() {
            rfd::FileDialog::new().set_file_name("picture.png")
                .add_filter("PNG", &["png"]).add_filter("JPEG", &["jpg", "jpeg"]).add_filter("BMP", &["bmp"]).save_file()
        } else { self.path.clone() };
        if let Some(mut p) = target {
            if p.extension().is_none() { p.set_extension("png"); }
            self.write_img(&p);
            self.path = Some(p);
        }
    }
    fn stamp(&mut self, q: P, c: [u8; 4]) {
        let s = self.size;
        match self.tool {
            Tool::Pencil => self.img.set(q.0, q.1, c),
            Tool::Brush => self.img.disc(q.0, q.1, s / 2, c),
            Tool::Marker => self.img.square(q.0, q.1, s + 2, c),
            Tool::Calligraphy => {
                for k in -s..=s { self.img.set(q.0 + k, q.1 - k, c); self.img.set(q.0 + k + 1, q.1 - k, c); }
            }
            Tool::Crayon => {
                let r = s + 1;
                for dy in -r..=r { for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r && self.rnd() < 0.45 { self.img.set(q.0 + dx, q.1 + dy, c); }
                } }
            }
            Tool::Eraser => { let bgc = col(self.secondary); self.img.square(q.0, q.1, s + 2, bgc) }
            _ => {}
        }
    }
    fn spray(&mut self, p: P, c: [u8; 4]) {
        for _ in 0..(self.size * 3) {
            let a = self.rnd() * 6.2832;
            let r = self.rnd() * self.size as f32 * 2.0;
            self.img.set(p.0 + (a.cos() * r).round() as i32, p.1 + (a.sin() * r).round() as i32, c);
        }
    }
    fn colors_for(&self, right: bool) -> ([u8; 4], [u8; 4]) {
        if right { (col(self.secondary), col(self.primary)) } else { (col(self.primary), col(self.secondary)) }
    }
    fn begin(&mut self, p: P, right: bool) {
        let (c, _alt) = self.colors_for(right);
        let mut st = Drag { start: p, last: p, right, base: None, mv: None };
        if self.tool == Tool::Select {
            let inside = matches!(&self.sel, Some(s) if p.0 >= s.x && p.1 >= s.y && p.0 < s.x + s.w && p.1 < s.y + s.h);
            if inside {
                let (sx, sy) = { let s = self.sel.as_ref().unwrap(); (s.x, s.y) };
                st.mv = Some((p.0 - sx, p.1 - sy));
                self.lift();
            } else {
                self.commit_sel();
                self.sel = Some(Sel { x: p.0.clamp(0, self.img.w - 1), y: p.1.clamp(0, self.img.h - 1), w: 1, h: 1, float: None });
            }
            self.drag = Some(st);
            return;
        }
        self.commit_sel();
        match self.tool {
            Tool::Magnifier => { self.zoom_by(if right { -1 } else { 1 }); return; }
            Tool::Picker => {
                if self.img.inb(p.0, p.1) {
                    let v = self.img.get(p.0, p.1);
                    let cc = Color32::from_rgb(v[0], v[1], v[2]);
                    if right { self.secondary = cc } else { self.set_color(cc) }
                }
                return;
            }
            Tool::Fill => { self.push_undo(); self.img.flood(p.0, p.1, c); return; }
            Tool::Text => { if self.font.is_some() { self.text = Some((p, String::new())); } return; }
            Tool::Airbrush => { self.push_undo(); self.spray(p, c); }
            t if is_freehand(t) => { self.push_undo(); self.stamp(p, c); }
            _ => { self.push_undo(); st.base = Some(self.img.clone()); }
        }
        self.drag = Some(st);
    }
    fn update_drag(&mut self, p: P, shift: bool) {
        let mut st = match self.drag.take() { Some(s) => s, None => return };
        let (c, alt) = self.colors_for(st.right);
        match self.tool {
            Tool::Airbrush => { self.spray(p, c); st.last = p; }
            t if is_freehand(t) => {
                for q in bres(st.last, p) { self.stamp(q, c); }
                st.last = p;
            }
            Tool::Line | Tool::Rect | Tool::RRect | Tool::Ellipse => {
                if let Some(b) = &st.base {
                    self.img = b.clone();
                    let (t, sz, m) = (self.tool, self.size, self.mode);
                    draw_shape(&mut self.img, t, st.start, p, sz, c, alt, m, shift);
                }
            }
            Tool::Select => {
                let (w, h) = (self.img.w, self.img.h);
                if let Some(s) = self.sel.as_mut() {
                    if let Some(off) = st.mv { s.x = p.0 - off.0; s.y = p.1 - off.1; }
                    else {
                        let (x0, y0) = (st.start.0.clamp(0, w - 1), st.start.1.clamp(0, h - 1));
                        let (x1, y1) = (p.0.clamp(0, w - 1), p.1.clamp(0, h - 1));
                        s.x = x0.min(x1); s.y = y0.min(y1); s.w = (x1 - x0).abs() + 1; s.h = (y1 - y0).abs() + 1;
                    }
                }
            }
            _ => {}
        }
        self.drag = Some(st);
    }
    fn end_drag(&mut self) {
        self.drag = None;
        let tiny = matches!(&self.sel, Some(s) if s.w < 2 && s.h < 2 && s.float.is_none());
        if tiny { self.sel = None; }
    }
    fn refresh(&mut self, ctx: &egui::Context) {
        let ci = {
            let tmp: Img;
            let r: &Img = if let Some(Sel { float: Some(f), x, y, .. }) = &self.sel {
                let mut t = self.img.clone();
                t.blit(f, *x, *y);
                tmp = t;
                &tmp
            } else { &self.img };
            egui::ColorImage::from_rgba_unmultiplied([r.w as usize, r.h as usize], &r.d)
        };
        if let Some(t) = self.tex.as_mut() { t.set(ci, egui::TextureOptions::NEAREST); }
        else { self.tex = Some(ctx.load_texture("canvas", ci, egui::TextureOptions::NEAREST)); }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (copy, cut, paste) = ctx.input(|i| (
            i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
            i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
            i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
        ));
        if copy { self.copy_sel(); }
        if cut { self.copy_sel(); self.del_sel(); }
        if paste { self.paste(); }
        let cmd = egui::Modifiers::COMMAND;
        if ctx.input_mut(|i| i.consume_key(cmd.plus(egui::Modifiers::SHIFT), Key::Z)) { self.do_redo(); }
        if ctx.input_mut(|i| i.consume_key(cmd, Key::Z)) { self.do_undo(); }
        if ctx.input_mut(|i| i.consume_key(cmd, Key::Y)) { self.do_redo(); }
        if ctx.input_mut(|i| i.consume_key(cmd, Key::S)) { self.save_file(false); }
        if ctx.input_mut(|i| i.consume_key(cmd, Key::O)) { self.open_file(); }
        if ctx.input_mut(|i| i.consume_key(cmd, Key::A)) { self.select_all(); }
        if self.text.is_none() {
            if ctx.input(|i| i.key_pressed(Key::Delete)) { self.del_sel(); }
            if ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape)) { self.commit_sel(); }
        }
    }
    fn tool_btn(&mut self, ui: &mut egui::Ui, t: Tool, name: &str) {
        let b = egui::Button::new(name).selected(self.tool == t).min_size(Vec2::new(58.0, 26.0));
        if ui.add(b).clicked() { self.tool = t; }
    }
    fn menu_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New").clicked() { self.new_doc(); ui.close_menu(); }
                if ui.button("Open...  Ctrl+O").clicked() { self.open_file(); ui.close_menu(); }
                if ui.button("Save  Ctrl+S").clicked() { self.save_file(false); ui.close_menu(); }
                if ui.button("Save as...").clicked() { self.save_file(true); ui.close_menu(); }
                if ui.button("Exit").clicked() { std::process::exit(0); }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Zoom in").clicked() { self.zoom_by(1); ui.close_menu(); }
                if ui.button("Zoom out").clicked() { self.zoom_by(-1); ui.close_menu(); }
                if ui.button("Actual size").clicked() { self.zoom_pct = 100.0; ui.close_menu(); }
            });
            ui.separator();
            if ui.button("Save").clicked() { self.save_file(false); }
            if ui.button("Undo").clicked() { self.do_undo(); }
            if ui.button("Redo").clicked() { self.do_redo(); }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("EV Raster Crafts by EVORA").weak());
            });
        });
    }
    fn ribbon(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            group(ui, "Clipboard", |ui| {
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Paste").min_size(Vec2::new(52.0, 56.0))).clicked() { self.paste(); }
                    ui.vertical(|ui| {
                        if ui.button("Cut").clicked() { self.copy_sel(); self.del_sel(); }
                        if ui.button("Copy").clicked() { self.copy_sel(); }
                    });
                });
            });
            ui.separator();
            group(ui, "Image", |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.menu_button("Select v", |ui| {
                            if ui.button("Rectangle select").clicked() { self.tool = Tool::Select; ui.close_menu(); }
                            if ui.button("Select all").clicked() { self.select_all(); ui.close_menu(); }
                            if ui.button("Delete selection").clicked() { self.del_sel(); ui.close_menu(); }
                        });
                        if ui.button("Crop").clicked() { self.crop_sel(); }
                    });
                    ui.vertical(|ui| {
                        if ui.button("Resize").clicked() { self.dw = self.img.w; self.dh = self.img.h; self.dialog = Some(false); }
                        ui.menu_button("Rotate v", |ui| {
                            if ui.button("Rotate right 90").clicked() { self.apply(|i| i.rot_cw()); ui.close_menu(); }
                            if ui.button("Rotate left 90").clicked() { self.apply(|i| i.rot_ccw()); ui.close_menu(); }
                            if ui.button("Rotate 180").clicked() { self.apply(|i| i.rot_cw().rot_cw()); ui.close_menu(); }
                            if ui.button("Flip vertical").clicked() { self.apply(|i| i.flip_v()); ui.close_menu(); }
                            if ui.button("Flip horizontal").clicked() { self.apply(|i| i.flip_h()); ui.close_menu(); }
                            ui.separator();
                            if ui.button("Stretch...").clicked() { self.dw = 100; self.dh = 100; self.dialog = Some(true); ui.close_menu(); }
                            if ui.button("Invert colors").clicked() { self.apply(|i| i.inverted()); ui.close_menu(); }
                            if ui.button("Clear image").clicked() {
                                let bgc = col(self.secondary);
                                self.apply(|i| Img::new(i.w, i.h, bgc));
                                ui.close_menu();
                            }
                        });
                    });
                });
            });
            ui.separator();
            group(ui, "Tools", |ui| {
                ui.horizontal(|ui| {
                    self.tool_btn(ui, Tool::Pencil, "Pencil");
                    self.tool_btn(ui, Tool::Fill, "Fill");
                    self.tool_btn(ui, Tool::Text, "Text");
                });
                ui.horizontal(|ui| {
                    self.tool_btn(ui, Tool::Eraser, "Eraser");
                    self.tool_btn(ui, Tool::Picker, "Picker");
                    self.tool_btn(ui, Tool::Magnifier, "Zoom");
                });
            });
            ui.separator();
            group(ui, "Brushes", |ui| {
                let b = egui::Button::new("Brush").selected(is_brush(self.tool)).min_size(Vec2::new(70.0, 26.0));
                if ui.add(b).clicked() && !is_brush(self.tool) { self.tool = Tool::Brush; }
                ui.menu_button("Brushes v", |ui| {
                    for (t, n) in BRUSHES.iter() {
                        if ui.selectable_label(self.tool == *t, *n).clicked() { self.tool = *t; ui.close_menu(); }
                    }
                });
            });
            ui.separator();
            group(ui, "Shapes", |ui| {
                ui.horizontal(|ui| {
                    self.tool_btn(ui, Tool::Line, "Line");
                    self.tool_btn(ui, Tool::Rect, "Rect");
                });
                ui.horizontal(|ui| {
                    self.tool_btn(ui, Tool::RRect, "Rounded");
                    self.tool_btn(ui, Tool::Ellipse, "Ellipse");
                });
                ui.menu_button("Outline / fill v", |ui| {
                    ui.radio_value(&mut self.mode, 0u8, "Outline");
                    ui.radio_value(&mut self.mode, 1u8, "Outline + fill");
                    ui.radio_value(&mut self.mode, 2u8, "Fill only");
                });
            });
            ui.separator();
            group(ui, "Size", |ui| {
                ui.menu_button(format!("Size {} v", self.size), |ui| {
                    ui.add(egui::Slider::new(&mut self.size, 1..=60));
                    ui.horizontal(|ui| {
                        for n in [1, 3, 5, 8, 16] { if ui.button(n.to_string()).clicked() { self.size = n; } }
                    });
                });
            });
            ui.separator();
            group(ui, "Colors", |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        if swatch(ui, Some(self.primary), 11.0, self.active == 0).on_hover_text("Color 1").clicked() { self.active = 0; }
                        if swatch(ui, Some(self.secondary), 11.0, self.active == 1).on_hover_text("Color 2").clicked() { self.active = 1; }
                    });
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
                        for row in 0..3usize {
                            ui.horizontal(|ui| {
                                for i in 0..10usize {
                                    if row < 2 {
                                        let h = PALETTE[row * 10 + i];
                                        let cc = Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8);
                                        let r = swatch(ui, Some(cc), 7.0, false);
                                        if r.clicked() { self.set_color(cc); }
                                        if r.secondary_clicked() { self.secondary = cc; }
                                    } else {
                                        let cur = self.custom[i];
                                        let r = swatch(ui, cur, 7.0, false);
                                        if r.clicked() {
                                            match cur {
                                                Some(cc) => self.set_color(cc),
                                                None => { let a = self.active_color(); self.custom[i] = Some(a); }
                                            }
                                        }
                                        if r.secondary_clicked() { if let Some(cc) = cur { self.secondary = cc; } }
                                    }
                                }
                            });
                        }
                    });
                    let mut c = self.active_color();
                    if ui.color_edit_button_srgba(&mut c).on_hover_text("Edit colors").changed() { self.set_color(c); }
                });
            });
        });
    }
    fn dialog_ui(&mut self, ctx: &egui::Context) {
        let stretch = match self.dialog { Some(s) => s, None => return };
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new(if stretch { "Stretch (percent)" } else { "Resize canvas (pixels)" })
            .collapsible(false).resizable(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut self.dw).range(1..=8000));
                    ui.label("Height");
                    ui.add(egui::DragValue::new(&mut self.dh).range(1..=8000));
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() { apply = true; }
                    if ui.button("Cancel").clicked() { cancel = true; }
                });
            });
        if apply {
            let (w, h) = if stretch {
                ((self.img.w * self.dw / 100).clamp(1, 8000), (self.img.h * self.dh / 100).clamp(1, 8000))
            } else { (self.dw, self.dh) };
            if stretch { self.apply(|i| i.stretched(w, h)); } else { self.apply(|i| i.resized(w, h)); }
        }
        if apply || cancel { self.dialog = None; }
    }
    fn text_ui(&mut self, ctx: &egui::Context) {
        let (pos, mut s) = match self.text.take() { Some(t) => t, None => return };
        let mut place = false;
        let mut cancel = false;
        egui::Window::new("Text").collapsible(false).resizable(false).show(ctx, |ui| {
            let r = ui.text_edit_singleline(&mut s);
            if !r.has_focus() && !place { r.request_focus(); }
            ui.horizontal(|ui| {
                if ui.button("Place text").clicked() { place = true; }
                if ui.button("Cancel").clicked() { cancel = true; }
            });
        });
        if place && !s.is_empty() {
            self.push_undo();
            let c = col(self.primary);
            let px = (self.size * 3 + 10) as f32;
            if let Some(f) = self.font.clone() { draw_text(&mut self.img, &f, pos.0, pos.1, px, &s, c); }
        } else if !place && !cancel {
            self.text = Some((pos, s));
        }
    }
    fn canvas_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let z = self.zoom();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let size = Vec2::new(self.img.w as f32 * z, self.img.h as f32 * z);
            let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
            let to_img = |p: Pos2| -> P { (((p.x - rect.min.x) / z).floor() as i32, ((p.y - rect.min.y) / z).floor() as i32) };
            let pos = ui.input(|i| i.pointer.latest_pos()).map(to_img);
            self.cursor = if resp.hovered() { pos } else { None };
            let (pp, sp, pd, sd, shift) = ui.input(|i| (
                i.pointer.button_pressed(egui::PointerButton::Primary), i.pointer.button_pressed(egui::PointerButton::Secondary),
                i.pointer.primary_down(), i.pointer.secondary_down(), i.modifiers.shift));
            if self.drag.is_none() {
                if resp.hovered() && (pp || sp) && self.text.is_none() { if let Some(p) = pos { self.begin(p, sp && !pp); } }
            } else {
                if let Some(p) = pos { self.update_drag(p, shift); }
                if !(pd || sd) { self.end_drag(); }
            }
            if self.drag.is_some() { ctx.request_repaint(); }
            let (cmd, scroll) = ui.input(|i| (i.modifiers.command, i.raw_scroll_delta.y));
            if cmd && resp.hovered() && scroll != 0.0 { self.zoom_by(if scroll > 0.0 { 1 } else { -1 }); }
            self.refresh(ctx);
            if let Some(t) = &self.tex {
                ui.painter().image(t.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.0, Color32::from_gray(205)));
            if let Some(s) = &self.sel {
                let r = Rect::from_min_size(
                    Pos2::new(rect.min.x + s.x as f32 * z, rect.min.y + s.y as f32 * z),
                    Vec2::new(s.w as f32 * z, s.h as f32 * z));
                ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.5, BLUE));
            }
            resp.on_hover_cursor(egui::CursorIcon::Crosshair);
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.shortcuts(ctx);
        let name = self.path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("Untitled").to_string();
        let title = format!("{} - EV Raster Crafts by EVORA", name);
        if title != self.last_title { ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone())); self.last_title = title; }
        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_row(ui));
        egui::TopBottomPanel::top("ribbon").show(ctx, |ui| { ui.add_space(4.0); self.ribbon(ui); ui.add_space(2.0); });
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::none().fill(Color32::WHITE).inner_margin(egui::Margin::symmetric(10.0, 4.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let c = self.cursor.map(|p| format!("{}, {}px", p.0, p.1)).unwrap_or_default();
                    ui.add_sized([110.0, 18.0], egui::Label::new(c));
                    ui.separator();
                    let s = self.sel.as_ref().map(|s| format!("{} x {}px", s.w, s.h)).unwrap_or_default();
                    ui.add_sized([110.0, 18.0], egui::Label::new(s));
                    ui.separator();
                    ui.label(format!("{} x {}px", self.img.w, self.img.h));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+").clicked() { self.zoom_by(1); }
                        ui.add(egui::Slider::new(&mut self.zoom_pct, 10.0..=800.0).logarithmic(true).show_value(false));
                        if ui.button("-").clicked() { self.zoom_by(-1); }
                        ui.label(format!("{}%", self.zoom_pct.round() as i32));
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(243, 243, 243)).inner_margin(egui::Margin::same(12.0)))
            .show(ctx, |ui| self.canvas_ui(ui, ctx));
        self.dialog_ui(ctx);
        self.text_ui(ctx);
        if self.tool != self.prev_tool { self.commit_sel(); self.prev_tool = self.tool; }
    }
}

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]).with_title("Untitled - EV Raster Crafts by EVORA"),
        ..Default::default()
    };
    eframe::run_native("EV Raster Crafts", opts, Box::new(|cc| { setup_style(&cc.egui_ctx); Ok(Box::new(App::new())) }))
}
