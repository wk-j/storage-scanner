//! The dot-matrix disk bar: a rounded field of square cells, one glowing run per category,
//! dim dots for free space, and particles that lift off a segment as it is cleared.

use rune::Pixels;

use crate::scan::N;

pub const BAR_W: f32 = 900.0;
pub const BAR_H: f32 = 132.0;
pub const BLEED_X: f32 = 36.0;
pub const BLEED_TOP: f32 = 86.0;

const SS: u32 = 2;
const PITCH: f32 = 5.7;
const CELL: f32 = 3.15;
const INSET: f32 = 11.0;
const RADIUS: f32 = 18.0;

#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub x: f32,
    pub w: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub color: [u8; 3],
    pub size: f32,
}

struct Tone {
    base: [u8; 3],
    spark: [u8; 3],
    glow: [u8; 3],
}

const TONE: [Tone; N] = [
    Tone { base: [46, 118, 230], spark: [198, 226, 255], glow: [72, 156, 255] },
    Tone { base: [32, 186, 104], spark: [198, 255, 214], glow: [52, 230, 132] },
    Tone { base: [226, 58, 176], spark: [255, 196, 232], glow: [255, 78, 196] },
    Tone { base: [148, 72, 236], spark: [226, 196, 255], glow: [178, 96, 255] },
    Tone { base: [32, 186, 196], spark: [186, 255, 248], glow: [52, 220, 230] },
    Tone { base: [224, 176, 42], spark: [255, 236, 168], glow: [255, 204, 64] },
    Tone { base: [156, 160, 170], spark: [232, 234, 240], glow: [214, 218, 226] },
    Tone { base: [228, 118, 36], spark: [255, 214, 160], glow: [255, 146, 48] },
    Tone { base: [224, 56, 70], spark: [255, 186, 186], glow: [255, 78, 92] },
];

struct Grid {
    cols: usize,
    rows: usize,
    ox: f32,
    oy: f32,
}

fn grid() -> Grid {
    let gap = PITCH - CELL;
    let inner_w = BAR_W - INSET * 2.0;
    let inner_h = BAR_H - INSET * 2.0;
    let cols = ((inner_w + gap) / PITCH).floor().max(1.0) as usize;
    let rows = ((inner_h + gap) / PITCH).floor().max(1.0) as usize;
    let grid_w = cols as f32 * PITCH - gap;
    let grid_h = rows as f32 * PITCH - gap;
    Grid { cols, rows, ox: (BAR_W - grid_w) / 2.0, oy: (BAR_H - grid_h) / 2.0 }
}

pub fn edges(bytes: &[f64; N], capacity: f64) -> [Edge; N] {
    let usable = BAR_W - INSET * 2.0;
    let cap = if capacity > 1.0 { capacity } else { 1.0 };
    let mut x = INSET;
    let mut out = [Edge { x: 0.0, w: 0.0 }; N];
    for i in 0..N {
        let w = ((bytes[i].max(0.0) / cap) as f32 * usable).max(0.0);
        out[i] = Edge { x, w };
        x += w;
    }
    out
}

pub fn advance(parts: &mut Vec<Particle>, prev: &mut Option<[Edge; N]>, now: &[Edge; N], dissolving: Option<usize>, dt: f32) {
    if let (Some(cat), Some(old)) = (dissolving, *prev) {
        let right_old = old[cat].x + old[cat].w;
        let right_new = now[cat].x + now[cat].w;
        if right_old > right_new + 0.35 {
            spawn(parts, right_new, right_old, TONE[cat].spark);
        }
    }
    *prev = Some(*now);
    let dt = dt.clamp(0.0, 0.05);
    if dt > 0.0 {
        for p in parts.iter_mut() {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vx *= (1.0 - 0.55 * dt).max(0.0);
            p.vy *= (1.0 - 0.28 * dt).max(0.0);
            p.life -= dt * 0.78;
        }
    }
    parts.retain(|p| p.life > 0.03 && p.y > -BLEED_TOP - 4.0 && p.x < BAR_W + BLEED_X);
    if parts.len() > 420 {
        let drop = parts.len() - 420;
        parts.drain(0..drop);
    }
}

fn spawn(parts: &mut Vec<Particle>, x0: f32, x1: f32, color: [u8; 3]) {
    let g = grid();
    let mut x = x0;
    let mut col = (x0 * 4.0) as u32;
    let mut added = 0u32;
    let step = if x1 - x0 > 36.0 { 2 } else { 1 };
    while x < x1 - 0.15 && added < 140 {
        for r in (0..g.rows).step_by(step) {
            let h = hash(col, r as u32);
            let y = g.oy + r as f32 * PITCH + CELL * 0.5;
            let speed = 0.55 + (h % 80) as f32 / 100.0;
            parts.push(Particle {
                x: x + CELL * 0.5 + (h % 5) as f32 * 0.15,
                y: y + (h % 3) as f32 * 0.2,
                vx: (8.0 + (h % 46) as f32) * speed,
                vy: -(28.0 + (h % 78) as f32) * speed,
                life: 0.75 + (h % 30) as f32 / 100.0,
                color: if h % 4 == 0 { mix(color, [255, 255, 255], 0.45) } else { color },
                size: 1.5 + (h % 3) as f32 * 0.55,
            });
            added += 1;
            if added >= 140 {
                break;
            }
        }
        x += PITCH * step as f32;
        col += 1;
    }
}

pub struct Input<'a> {
    pub bytes: &'a [f64; N],
    pub capacity: f64,
    pub hover: Option<usize>,
    pub scanning: bool,
    pub particles: &'a [Particle],
}

pub fn render(pixels: &Pixels, sig: &mut u64, input: &Input<'_>) {
    let next = signature(input);
    if next == *sig && *sig != 0 {
        return;
    }
    *sig = next;
    let (w, h, buf) = raster(input);
    pixels.set(w, h, buf);
}

fn signature(input: &Input<'_>) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mix = |h: &mut u64, v: u64| *h = h.wrapping_mul(0x100000001b3).wrapping_add(v);
    for b in input.bytes {
        mix(&mut h, (*b / 1.0e7) as u64);
    }
    mix(&mut h, (input.capacity / 1.0e9) as u64);
    mix(&mut h, input.hover.unwrap_or(64) as u64);
    mix(&mut h, input.scanning as u64);
    mix(&mut h, input.particles.len() as u64);
    for p in input.particles.iter().step_by(2) {
        mix(&mut h, (p.x * 4.0) as i32 as u32 as u64);
        mix(&mut h, (p.y * 4.0) as i32 as u32 as u64);
        mix(&mut h, (p.life * 16.0) as u64);
    }
    h
}

fn raster(input: &Input<'_>) -> (u32, u32, Vec<u8>) {
    let w = ((BAR_W + BLEED_X * 2.0) * SS as f32).round() as u32;
    let h = ((BAR_H + BLEED_TOP) * SS as f32).round() as u32;
    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    let edges = edges(input.bytes, input.capacity);
    let g = grid();

    fill_round_bar(&mut buf, w, h);
    for (i, edge) in edges.iter().enumerate() {
        if edge.w < 0.8 {
            continue;
        }
        let dim = input.hover.is_some_and(|h| h != i);
        paint_glow(&mut buf, w, h, *edge, TONE[i].glow, if dim { 0.42 } else { 1.0 });
    }
    if input.scanning {
        let frontier = edges.iter().fold(INSET, |acc, e| if e.w > 0.4 { e.x + e.w } else { acc });
        if frontier > INSET + 2.0 {
            paint_frontier(&mut buf, w, h, frontier);
        }
    }
    paint_cells(&mut buf, w, h, &g, &edges, input);
    paint_particles(&mut buf, w, h, input.particles);
    (w, h, buf)
}

fn buf_xy(x: f32, y: f32) -> (f32, f32) {
    ((x + BLEED_X) * SS as f32, (y + BLEED_TOP) * SS as f32)
}

fn fill_round_bar(buf: &mut [u8], w: u32, h: u32) {
    let (x0, y0) = buf_xy(0.0, 0.0);
    let (x1, y1) = buf_xy(BAR_W, BAR_H);
    let r = RADIUS * SS as f32;
    let cx = (x0 + x1) * 0.5;
    let cy = (y0 + y1) * 0.5;
    let hx = (x1 - x0) * 0.5;
    let hy = (y1 - y0) * 0.5;
    let fill = [16, 17, 20, 255];
    for y in (y0.floor() as i32).max(0)..(y1.ceil() as i32).min(h as i32) {
        for x in (x0.floor() as i32).max(0)..(x1.ceil() as i32).min(w as i32) {
            let d = sd_round(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy, hx, hy, r);
            if d <= 0.6 {
                put(buf, w, x, y, fill);
            }
        }
    }
}

fn paint_glow(buf: &mut [u8], w: u32, h: u32, edge: Edge, color: [u8; 3], gain: f32) {
    let pad = 20.0;
    let (x0, y0) = buf_xy(edge.x - pad, -pad);
    let (x1, y1) = buf_xy(edge.x + edge.w + pad, BAR_H + pad);
    let (cx, cy) = buf_xy(edge.x + edge.w * 0.5, BAR_H * 0.5);
    let hx = edge.w * 0.5 * SS as f32;
    let hy = BAR_H * 0.5 * SS as f32;
    // Interior cuts stay straight. The outer rim follows the bar's rounded ends.
    let box_r = (3.0 * SS as f32).min(hx.min(hy));
    let (bx, by) = buf_xy(BAR_W * 0.5, BAR_H * 0.5);
    let bhx = BAR_W * 0.5 * SS as f32;
    let bhy = BAR_H * 0.5 * SS as f32;
    let bar_r = RADIUS * SS as f32;
    let (sx0, _) = buf_xy(edge.x - 2.0, 0.0);
    let (sx1, _) = buf_xy(edge.x + edge.w + 2.0, 0.0);
    let sigma = 6.2 * SS as f32;
    let y_lo = (y0.floor() as i32).max(0);
    let y_hi = (y1.ceil() as i32).min(h as i32);
    let x_lo = (x0.floor() as i32).max(0);
    let x_hi = (x1.ceil() as i32).min(w as i32);
    for y in y_lo..y_hi {
        for x in x_lo..x_hi {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d_box = sd_round(px - cx, py - cy, hx, hy, box_r).abs();
            let d_bar = sd_round(px - bx, py - by, bhx, bhy, bar_r).abs();
            let on_rim = px >= sx0 && px <= sx1;
            let d = if on_rim { d_box.min(d_bar) } else { d_box };
            let g = (-(d * d) / (2.0 * sigma * sigma)).exp() * gain;
            if g > 0.02 {
                add_glow(buf, w, x, y, color, g * 0.85);
            }
        }
    }
}

fn paint_frontier(buf: &mut [u8], w: u32, h: u32, frontier: f32) {
    let (x0, y0) = buf_xy(frontier - 18.0, 0.0);
    let (x1, y1) = buf_xy(frontier + 8.0, BAR_H);
    let (fx, _) = buf_xy(frontier, 0.0);
    let sigma = 5.5 * SS as f32;
    for y in (y0.floor() as i32).max(0)..(y1.ceil() as i32).min(h as i32) {
        for x in (x0.floor() as i32).max(0)..(x1.ceil() as i32).min(w as i32) {
            let d = (x as f32 - fx).abs();
            let g = (-(d * d) / (2.0 * sigma * sigma)).exp();
            if g > 0.04 {
                add_glow(buf, w, x, y, [255, 255, 255], g * 0.85);
            }
        }
    }
}

fn paint_cells(buf: &mut [u8], w: u32, h: u32, g: &Grid, edges: &[Edge; N], input: &Input<'_>) {
    let frontier = edges.iter().fold(INSET, |acc, e| if e.w > 0.4 { e.x + e.w } else { acc });
    let cw = (CELL * SS as f32).round().max(1.0) as i32;
    for r in 0..g.rows {
        for c in 0..g.cols {
            let x = g.ox + c as f32 * PITCH;
            let y = g.oy + r as f32 * PITCH;
            let cx = x + CELL * 0.5;
            let cy = y + CELL * 0.5;
            if sd_round(cx - BAR_W * 0.5, cy - BAR_H * 0.5, BAR_W * 0.5, BAR_H * 0.5, RADIUS) > -0.2 {
                continue;
            }
            let cat = cat_at(edges, cx);
            let mut rgb = [68, 70, 76];
            let mut bright = 1.0f32;
            if let Some(i) = cat {
                let hsh = hash(c as u32, r as u32);
                let tone = &TONE[i];
                rgb = if hsh % 7 == 0 {
                    tone.spark
                } else if hsh % 4 == 0 {
                    mix(tone.base, tone.spark, 0.55)
                } else if hsh % 9 == 0 {
                    mix(tone.base, [0, 0, 0], 0.28)
                } else {
                    tone.base
                };
                if input.hover.is_some_and(|hv| hv != i) {
                    bright = 0.58;
                }
                if input.scanning {
                    let dist = frontier - cx;
                    if (0.0..18.0).contains(&dist) {
                        let k = (1.0 - dist / 18.0).clamp(0.0, 1.0);
                        rgb = mix(rgb, [255, 255, 255], k * 0.82);
                    }
                }
            }
            if bright < 1.0 {
                rgb = [scale(rgb[0], bright), scale(rgb[1], bright), scale(rgb[2], bright)];
            }
            let (bx, by) = buf_xy(x, y);
            fill_rect(buf, w, h, bx.round() as i32, by.round() as i32, cw, cw, [rgb[0], rgb[1], rgb[2], 255]);
        }
    }
}

fn paint_particles(buf: &mut [u8], w: u32, h: u32, parts: &[Particle]) {
    for p in parts {
        let (bx, by) = buf_xy(p.x, p.y);
        let s = (p.size * SS as f32).round().max(1.0) as i32;
        let a = (p.life.clamp(0.0, 1.0).powf(1.15) * 255.0) as u8;
        fill_rect(buf, w, h, bx.round() as i32 - s / 2, by.round() as i32 - s / 2, s, s, [p.color[0], p.color[1], p.color[2], a]);
        if p.life > 0.35 {
            add_glow(buf, w, bx.round() as i32, by.round() as i32, p.color, p.life * 0.45);
        }
    }
}

fn cat_at(edges: &[Edge; N], x: f32) -> Option<usize> {
    edges.iter().position(|e| e.w > 0.4 && x >= e.x && x < e.x + e.w)
}

fn sd_round(px: f32, py: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let r = r.min(hx.min(hy));
    let qx = px.abs() - hx + r;
    let qy = py.abs() - hy + r;
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    qx.max(qy).min(0.0) + (ax * ax + ay * ay).sqrt() - r
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

fn scale(c: u8, k: f32) -> u8 {
    (c as f32 * k).round() as u8
}

fn idx(w: u32, x: i32, y: i32) -> Option<usize> {
    if x < 0 || y < 0 || x as u32 >= w {
        return None;
    }
    Some((y as usize) * (w as usize) + x as usize)
}

fn put(buf: &mut [u8], w: u32, x: i32, y: i32, rgba: [u8; 4]) {
    let Some(i) = idx(w, x, y) else { return };
    let o = i * 4;
    if o + 3 < buf.len() {
        buf[o..o + 4].copy_from_slice(&rgba);
    }
}

fn add_glow(buf: &mut [u8], w: u32, x: i32, y: i32, rgb: [u8; 3], a: f32) {
    let Some(i) = idx(w, x, y) else { return };
    let o = i * 4;
    if o + 3 >= buf.len() {
        return;
    }
    let a = a.clamp(0.0, 1.0);
    for k in 0..3 {
        let d = buf[o + k] as f32;
        buf[o + k] = (d + rgb[k] as f32 * a).min(255.0) as u8;
    }
    let da = buf[o + 3] as f32;
    buf[o + 3] = (da + a * (255.0 - da)).min(255.0) as u8;
}

fn fill_rect(buf: &mut [u8], w: u32, h: u32, x: i32, y: i32, rw: i32, rh: i32, rgba: [u8; 4]) {
    for yy in y..(y + rh) {
        if yy < 0 || yy as u32 >= h {
            continue;
        }
        for xx in x..(x + rw) {
            if rgba[3] == 255 {
                put(buf, w, xx, yy, rgba);
            } else {
                blend(buf, w, xx, yy, rgba);
            }
        }
    }
}

fn blend(buf: &mut [u8], w: u32, x: i32, y: i32, src: [u8; 4]) {
    let Some(i) = idx(w, x, y) else { return };
    let o = i * 4;
    if o + 3 >= buf.len() {
        return;
    }
    let sa = src[3] as f32 / 255.0;
    let da = buf[o + 3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-4 {
        return;
    }
    for k in 0..3 {
        let s = src[k] as f32 / 255.0;
        let d = buf[o + k] as f32 / 255.0;
        let v = (s * sa + d * da * (1.0 - sa)) / out_a;
        buf[o + k] = (v * 255.0).round() as u8;
    }
    buf[o + 3] = (out_a * 255.0).round() as u8;
}

fn hash(x: u32, y: u32) -> u32 {
    let mut n = x.wrapping_mul(374761393).wrapping_add(y.wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    n ^ (n >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::{self, CAPACITY};
    use crate::scan::Cat;

    #[test]
    fn cells_colour_the_used_prefix_and_leave_free_space_dim() {
        let bytes = demo::bytes(demo::SCAN_END, None, None, false);
        let (w, h, buf) = raster(&Input { bytes: &bytes, capacity: CAPACITY, hover: Some(Cat::Apps.ix()), scanning: false, particles: &[] });
        assert!(w > 100 && h > 100);
        let sample = |lx: f32, ly: f32| {
            let (x, y) = buf_xy(lx, ly);
            let o = (y.round() as usize * w as usize + x.round() as usize) * 4;
            [buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]
        };
        let apps = sample(28.0, BAR_H * 0.5);
        let free = sample(BAR_W - 36.0, BAR_H * 0.5);
        assert!(apps[2] > apps[0], "apps cells lean blue: {apps:?}");
        assert!(apps[3] > 200);
        assert!(free[0] < 90 && free[2] < 90, "free space stays dim: {free:?}");
        let above = sample(28.0, -70.0);
        assert!(above[3] < 8, "far above the bar stays clear: {above:?}");
    }
}
