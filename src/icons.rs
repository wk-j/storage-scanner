//! Small category glyphs, drawn in the category colour. Coordinates are a 16-point box.

use rune::color::Color;
use rune::{El, Shape, el};

use crate::scan::Cat;

fn c(hex: u32) -> Color {
    rune::color::hex(&format!("#{hex:06x}")).expect("hex colour")
}

fn ink(cat: Cat) -> Color {
    c(cat.ui())
}

pub fn category(cat: Cat, size: f32) -> El {
    let shape = match cat {
        Cat::Apps => apps(),
        Cat::Developer => developer(),
        Cat::Photos => photos(),
        Cat::Movies => movies(),
        Cat::Documents => documents(),
        Cat::Music => music(),
        Cat::System => system(),
        Cat::Caches => caches(),
        Cat::Trash => trash(),
    };
    el().size(size, size).shape(shape.view_box(16.0, 16.0))
}

pub fn rescan(size: f32) -> El {
    let col = c(0xb7b9c2);
    let shape = Shape::new()
        .stroke(arc(8.0, 8.0, 5.3, 0.9, 5.3, 18), 1.7, col)
        .fill(tri([11.6, 1.5], [15.4, 3.6], [12.2, 6.4]), col)
        .stroke(arc(8.0, 8.0, 5.3, 4.0, 8.4, 18), 1.7, col)
        .fill(tri([4.4, 14.5], [0.6, 12.4], [3.8, 9.6]), col);
    el().size(size, size).shape(shape.view_box(16.0, 16.0))
}

pub fn sparkle(size: f32, color: u32) -> El {
    let col = c(color);
    let shape = Shape::new().fill(diamond(8.0, 8.0, 1.15, 6.2), col).fill(diamond(8.0, 8.0, 6.2, 1.15), col);
    el().size(size, size).shape(shape.view_box(16.0, 16.0))
}

fn apps() -> Shape {
    let col = ink(Cat::Apps);
    let dark = c(0x14283e);
    Shape::new()
        .fill(roundish(1.0, 2.0, 13.5, 10.5, 2.0), col)
        .stroke(vec![[1.5, 6.0], [14.0, 6.0]], 1.0, dark)
        .fill(ngon(3.5, 4.0, 0.65, 8), dark)
        .fill(ngon(6.0, 4.0, 0.65, 8), dark)
        .fill(vec![[8.0, 8.0], [15.0, 11.3], [11.5, 12.1], [10.0, 15.5]], dark)
        .fill(vec![[9.0, 9.3], [14.0, 11.5], [11.0, 12.0], [10.3, 14.1]], col)
}

fn developer() -> Shape {
    let col = ink(Cat::Developer);
    let dark = c(0x12351f);
    Shape::new()
        .fill(roundish(2.0, 1.6, 12.0, 13.0, 3.0), col)
        .stroke(vec![[5.5, 5.0], [5.5, 2.5], [10.5, 2.5], [10.5, 5.0]], 1.3, dark)
        .fill(roundish(4.0, 10.0, 4.0, 1.7, 0.6), dark)
}

fn photos() -> Shape {
    let col = ink(Cat::Photos);
    let sun = c(0xffd0ef);
    Shape::new()
        .fill(vec![[1.4, 13.4], [6.2, 6.4], [9.0, 10.2], [11.2, 7.6], [14.8, 13.4]], col)
        .fill(ngon(11.4, 4.4, 2.0, 10), sun)
}

fn movies() -> Shape {
    let col = ink(Cat::Movies);
    let hole = c(0x241038);
    Shape::new()
        .fill(roundish(1.2, 3.2, 13.6, 9.6, 1.8), col)
        .fill(vec![[2.2, 4.2], [3.5, 4.2], [3.5, 6.0], [2.2, 6.0]], hole)
        .fill(vec![[2.2, 10.0], [3.5, 10.0], [3.5, 11.8], [2.2, 11.8]], hole)
        .fill(vec![[12.5, 4.2], [13.8, 4.2], [13.8, 6.0], [12.5, 6.0]], hole)
        .fill(vec![[12.5, 10.0], [13.8, 10.0], [13.8, 11.8], [12.5, 11.8]], hole)
}

fn documents() -> Shape {
    let col = ink(Cat::Documents);
    let line = c(0x08343a);
    Shape::new()
        .fill(vec![[3.0, 1.4], [10.2, 1.4], [13.2, 4.4], [13.2, 14.6], [3.0, 14.6]], col)
        .fill(vec![[10.2, 1.4], [10.2, 4.4], [13.2, 4.4]], c(0x9cf3f6))
        .stroke(vec![[5.0, 7.4], [11.0, 7.4]], 1.05, line)
        .stroke(vec![[5.0, 10.0], [11.0, 10.0]], 1.05, line)
}

fn music() -> Shape {
    let col = ink(Cat::Music);
    Shape::new()
        .fill(ngon(6.1, 11.6, 2.5, 10), col)
        .fill(vec![[7.6, 3.0], [9.3, 3.0], [9.3, 11.2], [7.6, 11.2]], col)
        .fill(vec![[9.0, 3.0], [13.2, 2.0], [13.2, 4.2], [9.0, 5.2]], col)
        .fill(diamond(12.6, 7.4, 0.7, 1.8), col)
}

fn system() -> Shape {
    let col = ink(Cat::System);
    let hole = c(0x17181b);
    // Teeth first, then the disk covers their inner half, then the hole. What remains is a cog.
    let mut s = Shape::new();
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        let (sa, ca) = a.sin_cos();
        s = s.stroke(vec![[8.0 + ca * 1.5, 8.0 + sa * 1.5], [8.0 + ca * 7.1, 8.0 + sa * 7.1]], 2.15, col);
    }
    s.fill(ngon(8.0, 8.0, 4.15, 20), col).fill(ngon(8.0, 8.0, 1.7, 14), hole)
}

fn caches() -> Shape {
    let col = ink(Cat::Caches);
    let top = c(0xffc48a);
    let side = c(0xb85e14);
    Shape::new()
        .fill(vec![[8.0, 2.2], [14.2, 5.4], [8.0, 8.6], [1.8, 5.4]], top)
        .fill(vec![[1.8, 5.4], [8.0, 8.6], [8.0, 14.2], [1.8, 11.0]], col)
        .fill(vec![[8.0, 8.6], [14.2, 5.4], [14.2, 11.0], [8.0, 14.2]], side)
}

fn trash() -> Shape {
    let col = ink(Cat::Trash);
    let lid = c(0xffb4b8);
    Shape::new()
        .fill(roundish(3.2, 5.4, 9.6, 9.0, 1.3), col)
        .fill(vec![[2.4, 4.2], [13.6, 4.2], [13.6, 5.8], [2.4, 5.8]], lid)
        .fill(vec![[6.4, 2.2], [9.6, 2.2], [9.6, 4.2], [6.4, 4.2]], lid)
        .stroke(vec![[6.2, 7.2], [6.2, 12.2]], 1.05, c(0x4a1016))
        .stroke(vec![[9.8, 7.2], [9.8, 12.2]], 1.05, c(0x4a1016))
}

fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, n: usize) -> Vec<[f32; 2]> {
    (0..=n)
        .map(|i| {
            let t = a0 + (a1 - a0) * i as f32 / n as f32;
            [cx + r * t.cos(), cy + r * t.sin()]
        })
        .collect()
}

fn ngon(cx: f32, cy: f32, r: f32, n: usize) -> Vec<[f32; 2]> {
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32 * std::f32::consts::TAU - 1.2;
            [cx + r * t.cos(), cy + r * t.sin()]
        })
        .collect()
}

fn diamond(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<[f32; 2]> {
    vec![[cx, cy - ry], [cx + rx, cy], [cx, cy + ry], [cx - rx, cy]]
}

fn tri(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> Vec<[f32; 2]> {
    vec![a, b, c]
}

/// A rectangle with the corners cut, close enough to a rounded rect at this size.
fn roundish(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<[f32; 2]> {
    let r = r.min(w * 0.5).min(h * 0.5);
    vec![
        [x + r, y],
        [x + w - r, y],
        [x + w, y + r],
        [x + w, y + h - r],
        [x + w - r, y + h],
        [x + r, y + h],
        [x, y + h - r],
        [x, y + r],
    ]
}
