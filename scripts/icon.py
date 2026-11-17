#!/usr/bin/env python3
"""Draws the app icon, assets/icon.png: the dot-matrix disk bar on a dark macOS tile.

Standard library only. Run from anywhere: python3 scripts/icon.py
The tile follows Apple's icon grid (an 824 pt body on a 1024 canvas, with a soft shadow).
Colours are the categories' bar tones from src/bar.rs, in the same left-to-right order.
"""

import math
import os
import struct
import zlib

S = 1024
BODY = (100.0, 100.0, 924.0, 924.0)
BODY_R = 185.0
BAR = (172.0, 286.0, 852.0, 738.0)
BAR_R = 76.0
PITCH = 36.0
CELL = 26.0
CELL_R = 4.0

# (base, spark, glow) per category, from bar.rs TONE: Apps, Developer, Photos, Movies,
# Documents, Music, Caches.
TONES = [
    ((46, 118, 230), (198, 226, 255), (72, 156, 255)),
    ((32, 186, 104), (198, 255, 214), (52, 230, 132)),
    ((226, 58, 176), (255, 196, 232), (255, 78, 196)),
    ((148, 72, 236), (226, 196, 255), (178, 96, 255)),
    ((32, 186, 196), (186, 255, 248), (52, 220, 230)),
    ((224, 176, 42), (255, 236, 168), (255, 204, 64)),
    ((228, 118, 36), (255, 214, 160), (255, 146, 48)),
]
# Columns each category fills, left to right. The rest of the bar is free space.
RUNS = [4, 2, 2, 2, 1, 1, 1]


def clamp(v, lo=0.0, hi=1.0):
    return lo if v < lo else hi if v > hi else v


def mix(a, b, t):
    t = clamp(t)
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def sd_round(px, py, box, r):
    x0, y0, x1, y1 = box
    hx, hy = (x1 - x0) / 2, (y1 - y0) / 2
    qx = abs(px - (x0 + hx)) - hx + r
    qy = abs(py - (y0 + hy)) - hy + r
    ax, ay = max(qx, 0.0), max(qy, 0.0)
    return min(max(qx, qy), 0.0) + math.hypot(ax, ay) - r


def cell_hash(c, r):
    h = (c * 374761393 + r * 668265263) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return h ^ (h >> 16)


def grid():
    gap = PITCH - CELL
    x0, y0, x1, y1 = BAR
    cols = int((x1 - x0 - 2 * 18 + gap) // PITCH)
    rows = int((y1 - y0 - 2 * 18 + gap) // PITCH)
    ox = x0 + ((x1 - x0) - (cols * PITCH - gap)) / 2
    oy = y0 + ((y1 - y0) - (rows * PITCH - gap)) / 2
    return cols, rows, ox, oy


COLS, ROWS, OX, OY = grid()
USED = sum(RUNS)
OWNER = [i for i, n in enumerate(RUNS) for _ in range(n)]
# Segment edges sit in the middle of the gap between columns.
EDGES = []
start = 0
for i, n in enumerate(RUNS):
    left = BAR[0] if start == 0 else OX + start * PITCH - (PITCH - CELL) / 2
    right = OX + (start + n) * PITCH - (PITCH - CELL) / 2
    EDGES.append((left, right))
    start += n
FRONTIER = EDGES[-1][1]


def cell_colour(c, r):
    """Colour and lit amount of one cell, or None for free space."""
    if c >= USED:
        return None
    base, spark, _ = TONES[OWNER[c]]
    h = cell_hash(c, r)
    rgb = spark if h % 7 == 0 else mix(base, spark, 0.55) if h % 4 == 0 else base
    edge_row = r == 0 or r == ROWS - 1
    lit = 0.95 if edge_row else 0.9 if h % 100 < 46 else 0.16
    # The column at the scan frontier runs white hot, as the bar does while scanning.
    if c == USED - 1:
        rgb = mix(rgb, (255, 255, 255), 0.35)
    return mix((24, 25, 28), rgb, lit)


def over(dst, rgb, a):
    a = clamp(a)
    out_a = a + dst[3] * (1 - a)
    if out_a <= 0:
        return (0.0, 0.0, 0.0, 0.0)
    return tuple((rgb[i] * a + dst[i] * dst[3] * (1 - a)) / out_a for i in range(3)) + (out_a,)


def add(dst, rgb, a):
    return tuple(min(255.0, dst[i] + rgb[i] * a) for i in range(3)) + (dst[3],)


def pixel(x, y):
    px, py = x + 0.5, y + 0.5
    out = (0.0, 0.0, 0.0, 0.0)

    # Drop shadow under the tile.
    d_shadow = sd_round(px, py - 12, BODY, BODY_R)
    if d_shadow < 40:
        out = over(out, (0, 0, 0), 0.30 * clamp(1 - (d_shadow + 6) / 46) ** 2)

    d_body = sd_round(px, py, BODY, BODY_R)
    body = clamp(0.5 - d_body)
    if body <= 0:
        return out
    t = (py - BODY[1]) / (BODY[3] - BODY[1])
    tile = mix((38, 39, 45), (19, 20, 23), t)
    out = over(out, tile, body)
    # A faint light rim, brighter along the top edge.
    if -2.5 < d_body < 0.5:
        rim = clamp(1 - abs(d_body + 1.0) / 1.5) * (0.06 + 0.12 * (1 - t))
        out = over(out, (255, 255, 255), rim * body)

    d_bar = sd_round(px, py, BAR, BAR_R)

    # Coloured bloom just outside the used part of the bar, blending from one segment's
    # colour into the next and fading out past the scan frontier.
    if 0 < d_bar < 40 and px < FRONTIER + 40:
        weights = [math.exp(-(((px - (l + r) / 2) / 36.0) ** 2)) + 1e-6 for l, r in EDGES]
        total = sum(weights)
        glow = tuple(sum(TONES[i][2][k] * w for i, w in enumerate(weights)) / total for k in range(3))
        fade = clamp((FRONTIER + 24 - px) / 48)
        g = math.exp(-(d_bar * d_bar) / (2 * 12.0 * 12.0)) * 0.26 * fade
        out = add(out, glow, g * body)

    if d_bar < 1.0:
        out = over(out, (16, 17, 20), clamp(0.5 - d_bar))
        # The capsule's thin neutral rim.
        rim = clamp(1 - abs(d_bar + 0.8) / 1.6)
        out = over(out, (255, 255, 255), 0.16 * rim)

    if d_bar < 0:
        # Scan frontier glow.
        dx = px - FRONTIER
        if -30 < dx < 30:
            out = add(out, (255, 255, 255), 0.42 * math.exp(-(dx * dx) / (2 * 6.0 * 6.0)))

        c = int((px - OX) // PITCH)
        r = int((py - OY) // PITCH)
        if 0 <= c < COLS and 0 <= r < ROWS:
            cx0 = OX + c * PITCH
            cy0 = OY + r * PITCH
            ccx, ccy = cx0 + CELL / 2, cy0 + CELL / 2
            if sd_round(ccx, ccy, BAR, BAR_R) < -10:
                rgb = cell_colour(c, r)
                if rgb is None:
                    # Free space: a small dim dot in the middle of the cell.
                    dot = (ccx - 5, ccy - 5, ccx + 5, ccy + 5)
                    cov = clamp(0.5 - sd_round(px, py, dot, 2.0))
                    out = over(out, (44, 46, 52), cov)
                else:
                    cov = clamp(0.5 - sd_round(px, py, (cx0, cy0, cx0 + CELL, cy0 + CELL), CELL_R))
                    out = over(out, rgb, cov)

        # Each segment's outline in its own glow colour.
        for i, (l, rgt) in enumerate(EDGES):
            if l - 3 < px < rgt + 3:
                d_seg = max(sd_round(px, py, (l, BAR[1], rgt, BAR[3]), 0.0), d_bar)
                stroke = clamp(1 - abs(d_seg + 1.4) / 1.6)
                if stroke > 0:
                    out = over(out, TONES[i][2], 0.62 * stroke)
    return out


def png(width, height, rows):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + row for row in rows)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def main():
    rows = []
    for y in range(S):
        line = bytearray()
        for x in range(S):
            r, g, b, a = pixel(x, y)
            line += bytes((round(r), round(g), round(b), round(a * 255)))
        rows.append(bytes(line))
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "icon.png")
    with open(out, "wb") as f:
        f.write(png(S, S, rows))
    print(os.path.normpath(out), COLS, "x", ROWS, "cells")


if __name__ == "__main__":
    main()
