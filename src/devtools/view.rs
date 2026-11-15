//! The developer tools tab body, drawn like the overview: the dot-matrix bar with one segment per
//! tool family, the status line, a 3×3 legend, the divider, and a footer.
//!
//! The legend lists families until one is clicked; then it lists that family's items. Clicking
//! an item puts its cleanup command in the footer with a Copy button. Nothing here deletes files.

use rune::{Cursor, El, ScrollSpec, TextStyle, Tx, col, el, row, spacer, text};

use super::tab::{Foot, FootId, Hit, Status, StatusId};
use super::{fmt_size, Group, Row, MIN_ROW};
use crate::bar;
use crate::icons;
use crate::model::{self, State, BAR_Y, CARD_W, CARD_H, DIVIDER_Y, LEGEND_GAP_X, LEGEND_GAP_Y, LEGEND_H, LEGEND_ROW, LEGEND_Y, PAD_X, STATUS_Y};
use crate::ui::{c, ca, label, stack};

/// Legend order, row by row: the bar's slot order, as the overview's legend is.
const LEGEND: [[Group; 3]; 3] = [
    [Group::Docker, Group::Node, Group::Apple],
    [Group::Dotnet, Group::Python, Group::Homebrew],
    [Group::Other, Group::Rust, Group::Java],
];
const COPY_W: f32 = 84.0;
const BACK_W: f32 = 84.0;

/// Everything under the header, in the overview's layers.
pub fn body(mut card: El, s: &State) -> El {
    card = card
        .child(pixels(s))
        .child(status(s))
        .child(legend(s))
        .child(el().absolute(0.0, DIVIDER_Y).size(CARD_W, 1.0).fill(ca(0xffffff, 0.08)))
        .child(footer(s));
    bar_targets(card, s)
}

fn pixels(s: &State) -> El {
    el().pixels(&s.dev.pixels)
        .absolute(PAD_X - bar::BLEED_X, BAR_Y - bar::BLEED_TOP)
        .size(bar::BAR_W + bar::BLEED_X * 2.0, bar::BAR_H + bar::BLEED_TOP)
}

/// One hit span per family segment, abutting the next, as the overview's bar has.
fn bar_targets(mut card: El, s: &State) -> El {
    for (cat, x0, x1) in model::bar_hit_spans(&s.dev.live, s.dev.cap.max(1.0)) {
        let g = Group::ALL[cat.ix()];
        card = card.child(
            el().key(format!("dev-bar-{}", g.key())).absolute(x0, BAR_Y).size((x1 - x0).max(1.0), bar::BAR_H).hit()
                .cursor(Cursor::Pointer)
                .on_hover_change(move |st: &mut State, inside, tx| {
                    st.dev.note_hit(Hit::Group(g), inside);
                    model::repoll(tx);
                })
                .on_press_tx(move |st: &mut State, tx| {
                    st.dev.press_group(g);
                    model::repoll(tx);
                }),
        );
    }
    card
}

fn status(s: &State) -> El {
    let slot = el().absolute(PAD_X, STATUS_Y).size(bar::BAR_W, 22.0);
    stack(slot, &s.dev.status_fade, bar::BAR_W, 20.0, status_line)
}

fn status_line(snap: &Status) -> El {
    let line = row().size(bar::BAR_W, 20.0).gap(6.0).center_y();
    match &snap.id {
        StatusId::Quiet => line.child(label(&snap.title, 14.0, 400.0, 0x8e9099)),
        StatusId::Measuring => line
            .child(label(&snap.title, 14.0, 400.0, 0xc8cad1))
            .child(text(snap.detail.clone()).font(TextStyle::new(12.5, 400.0).mono()).color(c(0x8b8d96)).truncate_head()),
        StatusId::Group(_) | StatusId::Item(_) => {
            let g = snap.group.unwrap_or(Group::Other);
            let blurb = label(&snap.blurb, 14.0, 400.0, 0x8e9099);
            let blurb = if matches!(snap.id, StatusId::Item(_)) { blurb.truncate_middle() } else { blurb.truncate() };
            line.child(icons::dev_group(g, 15.0))
                .child(label(&snap.title, 14.0, 450.0, 0xf4f5f7))
                .child(label(&snap.detail, 14.0, 400.0, 0xb4b6be))
                .child(blurb.grow(1.0))
        }
    }
}

fn legend(s: &State) -> El {
    match s.dev.group {
        Some(g) if s.dev.snap.done => items(s, g),
        _ => groups(s),
    }
}

/// The families, laid out and faded exactly as the overview's legend, with hit cells that tile it.
fn groups(s: &State) -> El {
    let mut list = col().absolute(PAD_X, LEGEND_Y).size(bar::BAR_W, LEGEND_H).gap(LEGEND_GAP_Y);
    for line_groups in LEGEND {
        let mut line = row().w_full().h(LEGEND_ROW).gap(LEGEND_GAP_X);
        for g in line_groups {
            line = line.child(group_cell(s, g).grow(1.0));
        }
        list = list.child(line);
    }
    let mut host = el().absolute(0.0, 0.0).size(CARD_W, CARD_H).child(list);
    for (r, line_groups) in LEGEND.iter().enumerate() {
        for (k, g) in line_groups.iter().copied().enumerate() {
            let (x, y, w, h) = model::legend_hit_rect(r, k);
            host = host.child(
                el().key(format!("dev-group-{}", g.key())).role("button").label(g.name())
                    .absolute(x, y).size(w, h).hit().cursor(Cursor::Pointer)
                    .on_hover_change(move |st: &mut State, inside, tx| {
                        st.dev.note_hit(Hit::Group(g), inside);
                        model::repoll(tx);
                    })
                    .on_press_tx(move |st: &mut State, tx| {
                        st.dev.press_group(g);
                        model::repoll(tx);
                    }),
            );
        }
    }
    host
}

fn group_cell(s: &State, g: Group) -> El {
    let mine = s.dev.amt[g.ix()];
    let others = s.dev.amt.iter().enumerate().filter(|(i, _)| *i != g.ix()).fold(0.0f32, |m, (_, v)| m.max(*v));
    let bg = 0.001 + 0.074 * mine;
    let fade = 1.0 - 0.42 * others;
    row().key(format!("dev-cell-{}", g.key())).h(LEGEND_ROW).px(2.0).gap(9.0).center_y().radius(7.0)
        .fill(ca(0xffffff, bg))
        .child(icons::dev_group(g, 16.0).opacity(fade))
        .child(label(g.name(), 14.0, 400.0, 0xf3f4f6).opacity(fade))
        .child(spacer())
        .child(label(fmt_size_gb(s.dev.live[g.ix()]), 14.0, 400.0, 0x8d8f98).opacity(fade))
}

/// "36.3 GB" like the overview legend, which always reads in gigabytes.
fn fmt_size_gb(bytes: f64) -> String {
    format!("{} GB", crate::demo::fmt_gb(bytes))
}

/// The open family's items, three to a row in the legend's place, scrolling past nine.
/// Each item's hit cell tiles the grid; the visible cell sits inside it as the overview's does.
fn items(s: &State, g: Group) -> El {
    let rows: Vec<&Row> = s.dev.snap.group_rows(g).collect();
    let small = s.dev.snap.small[g.ix()];
    let cell_w = bar::BAR_W / 3.0;
    let cell_h = LEGEND_H / 3.0;
    let mut cells: Vec<El> = rows.iter().map(|r| item_cell(s, r, cell_w, cell_h)).collect();
    if small.count > 0 {
        let noun = if small.count == 1 { "item" } else { "items" };
        cells.push(
            row().w(cell_w).h(cell_h).px(8.0).gap(9.0).center_y()
                .child(label(format!("{} smaller {noun} under {}", small.count, fmt_size(MIN_ROW)), 13.0, 400.0, 0x7d808a).truncate())
                .child(spacer())
                .child(label(fmt_size(small.bytes), 13.0, 400.0, 0x7d808a)),
        );
    }
    if cells.is_empty() {
        cells.push(row().w(bar::BAR_W).h(cell_h).px(8.0).center_y().child(label("Nothing found for this family.", 13.0, 400.0, 0x8e9099)));
    }
    let mut grid = col().key(format!("dev-items-{}", g.key())).absolute(PAD_X, LEGEND_Y).size(bar::BAR_W, LEGEND_H)
        .scroll(ScrollSpec { stick_bottom: false })
        .scroll_edge_fade(10.0);
    let mut cells = cells.into_iter();
    loop {
        let mut line = row().w_full().h(cell_h);
        let mut any = false;
        for _ in 0..3 {
            if let Some(cell) = cells.next() {
                line = line.child(cell);
                any = true;
            }
        }
        if !any {
            break;
        }
        grid = grid.child(line);
    }
    grid
}

fn item_cell(s: &State, r: &Row, w: f32, h: f32) -> El {
    let hovered = s.dev.hover == Some(Hit::Item(r.key.clone()));
    let on = s.dev.selected.as_deref() == Some(r.key.as_str());
    let bg = if on { 0.085 } else if hovered { 0.05 } else { 0.001 };
    let key = r.key.clone();
    let hit_key = r.key.clone();
    el().key(format!("dev-item-{}", r.key)).role("button").label(format!("{}, {}", r.label, fmt_size(r.bytes)))
        .w(w).h(h).padding(2.0).hit().cursor(Cursor::Pointer)
        .on_hover_change(move |st: &mut State, inside, tx| {
            st.dev.note_hit(Hit::Item(hit_key.clone()), inside);
            model::repoll(tx);
        })
        .on_press_tx(move |st: &mut State, tx| {
            st.dev.press_item(&key);
            model::repoll(tx);
        })
        .child(
            row().w_full().h_full().px(8.0).gap(8.0).center_y().radius(7.0)
                .fill(ca(0xffffff, bg))
                .child(el().size(6.0, 6.0).radius(1.5).fill(c(r.group.ui())))
                .child(label(&r.label, 14.0, if on { 500.0 } else { 400.0 }, 0xf3f4f6).truncate_middle().grow(1.0))
                .child(label(fmt_size(r.bytes), 14.0, 400.0, 0x8d8f98)),
        )
}

fn footer(s: &State) -> El {
    let h = CARD_H - DIVIDER_Y - 1.0;
    let slot = el().absolute(0.0, DIVIDER_Y + 1.0).size(CARD_W, h);
    stack(slot, &s.dev.footer_fade, CARD_W, h, footer_line)
}

fn footer_line(f: &Foot) -> El {
    let h = CARD_H - DIVIDER_Y - 1.0;
    let icon = match (&f.id, f.group) {
        (FootId::Measuring, _) => icons::sparkle(15.0, 0x9aa0aa),
        (FootId::Groups, _) => icons::sparkle(15.0, 0x8fd9b0),
        (_, Some(g)) => icons::dev_group(g, 16.0),
        (_, None) => icons::sparkle(15.0, 0x9aa0aa),
    };
    let line = row().size(CARD_W, h).px(PAD_X).gap(9.0).center_y().child(icon);
    match &f.id {
        FootId::Measuring => line.child(label(&f.message, 14.0, 400.0, 0x9a9ca6).truncate()),
        FootId::Groups => line.child(label(&f.message, 14.0, 400.0, 0x9a9ca6).truncate()),
        FootId::Group(_) => line
            .child(label(&f.message, 14.0, 400.0, 0xb7b9c0).truncate())
            .child(spacer())
            .child(back_button()),
        FootId::Item(_) => line
            .child(
                col().grow(1.0).min_w(0.0).gap(3.0)
                    .child(label(&f.message, 12.0, 400.0, 0x9a9ca6).truncate())
                    .child(
                        row().gap(6.0).center_y()
                            .child(text("$").font(TextStyle::new(13.0, 400.0).mono()).color(c(0x5f626b)))
                            .child(text(f.command.clone()).font(TextStyle::new(13.0, 400.0).mono()).color(c(0xd9f7e6)).selectable(true).truncate_middle().grow(1.0)),
                    ),
            )
            .child(back_button())
            .child(copy_button(&f.command, &f.key, f.copied)),
    }
}

fn back_button() -> El {
    row().key("dev-back").role("button").label("All tools")
        .w(BACK_W).h(36.0).radius(8.0).center()
        .hover(ca(0xffffff, 0.06))
        .cursor(Cursor::Pointer)
        .press_scale(0.97)
        .on_press_tx(|st: &mut State, tx| {
            st.dev.press_back();
            model::repoll(tx);
        })
        .child(label("All tools", 14.0, 400.0, 0xa5a7b0))
}

/// The footer's action, styled as the overview's Empty Trash button. It copies; it never runs.
fn copy_button(command: &str, key: &str, copied: bool) -> El {
    let cmd = command.to_string();
    let key = key.to_string();
    let (name, color, fill) = if copied { ("Copied", 0x8fd9b0, ca(0x3ddc84, 0.14)) } else { ("Copy", 0xf4f5f7, c(0x2c2e33)) };
    row().key("copy-command").role("button").label("Copy command")
        .w(COPY_W).h(36.0).radius(8.0).center()
        .fill(fill)
        .hover(c(0x3a3c42))
        .border(1.0, ca(0xffffff, 0.10))
        .cursor(Cursor::Pointer)
        .press_scale(0.97)
        .on_press_tx(move |st: &mut State, tx: &mut Tx| {
            tx.copy(cmd.clone());
            st.dev.note_copied(&key);
            model::repoll(tx);
        })
        .child(label(name, 14.0, 450.0, color))
}
