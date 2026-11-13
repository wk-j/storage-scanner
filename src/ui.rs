//! The single card: total, scanning line, dot-matrix bar, legend, and the clear action.

use rune::color::Color;
use rune::{Cursor, Cx, El, TextStyle, col, el, row, spacer, text};

use crate::bar;
use crate::demo;
use crate::icons;
use crate::model::{self, Footer, Readout, State, CARD_H, CARD_W};
use crate::scan::Cat;

const PAD_X: f32 = 30.0;
const HEADER_Y: f32 = 26.0;
const STATUS_Y: f32 = 78.0;
const BAR_Y: f32 = 118.0;
const LEGEND_Y: f32 = 272.0;
const DIVIDER_Y: f32 = 468.0;

pub fn view(s: &State, _: &mut Cx<State>) -> El {
    // The window is the card. A titled window cannot show transparent rounded corners, so the
    // root is the card color edge to edge.
    el().key("root").w_full().h_full().fill(c(0x17181b)).child(card(s))
}

fn card(s: &State) -> El {
    let mut card = el()
        .key("card")
        .w_full()
        .h_full()
        .fill(c(0x17181b))
        .child(pixels(s))
        .child(header(s))
        .child(status(s))
        .child(legend(s))
        .child(el().absolute(0.0, DIVIDER_Y).size(CARD_W, 1.0).fill(ca(0xffffff, 0.08)))
        .child(footer(s));
    card = segment_hits(card, s);
    if s.confirm.is_some() {
        card = card.child(confirm(s));
    }
    card
}

fn pixels(s: &State) -> El {
    el().pixels(&s.pixels)
        .absolute(PAD_X - bar::BLEED_X, BAR_Y - bar::BLEED_TOP)
        .size(bar::BAR_W + bar::BLEED_X * 2.0, bar::BAR_H + bar::BLEED_TOP)
}

fn segment_hits(mut card: El, s: &State) -> El {
    let edges = bar::edges(&s.live, s.capacity);
    for cat in Cat::ALL {
        let edge = edges[cat.ix()];
        if edge.w < 6.0 {
            continue;
        }
        card = card.child(
            el().absolute(PAD_X + edge.x, BAR_Y)
                .size(edge.w, bar::BAR_H)
                .fill(ca(0xffffff, 0.001))
                .on_hover_change(move |st: &mut State, inside, _| model::set_hover(st, cat, inside)),
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
        .child(label(format!("{} GB", demo::fmt_gb(total)), 34.0, 700.0, 0xf5f6f8))
        .child(label(cap, 15.5, 500.0, 0x8e9099).mb(6.0).ml(12.0))
        .child(spacer())
        .child(
            row().key("rescan").role("button").label("Rescan").h(32.0).px(10.0).gap(6.0).center().self_center()
                .radius(8.0)
                .cursor(Cursor::Pointer)
                .hover(ca(0xffffff, 0.06))
                .on_press(model::press_rescan)
                .child(icons::rescan(15.0))
                .child(label("Rescan", 13.5, 550.0, 0xa5a7b0)),
        )
}

fn status(s: &State) -> El {
    let line = match &s.readout {
        Readout::Calculating(path) => verb("Calculating", path),
        Readout::Clearing(path) => verb("Clearing caches", path),
        Readout::Emptying(path) => verb("Emptying the Trash", path),
        Readout::Notice(msg) => row().w_full().h(24.0).center_y().child(label(msg, 13.5, 500.0, 0xe7b4ae)),
        Readout::Quiet => row().w_full().h(24.0).center_y().child(label(
            "Hover a category to see what's using space.",
            13.5,
            450.0,
            0x8e9099,
        )),
        Readout::Hover => hover_line(s),
    };
    line.absolute(PAD_X, STATUS_Y).size(bar::BAR_W, 24.0)
}

fn verb(name: &str, path: &str) -> El {
    row().w_full().h(24.0).gap(8.0).center_y()
        .child(label(name, 13.5, 560.0, 0xc8cad1))
        .child(text(path).font(TextStyle::new(13.0, 450.0).mono()).color(c(0x8b8d96)))
}

fn hover_line(s: &State) -> El {
    let cat = s.hover.unwrap_or(Cat::Apps);
    let sentence = if s.demo {
        demo::blurb(cat).to_string()
    } else {
        crate::scan::blurb(cat, &s.stats[cat.ix()])
    };
    row().w_full().h(24.0).gap(8.0).center_y()
        .child(icons::category(cat, 14.0))
        .child(label(cat.name(), 13.5, 650.0, 0xf4f5f7))
        .child(label(format!("{} GB", demo::fmt_gb(s.live[cat.ix()])), 13.5, 500.0, 0xb4b6be))
        .child(label(sentence, 13.5, 450.0, 0x8e9099).shrink())
}

fn legend(s: &State) -> El {
    let rows = [
        [Cat::Apps, Cat::Developer, Cat::Photos],
        [Cat::Movies, Cat::Documents, Cat::Music],
        [Cat::System, Cat::Caches, Cat::Trash],
    ];
    let mut list = col().absolute(PAD_X, LEGEND_Y).size(bar::BAR_W, 132.0).gap(6.0);
    for group in rows {
        let mut line = row().w_full().h(40.0).gap(16.0);
        for cat in group {
            line = line.child(cell(s, cat).grow(1.0));
        }
        list = list.child(line);
    }
    list
}

fn cell(s: &State, cat: Cat) -> El {
    let on = s.hover == Some(cat);
    row().key(format!("cat-{}", cat.key())).h(40.0).px(10.0).gap(8.0).center_y().radius(10.0)
        .fill(if on { ca(0xffffff, 0.075) } else { ca(0xffffff, 0.001) })
        .cursor(Cursor::Pointer)
        .on_hover_change(move |st: &mut State, inside, _| model::set_hover(st, cat, inside))
        .child(icons::category(cat, 16.0))
        .child(label(cat.name(), 14.5, 600.0, 0xf3f4f6))
        .child(spacer())
        .child(label(format!("{} GB", demo::fmt_gb(s.live[cat.ix()])), 13.5, 500.0, 0x8d8f98))
}

fn footer(s: &State) -> El {
    let h = CARD_H - DIVIDER_Y - 1.0;
    let mut line = row().absolute(0.0, DIVIDER_Y + 1.0).size(CARD_W, h).px(PAD_X).gap(12.0).center_y();
    line = match s.footer {
        Footer::Scanning => line
            .child(icons::sparkle(15.0, 0x9aa0aa))
            .child(label("Looking for things you can clear", 13.5, 450.0, 0x9a9ca6)),
        Footer::Caches => line
            .child(icons::category(Cat::Caches, 16.0))
            .child(label(
                format!(
                    "Caches are using {} GB. Apps rebuild them when they need them.",
                    demo::fmt_gb(s.held_caches)
                ),
                13.5,
                450.0,
                0xb7b9c0,
            ).shrink())
            .child(spacer())
            .child(action("clear-caches", "Clear caches", model::press_clear)),
        Footer::Trash => line
            .child(icons::category(Cat::Trash, 16.0))
            .child(label(
                format!(
                    "The Trash is holding {} GB of things you've already deleted.",
                    demo::fmt_gb(s.held_trash)
                ),
                13.5,
                450.0,
                0xb7b9c0,
            ).shrink())
            .child(spacer())
            .child(action("empty-trash", "Empty Trash", model::press_trash)),
        Footer::Done => line
            .child(icons::sparkle(15.0, 0x8fd9b0))
            .child(label("Caches and the Trash are clear.", 13.5, 450.0, 0x9a9ca6)),
    };
    line
}

fn action(key: &str, name: &str, on_press: fn(&mut State)) -> El {
    row().key(key).role("button").label(name).h(34.0).px(14.0).radius(9.0).center()
        .fill(c(0x2c2e33))
        .hover(c(0x3a3c42))
        .border(1.0, ca(0xffffff, 0.10))
        .cursor(Cursor::Pointer)
        .press_scale(0.97)
        .on_press(on_press)
        .child(label(name, 13.5, 600.0, 0xf4f5f7))
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
    let dialog_w = 440.0;
    let dialog_h = 196.0;
    el().absolute(0.0, 0.0).size(CARD_W, CARD_H).fill(ca(0x000000, 0.55)).child(
        col().absolute((CARD_W - dialog_w) / 2.0, (CARD_H - dialog_h) / 2.0)
            .size(dialog_w, dialog_h)
            .padding(22.0)
            .gap(12.0)
            .radius(16.0)
            .fill(c(0x222328))
            .border(1.0, ca(0xffffff, 0.10))
            .child(label(title, 18.0, 650.0, 0xf4f5f7))
            .child(label(body, 13.5, 450.0, 0xb7b9c0))
            .child(spacer())
            .child(
                row().w_full().gap(8.0)
                    .child(spacer())
                    .child(action("confirm-no", "Cancel", model::press_cancel).fill(c(0x2c2e33)))
                    .child(action("confirm-yes", yes, model::press_confirm)),
            ),
    )
}

fn label(value: impl Into<String>, size: f32, weight: f32, color: u32) -> El {
    text(value).font(TextStyle::new(size, weight)).color(c(color))
}

fn c(hex: u32) -> Color {
    rune::color::hex(&format!("#{hex:06x}")).expect("hex colour")
}

fn ca(hex: u32, alpha: f32) -> Color {
    c(hex).alpha(alpha)
}
