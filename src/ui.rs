//! The single card: total, scanning line, dot-matrix bar, legend, and the clear action.

use rune::color::Color;
use rune::{Cursor, Cx, El, Shape, TextStyle, col, el, row, spacer, text};

use crate::bar;
use crate::demo;
use crate::icons;
use crate::model::{self, Crossfade, Footer, FooterSnap, State, StatusId, StatusSnap, BAR_Y, CARD_H, CARD_W, DIVIDER_Y, HEADER_Y, LEGEND_Y, PAD_X, STATUS_Y};
use crate::scan::Cat;

pub fn view(s: &State, _: &mut Cx<State>) -> El {
    // The window is the card. A titled window cannot show transparent rounded corners, so the
    // root is the card color edge to edge.
    el().key("root").w_full().h_full().fill(c(0x17181b))
        .on_pointer_move(|st: &mut State, p| model::set_pointer(st, p.x, p.y))
        .child(card(s))
}

fn card(s: &State) -> El {
    let mut card = el()
        .key("card")
        .w_full()
        .h_full()
        .fill(c(0x17181b))
        .child(pixels(s))
        .child(burning(s))
        .child(header(s))
        .child(status(s))
        .child(legend(s))
        .child(el().absolute(0.0, DIVIDER_Y).size(CARD_W, 1.0).fill(ca(0xffffff, 0.08)))
        .child(footer(s));
    card = hover_targets(card, s);
    if s.confirm.is_some() {
        card = card.child(confirm(s));
    }
    if rune::is_headless() {
        if let Some((x, y)) = s.pointer {
            card = card.child(cursor_at(x, y));
        }
    }
    card
}

/// A small arrow for scripted headless frames. The system pointer covers the live window.
fn cursor_at(x: f32, y: f32) -> El {
    let arrow = |o: f32| -> Vec<[f32; 2]> {
        vec![
            [1.0 + o, 1.0 + o],
            [1.0 + o, 17.0 + o],
            [5.2 + o, 13.2 + o],
            [8.4 + o, 20.2 + o],
            [11.2 + o, 18.8 + o],
            [7.8 + o, 12.0 + o],
            [13.4 + o, 11.6 + o],
        ]
    };
    el().absolute(x, y).size(16.0, 22.0).shape(
        Shape::new().fill(arrow(0.8), c(0x111214)).fill(arrow(0.0), c(0xf4f5f7)),
    )
}

fn burning(s: &State) -> El {
    let Some((edge, amount)) = model::burn_area(s) else { return el(); };
    // Fire rises from the bottom of the segment being cleared, never the whole bar.
    rune::effect(crate::fire::FIRE).key("clear-fire")
        .effect_data([bar::BAR_H, 1.8 * amount, 0.6 * amount, 0.0])
        .absolute(PAD_X + edge.x, BAR_Y + bar::BAR_H - 3.0)
        .size(edge.w, 3.0).effect_bleed(bar::BAR_H + 24.0)
}

fn pixels(s: &State) -> El {
    el().pixels(&s.pixels)
        .absolute(PAD_X - bar::BLEED_X, BAR_Y - bar::BLEED_TOP)
        .size(bar::BAR_W + bar::BLEED_X * 2.0, bar::BAR_H + bar::BLEED_TOP)
}

/// Legend cells and bar segments tile their regions, so a move across a visual gap stays on a category.
fn hover_targets(mut card: El, s: &State) -> El {
    for row in 0..3 {
        for col in 0..3 {
            let (x, y, w, h) = model::legend_hit_rect(row, col);
            let cat = model::LEGEND[row][col];
            card = card.child(
                el().absolute(x, y).size(w, h).hit().cursor(Cursor::Pointer)
                    .on_hover_change(move |st: &mut State, inside, _| model::note_hit(st, cat, inside)),
            );
        }
    }
    for (cat, x0, x1) in model::bar_hit_spans(&s.live, s.capacity) {
        card = card.child(
            el().absolute(x0, BAR_Y).size((x1 - x0).max(1.0), bar::BAR_H).hit().cursor(Cursor::Pointer)
                .on_hover_change(move |st: &mut State, inside, _| model::note_hit(st, cat, inside)),
        );
    }
    card
}

fn header(s: &State) -> El {
    let total = demo::sum(&s.live);
    let cap = if s.capacity > 0.0 {
        format!("of {} GB used", demo::fmt_cap(s.capacity))
    } else {
        "of … GB used".into()
    };
    row().absolute(PAD_X, HEADER_Y).size(bar::BAR_W, 44.0).items_end()
        .child(label(format!("{} GB", demo::fmt_gb(total)), 34.0, 500.0, 0xf5f6f8))
        .child(label(cap, 20.0, 400.0, 0x8e9099).mb(6.0).ml(12.0))
        .child(spacer())
        .child(
            row().key("rescan").role("button").label("Rescan").h(32.0).px(10.0).gap(6.0).center().self_center()
                .radius(8.0)
                .cursor(Cursor::Pointer)
                .hover(ca(0xffffff, 0.06))
                .on_press(model::press_rescan)
                .child(icons::rescan(21.0))
                .child(label("Rescan", 19.0, 400.0, 0xa5a7b0)),
        )
}

fn status(s: &State) -> El {
    let slot = el().absolute(PAD_X, STATUS_Y).size(bar::BAR_W, 28.0);
    stack(slot, &s.status_fade, bar::BAR_W, 24.0, status_line)
}

fn status_line(snap: &StatusSnap) -> El {
    match snap.id {
        StatusId::Quiet => row().size(bar::BAR_W, 24.0).center_y().child(label(&snap.title, 19.0, 400.0, 0x8e9099)),
        StatusId::Notice => row().size(bar::BAR_W, 24.0).center_y().child(label(&snap.title, 19.0, 400.0, 0xe7b4ae)),
        StatusId::Calculating | StatusId::Clearing | StatusId::Emptying => row().size(bar::BAR_W, 24.0).gap(8.0).center_y()
            .child(label(&snap.title, 19.0, 400.0, 0xc8cad1))
            .child(text(snap.detail.clone()).font(TextStyle::new(17.0, 400.0).mono()).color(c(0x8b8d96))),
        StatusId::Hover(_) => {
            let cat = snap.cat.unwrap_or(Cat::Apps);
            row().size(bar::BAR_W, 24.0).gap(8.0).center_y()
                .child(icons::category(cat, 20.0))
                .child(label(&snap.title, 19.0, 450.0, 0xf4f5f7))
                .child(label(&snap.detail, 19.0, 400.0, 0xb4b6be))
                .child(label(&snap.blurb, 19.0, 400.0, 0x8e9099).shrink())
        }
    }
}

fn legend(s: &State) -> El {
    let mut list = col().absolute(PAD_X, LEGEND_Y).size(bar::BAR_W, model::LEGEND_H).gap(6.0);
    for group in model::LEGEND {
        let mut line = row().w_full().h(40.0).gap(16.0);
        for cat in group {
            line = line.child(cell(s, cat).grow(1.0));
        }
        list = list.child(line);
    }
    list
}

fn cell(s: &State, cat: Cat) -> El {
    let (mine, others) = model::hover_pair(s, cat);
    // Other rows fade while this one's highlight eases in. A hair of fill keeps the row hittable at rest.
    let bg = 0.001 + 0.074 * mine;
    let fade = 1.0 - 0.42 * others;
    row().key(format!("cat-{}", cat.key())).h(40.0).px(2.0).gap(12.0).center_y().radius(10.0)
        .fill(ca(0xffffff, bg))
        .child(icons::category(cat, 22.0).opacity(fade))
        .child(label(cat.name(), 19.0, 400.0, 0xf3f4f6).opacity(fade))
        .child(spacer())
        .child(label(format!("{} GB", demo::fmt_gb(s.live[cat.ix()])), 19.0, 400.0, 0x8d8f98).opacity(fade))
}

fn footer(s: &State) -> El {
    let h = CARD_H - DIVIDER_Y - 1.0;
    let slot = el().absolute(0.0, DIVIDER_Y + 1.0).size(CARD_W, h);
    stack(slot, &s.footer_fade, CARD_W, h, footer_line)
}

fn footer_line(snap: &FooterSnap) -> El {
    let icon = match snap.kind {
        Footer::Scanning => icons::sparkle(20.0, 0x9aa0aa),
        Footer::Caches => icons::category(Cat::Caches, 22.0),
        Footer::Trash => icons::category(Cat::Trash, 22.0),
        Footer::Done => icons::sparkle(20.0, 0x8fd9b0),
    };
    let mut line = row().size(CARD_W, CARD_H - DIVIDER_Y - 1.0).px(PAD_X).gap(12.0).center_y()
        .child(icon)
        .child(label(&snap.message, 19.0, 400.0, if snap.kind == Footer::Scanning || snap.kind == Footer::Done { 0x9a9ca6 } else { 0xb7b9c0 }).shrink());
    if let Some(name) = snap.button {
        let press = if name == "Empty Trash" { model::press_trash } else { model::press_clear };
        let key = if name == "Empty Trash" { "empty-trash" } else { "clear-caches" };
        line = line.child(spacer()).child(action(key, name, press));
    }
    line
}

/// Overlapping copies of a line. The outgoing one rises a few pixels as it fades; the incoming one settles down into place.
fn stack<T: Clone>(host: El, fade: &Crossfade<T>, _w: f32, _h: f32, draw: fn(&T) -> El) -> El {
    if let Some(inc) = &fade.incoming {
        let (inc_dy, base_dy) = fade.slide();
        let base_op = (1.0 - fade.mix).clamp(0.0, 1.0);
        let inc_op = fade.mix.clamp(0.0, 1.0);
        // The clearer copy is painted last so it keeps the clicks.
        if inc_op >= base_op {
            host.child(draw(&fade.base).opacity(base_op).absolute(0.0, base_dy))
                .child(draw(inc).opacity(inc_op).absolute(0.0, inc_dy))
        } else {
            host.child(draw(inc).opacity(inc_op).absolute(0.0, inc_dy))
                .child(draw(&fade.base).opacity(base_op).absolute(0.0, base_dy))
        }
    } else {
        host.child(draw(&fade.base).absolute(0.0, 0.0))
    }
}

fn action(key: &str, name: &str, on_press: fn(&mut State)) -> El {
    row().key(key).role("button").label(name).h(48.0).px(17.0).radius(11.0).center()
        .fill(c(0x2c2e33))
        .hover(c(0x3a3c42))
        .border(1.0, ca(0xffffff, 0.10))
        .cursor(Cursor::Pointer)
        .press_scale(0.97)
        .on_press(on_press)
        .child(label(name, 19.0, 450.0, 0xf4f5f7))
}

fn confirm(s: &State) -> El {
    let (title, body, yes) = match s.confirm {
        Some(crate::scan::Purge::Trash) => (
            "Empty Trash?",
            "This permanently deletes everything inside ~/.Trash. Nothing else is touched.",
            "Empty Trash",
        ),
        _ => (
            "Clear caches?",
            "This permanently deletes everything inside ~/Library/Caches. Apps rebuild caches when they need them.",
            "Clear caches",
        ),
    };
    let dialog_w = 540.0;
    let dialog_h = 230.0;
    el().absolute(0.0, 0.0).size(CARD_W, CARD_H).fill(ca(0x000000, 0.55)).child(
        col().absolute((CARD_W - dialog_w) / 2.0, (CARD_H - dialog_h) / 2.0)
            .size(dialog_w, dialog_h)
            .padding(22.0)
            .gap(12.0)
            .radius(16.0)
            .fill(c(0x222328))
            .border(1.0, ca(0xffffff, 0.10))
            .child(label(title, 18.0, 650.0, 0xf4f5f7))
            .child(label(body, 17.0, 400.0, 0xb7b9c0).w_full().wrap())
            .child(spacer())
            .child(
                row().w_full().gap(8.0)
                    .child(spacer())
                    .child(action("confirm-no", "Cancel", model::press_cancel).fill(c(0x2c2e33)))
                    .child(action("confirm-yes", yes, model::press_confirm)),
            ),
    )
}

fn label(value: impl AsRef<str>, size: f32, weight: f32, color: u32) -> El {
    text(value.as_ref()).font(TextStyle::new(size, weight)).color(c(color))
}

fn c(hex: u32) -> Color {
    rune::color::hex(&format!("#{hex:06x}")).expect("hex colour")
}

fn ca(hex: u32, alpha: f32) -> Color {
    c(hex).alpha(alpha)
}
