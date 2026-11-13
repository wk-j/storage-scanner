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
    /// Per-category hover, 0 at rest and 1 fully under the pointer. Neighbours dim by the strongest other value.
    pub hover: &'a [f32; N],
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
    for v in input.hover {
        mix(&mut h, (*v * 1000.0).round() as u64);
    }
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
    paint_bar_outline(&mut buf, w, h);
    for (i, edge) in edges.iter().enumerate() {
        if edge.w < 0.8 {
            continue;
        }
        // A soft halo only while this segment is the one under the pointer. Neighbours stay flat.
        paint_hover_glow(&mut buf, w, h, *edge, TONE[i].glow, input.hover[i]);
    }
    if input.scanning {
        let frontier = edges.iter().fold(INSET, |acc, e| if e.w > 0.4 { e.x + e.w } else { acc });
        if frontier > INSET + 2.0 {
            paint_frontier(&mut buf, w, h, frontier);
        }
    }
    paint_cells(&mut buf, w, h, &g, &edges, input);
    // Stronger hovers paint last, so a shared edge takes the hovered colour instead of stacking.
    let mut order: Vec<usize> = (0..N).collect();
    order.sort_by(|&a, &b| input.hover[a].total_cmp(&input.hover[b]));
    for i in order {
        let edge = edges[i];
        if edge.w < 0.8 {
            continue;
        }
        let mine = input.hover[i];
        let tinted = mix(TONE[i].base, [128, 132, 140], 0.62 * (1.0 - mine));
        let col = mix(tinted, TONE[i].glow, mine);
        let alpha = lerp(0.22, 0.78, mine);
        paint_segment_outline(&mut buf, w, h, edge, col, alpha);
    }
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

/// One-pixel rim of the whole capsule, in a faint neutral colour.
fn paint_bar_outline(buf: &mut [u8], w: u32, h: u32) {
    let (bx, by, bhx, bhy, bar_r) = bar_sdf_frame();
    let pad = 2.0;
    let (x0, y0) = buf_xy(-pad, -pad);
    let (x1, y1) = buf_xy(BAR_W + pad, BAR_H + pad);
    let outer = 1.05 * SS as f32;
    for y in (y0.floor() as i32).max(0)..(y1.ceil() as i32).min(h as i32) {
        for x in (x0.floor() as i32).max(0)..(x1.ceil() as i32).min(w as i32) {
            let d = sd_round(x as f32 + 0.5 - bx, y as f32 + 0.5 - by, bhx, bhy, bar_r);
            let cov = stroke_cover(d, outer);
            if cov > 0.04 {
                blend(buf, w, x, y, [255, 255, 255, (cov * 0.16 * 255.0) as u8]);
            }
        }
    }
}

/// A short, faint halo just outside the hovered segment. Non-hovered segments pass `amount` 0.
fn paint_hover_glow(buf: &mut [u8], w: u32, h: u32, edge: Edge, color: [u8; 3], amount: f32) {
    if amount < 0.02 {
        return;
    }
    let pad = 8.0;
    let (x0, y0) = buf_xy(edge.x - pad, -pad);
    let (x1, y1) = buf_xy(edge.x + edge.w + pad, BAR_H + pad);
    let sigma = 3.2 * SS as f32;
    for y in (y0.floor() as i32).max(0)..(y1.ceil() as i32).min(h as i32) {
        for x in (x0.floor() as i32).max(0)..(x1.ceil() as i32).min(w as i32) {
            let d = segment_distance(x as f32 + 0.5, y as f32 + 0.5, edge);
            // Outside the segment only, so the cells stay crisp and the seam does not bloom.
            if d > 0.3 && d < sigma * 2.6 {
                let g = (-(d * d) / (2.0 * sigma * sigma)).exp() * 0.48 * amount;
                if g > 0.015 {
                    add_glow(buf, w, x, y, color, g);
                }
            }
        }
    }
}

/// A hair over one point, in the segment's own colour. Shared edges overwrite instead of adding.
fn paint_segment_outline(buf: &mut [u8], w: u32, h: u32, edge: Edge, color: [u8; 3], alpha: f32) {
    let pad = 2.0;
    let (x0, y0) = buf_xy(edge.x - pad, -pad);
    let (x1, y1) = buf_xy(edge.x + edge.w + pad, BAR_H + pad);
    let outer = 0.85 * SS as f32;
    for y in (y0.floor() as i32).max(0)..(y1.ceil() as i32).min(h as i32) {
        for x in (x0.floor() as i32).max(0)..(x1.ceil() as i32).min(w as i32) {
            let d = segment_distance(x as f32 + 0.5, y as f32 + 0.5, edge);
            let cov = stroke_cover(d, outer);
            if cov > 0.04 {
                blend(buf, w, x, y, [color[0], color[1], color[2], (cov * alpha * 255.0) as u8]);
            }
        }
    }
}

fn bar_sdf_frame() -> (f32, f32, f32, f32, f32) {
    let (bx, by) = buf_xy(BAR_W * 0.5, BAR_H * 0.5);
    (bx, by, BAR_W * 0.5 * SS as f32, BAR_H * 0.5 * SS as f32, RADIUS * SS as f32)
}

/// Signed distance to the segment, clipped to the bar's rounded capsule so the outer ends curve.
fn segment_distance(px: f32, py: f32, edge: Edge) -> f32 {
    let (x0, y0) = buf_xy(edge.x, 0.0);
    let (x1, y1) = buf_xy(edge.x + edge.w, BAR_H);
    let d_seg = sd_box(px, py, (x0 + x1) * 0.5, (y0 + y1) * 0.5, (x1 - x0) * 0.5, (y1 - y0) * 0.5);
    let (bx, by, bhx, bhy, bar_r) = bar_sdf_frame();
    let d_bar = sd_round(px - bx, py - by, bhx, bhy, bar_r);
    d_seg.max(d_bar)
}

/// 1 inside the stroke, falling to 0 at `outer` buffer pixels past the edge.
fn stroke_cover(d: f32, outer: f32) -> f32 {
    if d < -0.65 {
        0.0
    } else if d < 0.0 {
        1.0
    } else {
        (1.0 - d / outer).clamp(0.0, 1.0)
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
                // Resting segments sit below full colour. The one under the pointer comes up;
                // the others drop toward the bar colour, which is what the reference does on hover.
                let mine = input.hover[i];
                let dim = others(input.hover, i);
                bright = lerp(lerp(0.82, 0.50, dim), 1.0, mine);
                // A little grey in the dimmed segments, so they read as the reference's dusty dots.
                rgb = mix(rgb, [86, 90, 98], 0.16 * dim * (1.0 - mine));
                if mine > 0.0 {
                    rgb = mix(rgb, tone.spark, 0.30 * mine);
                }
                if input.scanning {
                    let dist = frontier - cx;
                    if (0.0..18.0).contains(&dist) {
                        let k = (1.0 - dist / 18.0).clamp(0.0, 1.0);
                        rgb = mix(rgb, [255, 255, 255], k * 0.82);
                    }
                }
            }
            if bright < 0.999 {
                rgb = mix([24, 25, 28], rgb, bright);
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

fn others(hover: &[f32; N], index: usize) -> f32 {
    let mut max = 0.0f32;
    for (i, v) in hover.iter().enumerate() {
        if i != index {
            max = max.max(*v);
        }
    }
    max.clamp(0.0, 1.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn cat_at(edges: &[Edge; N], x: f32) -> Option<usize> {
    edges.iter().position(|e| e.w > 0.4 && x >= e.x && x < e.x + e.w)
}

fn sd_box(px: f32, py: f32, cx: f32, cy: f32, hx: f32, hy: f32) -> f32 {
    let qx = (px - cx).abs() - hx;
    let qy = (py - cy).abs() - hy;
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    qx.max(qy).min(0.0) + (ax * ax + ay * ay).sqrt()
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
        let mut hover = [0.0; N];
        hover[Cat::Apps.ix()] = 1.0;
        let (w, h, buf) = raster(&Input { bytes: &bytes, capacity: CAPACITY, hover: &hover, scanning: false, particles: &[] });
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

        let spans = edges(&bytes, CAPACITY);
        let seam = spans[Cat::Photos.ix()].x + spans[Cat::Photos.ix()].w;
        let above_seam = sample(seam, -5.0);
        assert!(above_seam[3] < 16, "a non-hovered boundary does not glow upward: {above_seam:?}");
        let above_apps = sample(spans[Cat::Apps.ix()].x + 12.0, -4.0);
        assert!(above_apps[3] < 90, "the hovered halo stays faint: {above_apps:?}");
        // The top of the bar, on the seam and a few points beside it: a line, not a wide bright band.
        let on_line = sample(seam, 3.0);
        let beside = sample(seam + 7.0, 3.0);
        let line_peak = on_line[0].max(on_line[1]).max(on_line[2]);
        let beside_peak = beside[0].max(beside[1]).max(beside[2]);
        assert!(line_peak < 160, "the seam is not a neon strip: {on_line:?}");
        assert!(beside_peak + 12 < line_peak || beside_peak < 70, "the outline is about a pixel wide: line {on_line:?} beside {beside:?}");
    }
}
