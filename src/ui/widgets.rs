use egui::{
    Color32, CornerRadius, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2,
    vec2,
};
use std::f32::consts::PI;

pub const ACCENT: Color32 = Color32::from_rgb(255, 157, 46);
pub const BG_DARK: Color32 = Color32::from_rgb(30, 34, 37);
pub const BG: Color32 = Color32::from_rgb(52, 58, 62);
pub const BG_LIGHT: Color32 = Color32::from_rgb(74, 82, 88);
pub const TEXT_DIM: Color32 = Color32::from_rgb(150, 160, 166);

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn shade(c: Color32, k: f32) -> Color32 {
    let f = |v: u8| (v as f32 * k).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

fn knob_norm(ui: &mut Ui, t: &mut f32, default: f32, size: f32, text: String) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    if resp.dragged() {
        let d = resp.drag_delta();
        let k = if ui.input(|i| i.modifiers.shift) {
            0.0015
        } else {
            0.006
        };
        *t = (*t + (d.x - d.y) * k).clamp(0.0, 1.0);
        resp.mark_changed();
    }
    if resp.double_clicked() {
        *t = default;
        resp.mark_changed();
    }
    let p = ui.painter();
    let c = rect.center();
    let r = size * 0.5 - 2.0;
    p.circle_filled(c, r, BG_DARK);
    let a0 = PI * 0.75;
    let a1 = PI * 2.25;
    let arc = |from: f32, to: f32| -> Vec<Pos2> {
        (0..=24)
            .map(|i| {
                let a = from + (to - from) * i as f32 / 24.0;
                c + vec2(a.cos(), a.sin()) * r
            })
            .collect()
    };
    p.add(Shape::line(arc(a0, a1), Stroke::new(2.5, BG_LIGHT)));
    let av = a0 + (a1 - a0) * *t;
    p.add(Shape::line(
        arc(a0, av),
        Stroke::new(
            2.5,
            if resp.hovered() {
                Color32::WHITE
            } else {
                ACCENT
            },
        ),
    ));
    p.line_segment(
        [c, c + vec2(av.cos(), av.sin()) * r * 0.7],
        Stroke::new(2.0, Color32::from_gray(230)),
    );
    resp.on_hover_text(text)
}

/// Linear knob. Drag up/right to increase, Shift for fine control, double-click to reset.
pub fn knob(ui: &mut Ui, v: &mut f32, min: f32, max: f32, default: f32, label: &str) -> Response {
    let mut t = ((*v - min) / (max - min)).clamp(0.0, 1.0);
    let r = knob_norm(
        ui,
        &mut t,
        (default - min) / (max - min),
        26.0,
        format!("{label}: {:.2}", *v),
    );
    if r.changed() {
        *v = min + t * (max - min);
    }
    r
}

pub fn knob_log(
    ui: &mut Ui,
    v: &mut f32,
    min: f32,
    max: f32,
    default: f32,
    label: &str,
) -> Response {
    let span = (max / min).ln();
    let mut t = ((*v / min).ln() / span).clamp(0.0, 1.0);
    let r = knob_norm(
        ui,
        &mut t,
        (default / min).ln() / span,
        26.0,
        format!("{label}: {:.0} Hz", *v),
    );
    if r.changed() {
        *v = min * (t * span).exp();
    }
    r
}

/// Knob with a caption underneath.
pub fn labeled(ui: &mut Ui, caption: &str, add: impl FnOnce(&mut Ui) -> Response) -> Response {
    ui.vertical_centered(|ui| {
        ui.set_width(52.0);
        let r = add(ui);
        ui.label(egui::RichText::new(caption).small().color(TEXT_DIM));
        r
    })
    .inner
}

pub fn led(ui: &mut Ui, on: bool, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::click());
    let p = ui.painter();
    p.circle_filled(rect.center(), 5.5, BG_DARK);
    p.circle_filled(
        rect.center(),
        4.0,
        if on { color } else { Color32::from_gray(60) },
    );
    resp
}

pub fn meter(ui: &mut Ui, peak: [f32; 2], size: Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, BG_DARK);
    let w = (rect.width() - 3.0) / 2.0;
    for (i, v) in peak.iter().enumerate() {
        let db = 20.0 * v.max(1e-6).log10();
        let t = ((db + 48.0) / 54.0).clamp(0.0, 1.0);
        let x = rect.left() + 1.0 + i as f32 * (w + 1.0);
        let bar = Rect::from_min_max(
            pos2(x, rect.bottom() - 1.0 - t * (rect.height() - 2.0)),
            pos2(x + w, rect.bottom() - 1.0),
        );
        let color = if *v >= 1.0 {
            Color32::from_rgb(240, 60, 50)
        } else if t > 0.8 {
            Color32::from_rgb(240, 200, 60)
        } else {
            Color32::from_rgb(90, 210, 110)
        };
        p.rect_filled(bar, 1.0, color);
    }
    resp
}

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Pause,
    Stop,
}

pub fn icon_button(ui: &mut Ui, icon: Icon, active: bool, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 24.0), Sense::click());
    let p = ui.painter();
    let fill = if resp.hovered() { BG_LIGHT } else { BG_DARK };
    p.rect(
        rect,
        CornerRadius::same(4),
        fill,
        Stroke::new(1.0, Color32::from_gray(80)),
        StrokeKind::Inside,
    );
    let c = rect.center();
    let col = if active {
        ACCENT
    } else {
        Color32::from_gray(220)
    };
    match icon {
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![
                    c + vec2(-5.0, -7.0),
                    c + vec2(7.0, 0.0),
                    c + vec2(-5.0, 7.0),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            p.rect_filled(
                Rect::from_center_size(c - vec2(3.5, 0.0), vec2(4.0, 13.0)),
                1.0,
                col,
            );
            p.rect_filled(
                Rect::from_center_size(c + vec2(3.5, 0.0), vec2(4.0, 13.0)),
                1.0,
                col,
            );
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(12.0, 12.0)), 1.0, col);
        }
    }
    resp.on_hover_text(tip)
}

/// Clickable keyboard; returns the key held under the pointer, if any.
pub fn mini_keyboard(ui: &mut Ui, base: u8, octaves: u8) -> Option<u8> {
    let whites = octaves as usize * 7;
    let ww = 16.0;
    let h = 54.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(whites as f32 * ww, h), Sense::click_and_drag());
    let p = ui.painter();
    const WHITE: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
    const BLACK: [(usize, u8); 5] = [(0, 1), (1, 3), (3, 6), (4, 8), (5, 10)];
    let pointer = resp
        .interact_pointer_pos()
        .filter(|_| resp.is_pointer_button_down_on());
    let mut held = None;
    let mut black_rects = Vec::new();
    for o in 0..octaves as usize {
        for (wi, semi) in BLACK {
            let x = rect.left() + (o * 7 + wi) as f32 * ww + ww * 0.65;
            let r = Rect::from_min_size(pos2(x, rect.top()), vec2(ww * 0.7, h * 0.6));
            black_rects.push((r, base + (o as u8) * 12 + semi));
        }
    }
    if let Some(pt) = pointer {
        held = black_rects
            .iter()
            .find(|(r, _)| r.contains(pt))
            .map(|(_, k)| *k);
        if held.is_none() && rect.contains(pt) {
            let wi = ((pt.x - rect.left()) / ww) as usize;
            held = Some(base + (wi / 7) as u8 * 12 + WHITE[wi % 7]);
        }
    }
    for wi in 0..whites {
        let key = base + (wi / 7) as u8 * 12 + WHITE[wi % 7];
        let r = Rect::from_min_size(
            pos2(rect.left() + wi as f32 * ww, rect.top()),
            vec2(ww - 1.0, h),
        );
        p.rect_filled(
            r,
            2.0,
            if held == Some(key) {
                ACCENT
            } else {
                Color32::from_gray(225)
            },
        );
    }
    for (r, key) in black_rects {
        p.rect_filled(
            r,
            2.0,
            if held == Some(key) {
                ACCENT
            } else {
                Color32::from_gray(25)
            },
        );
    }
    held
}
