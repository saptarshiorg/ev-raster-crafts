//! EV Raster Crafts core engine (by EVORA).
//!
//! Usage: ev-raster "cmd:args;cmd:args;render"
//!
//! Commands:
//!   size:N                      reset canvas to N x N (1..=64)
//!   color:<color>               set primary color (hex "#RRGGBB"/"#RGB" or a name)
//!   pixel:x,y[,color]           draw a single pixel (pencil)
//!   line:x0,y0,x1,y1[,color]    draw a continuous stroke (brush, Bresenham)
//!   erase:x,y                   overwrite a pixel with the background (eraser)
//!   fill:x,y[,color]            4-way flood fill (bucket)
//!   rotate:deg                  rotate clockwise by 90 / 180 / 270 (-90 = left 90)
//!   flip:horizontal|vertical    mirror the canvas
//!   clear                       wipe canvas to background (palette is kept)
//!   render                      print the canvas to stdout

use std::env;
use std::process;

const DEFAULT_SIZE: usize = 16;
const MAX_SIZE: usize = 64;
const BACKGROUND: u32 = 0x00FF_FFFF;
const PALETTE_LIMIT: usize = 16;

/// Emoji palette used for rendering; each pixel maps to the nearest entry.
const EMOJI_PALETTE: [(u32, &str); 9] = [
    (0xFFFFFF, "⚪"),
    (0xFF0000, "🔴"),
    (0x0000FF, "🔵"),
    (0x000000, "⚫"),
    (0x00C000, "🟢"),
    (0xFFE000, "🟡"),
    (0xFF8C00, "🟠"),
    (0x8000FF, "🟣"),
    (0x8B4513, "🟤"),
];

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<Vec<u32>>,
    active_color: u32,
    palette_history: Vec<u32>,
}

impl Canvas {
    fn new(size: usize) -> Canvas {
        Canvas {
            width: size,
            height: size,
            pixels: vec![vec![BACKGROUND; size]; size],
            active_color: 0x0000_0000,
            palette_history: vec![0x0000_0000],
        }
    }

    fn in_bounds(&self, x: i64, y: i64) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    fn check_bounds(&self, x: i64, y: i64) -> Result<(usize, usize), String> {
        if self.in_bounds(x, y) {
            Ok((x as usize, y as usize))
        } else {
            Err(format!(
                "coordinate ({}, {}) is outside the {}x{} canvas",
                x, y, self.width, self.height
            ))
        }
    }

    /// Set the primary color and remember it (most recent first, deduplicated).
    fn set_active_color(&mut self, color: u32) {
        self.active_color = color;
        self.palette_history.retain(|c| *c != color);
        self.palette_history.insert(0, color);
        self.palette_history.truncate(PALETTE_LIMIT);
    }

    fn draw_pixel(&mut self, x: i64, y: i64, color: u32) -> Result<(), String> {
        let (ux, uy) = self.check_bounds(x, y)?;
        self.pixels[uy][ux] = color;
        Ok(())
    }

    fn erase_pixel(&mut self, x: i64, y: i64) -> Result<(), String> {
        self.draw_pixel(x, y, BACKGROUND)
    }

    /// Bresenham line: every cell between the two points is painted.
    fn draw_line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, color: u32) -> Result<(), String> {
        self.check_bounds(x0, y0)?;
        self.check_bounds(x1, y1)?;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            self.draw_pixel(x, y, color)?;
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
        Ok(())
    }

    /// Stack-based, 4-connected flood fill with boundary checks.
    fn flood_fill(&mut self, x: i64, y: i64, color: u32) -> Result<(), String> {
        let (sx, sy) = self.check_bounds(x, y)?;
        let target = self.pixels[sy][sx];
        if target == color {
            return Ok(());
        }
        let mut stack: Vec<(usize, usize)> = vec![(sx, sy)];
        while let Some((cx, cy)) = stack.pop() {
            if self.pixels[cy][cx] != target {
                continue;
            }
            self.pixels[cy][cx] = color;
            if cx > 0 {
                stack.push((cx - 1, cy));
            }
            if cx + 1 < self.width {
                stack.push((cx + 1, cy));
            }
            if cy > 0 {
                stack.push((cx, cy - 1));
            }
            if cy + 1 < self.height {
                stack.push((cx, cy + 1));
            }
        }
        Ok(())
    }

    /// Rotate clockwise. Accepts 90, 180, 270 and their negatives / multiples of 360.
    fn rotate_canvas(&mut self, degrees: i64) -> Result<(), String> {
        let normalized = degrees.rem_euclid(360);
        match normalized {
            0 => {}
            90 => {
                let (w, h) = (self.width, self.height);
                let mut out = vec![vec![BACKGROUND; h]; w];
                for r in 0..w {
                    for c in 0..h {
                        out[r][c] = self.pixels[h - 1 - c][r];
                    }
                }
                self.pixels = out;
                self.width = h;
                self.height = w;
            }
            180 => {
                self.pixels.reverse();
                for row in self.pixels.iter_mut() {
                    row.reverse();
                }
            }
            270 => {
                let (w, h) = (self.width, self.height);
                let mut out = vec![vec![BACKGROUND; h]; w];
                for r in 0..w {
                    for c in 0..h {
                        out[r][c] = self.pixels[c][w - 1 - r];
                    }
                }
                self.pixels = out;
                self.width = h;
                self.height = w;
            }
            _ => {
                return Err(format!(
                    "rotation must be a multiple of 90 degrees, got {}",
                    degrees
                ))
            }
        }
        Ok(())
    }

    fn flip_canvas(&mut self, axis: &str) -> Result<(), String> {
        match axis {
            "horizontal" => {
                for row in self.pixels.iter_mut() {
                    row.reverse();
                }
                Ok(())
            }
            "vertical" => {
                self.pixels.reverse();
                Ok(())
            }
            other => Err(format!(
                "flip axis must be 'horizontal' or 'vertical', got '{}'",
                other
            )),
        }
    }

    fn clear(&mut self) {
        for row in self.pixels.iter_mut() {
            for cell in row.iter_mut() {
                *cell = BACKGROUND;
            }
        }
    }

    fn render(&self) -> String {
        let mut out = String::new();
        for row in &self.pixels {
            for &color in row {
                out.push_str(nearest_emoji(color));
            }
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&format!(
            "Canvas: {}x{} | Active color: {}\n",
            self.width,
            self.height,
            hex_string(self.active_color)
        ));
        let history: Vec<String> = self.palette_history.iter().map(|c| hex_string(*c)).collect();
        out.push_str(&format!("Palette history: {}\n", history.join(" ")));
        out
    }
}

fn hex_string(color: u32) -> String {
    format!("#{:06X}", color & 0x00FF_FFFF)
}

fn nearest_emoji(color: u32) -> &'static str {
    let (r, g, b) = (
        ((color >> 16) & 0xFF) as i64,
        ((color >> 8) & 0xFF) as i64,
        (color & 0xFF) as i64,
    );
    let mut best = EMOJI_PALETTE[0].1;
    let mut best_dist = i64::MAX;
    for (ref_color, emoji) in EMOJI_PALETTE.iter() {
        let (rr, rg, rb) = (
            ((ref_color >> 16) & 0xFF) as i64,
            ((ref_color >> 8) & 0xFF) as i64,
            (ref_color & 0xFF) as i64,
        );
        let dist = (r - rr).pow(2) + (g - rg).pow(2) + (b - rb).pow(2);
        if dist < best_dist {
            best_dist = dist;
            best = emoji;
        }
    }
    best
}

fn parse_color(input: &str) -> Result<u32, String> {
    let s = input.trim().to_lowercase();
    let named = match s.as_str() {
        "white" => Some(0xFFFFFF),
        "black" => Some(0x000000),
        "red" => Some(0xFF0000),
        "blue" => Some(0x0000FF),
        "green" => Some(0x00C000),
        "yellow" => Some(0xFFE000),
        "orange" => Some(0xFF8C00),
        "purple" => Some(0x8000FF),
        "brown" => Some(0x8B4513),
        _ => None,
    };
    if let Some(c) = named {
        return Ok(c);
    }
    let hex = s.trim_start_matches('#');
    let expanded: String = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 => hex.to_string(),
        _ => return Err(format!("invalid color '{}'", input)),
    };
    u32::from_str_radix(&expanded, 16).map_err(|_| format!("invalid color '{}'", input))
}

fn parse_int(input: &str) -> Result<i64, String> {
    input
        .trim()
        .parse::<i64>()
        .map_err(|_| format!("invalid integer '{}'", input))
}

fn split_args(args: &str) -> Vec<&str> {
    if args.trim().is_empty() {
        Vec::new()
    } else {
        args.split(',').map(|a| a.trim()).collect()
    }
}

/// Resolve an optional trailing color argument; explicit colors become the primary color.
fn resolve_color(canvas: &mut Canvas, arg: Option<&&str>) -> Result<u32, String> {
    match arg {
        Some(text) => {
            let color = parse_color(text)?;
            canvas.set_active_color(color);
            Ok(color)
        }
        None => Ok(canvas.active_color),
    }
}

fn expect_len(cmd: &str, parts: &[&str], min: usize, max: usize) -> Result<(), String> {
    if parts.len() < min || parts.len() > max {
        Err(format!(
            "command '{}' expects {}-{} arguments, got {}",
            cmd,
            min,
            max,
            parts.len()
        ))
    } else {
        Ok(())
    }
}

/// Execute a "cmd:args;cmd:args" program. Returns the rendered output if `render` ran.
fn run_program(program: &str) -> Result<Option<String>, String> {
    let mut canvas = Canvas::new(DEFAULT_SIZE);
    let mut output: Option<String> = None;

    for token in program.split(';') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let (cmd, args) = match token.find(':') {
            Some(i) => (token[..i].trim().to_lowercase(), token[i + 1..].trim()),
            None => (token.to_lowercase(), ""),
        };
        let parts = split_args(args);

        match cmd.as_str() {
            "size" => {
                expect_len("size", &parts, 1, 1)?;
                let n = parse_int(parts[0])?;
                if n < 1 || n as usize > MAX_SIZE {
                    return Err(format!("size must be between 1 and {}", MAX_SIZE));
                }
                let keep_color = canvas.active_color;
                let keep_history = canvas.palette_history.clone();
                canvas = Canvas::new(n as usize);
                canvas.active_color = keep_color;
                canvas.palette_history = keep_history;
            }
            "color" => {
                expect_len("color", &parts, 1, 1)?;
                let color = parse_color(parts[0])?;
                canvas.set_active_color(color);
            }
            "pixel" => {
                expect_len("pixel", &parts, 2, 3)?;
                let (x, y) = (parse_int(parts[0])?, parse_int(parts[1])?);
                let color = resolve_color(&mut canvas, parts.get(2))?;
                canvas.draw_pixel(x, y, color)?;
            }
            "line" => {
                expect_len("line", &parts, 4, 5)?;
                let (x0, y0) = (parse_int(parts[0])?, parse_int(parts[1])?);
                let (x1, y1) = (parse_int(parts[2])?, parse_int(parts[3])?);
                let color = resolve_color(&mut canvas, parts.get(4))?;
                canvas.draw_line(x0, y0, x1, y1, color)?;
            }
            "erase" => {
                expect_len("erase", &parts, 2, 2)?;
                let (x, y) = (parse_int(parts[0])?, parse_int(parts[1])?);
                canvas.erase_pixel(x, y)?;
            }
            "fill" => {
                expect_len("fill", &parts, 2, 3)?;
                let (x, y) = (parse_int(parts[0])?, parse_int(parts[1])?);
                let color = resolve_color(&mut canvas, parts.get(2))?;
                canvas.flood_fill(x, y, color)?;
            }
            "rotate" => {
                expect_len("rotate", &parts, 1, 1)?;
                canvas.rotate_canvas(parse_int(parts[0])?)?;
            }
            "flip" => {
                expect_len("flip", &parts, 1, 1)?;
                canvas.flip_canvas(&parts[0].to_lowercase())?;
            }
            "clear" => {
                canvas.clear();
            }
            "render" => {
                output = Some(canvas.render());
            }
            other => return Err(format!("unknown command '{}'", other)),
        }
    }
    Ok(output)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: ev-raster \"cmd:args;cmd:args;render\"");
        process::exit(64);
    }
    let program = args.join(" ");
    match run_program(&program) {
        Ok(Some(rendered)) => print!("{}", rendered),
        Ok(None) => {}
        Err(message) => {
            eprintln!("ev-raster error: {}", message);
            process::exit(2);
        }
    }
}
