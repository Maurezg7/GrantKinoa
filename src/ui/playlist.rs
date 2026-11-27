use super::GrantKinoa;
use super::widgets::*;
use crate::engine::{Cmd, PlayMode};
use crate::project::*;
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

pub struct PlaylistState {
    drag: Option<(usize, f32)>,
    bar_w: f32,
    snap_bar: bool,
}

impl Default for PlaylistState {
    fn default() -> Self {
        Self {
            drag: None,
            bar_w: 64.0,
            snap_bar: true,
        }
    }
}

const HEADER_W: f32 = 96.0;
const RULER_H: f32 = 22.0;
const TRACK_H: f32 = 40.0;

impl GrantKinoa {
    pub(super) fn playlist_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Playlist — Arreglo").strong());
            ui.separator();
            ui.label("Pincel");
            let p = &self.project.patterns[self.pattern];
            ui.label(RichText::new(&p.name).color(rgb(p.color)));
            ui.separator();
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.playlist.bar_w, 24.0..=160.0).show_value(false));
            let label = if self.playlist.snap_bar { "Snap: compás" } else { "Snap: pulso" };
            ui.toggle_value(&mut self.playlist.snap_bar, label);
            ui.label(
                RichText::new("Click: colocar pattern · arrastrar: mover · click derecho: borrar · regla: posición")
                    .small()
                    .color(TEXT_DIM),
            );
        });
        egui::ScrollArea::both()
            .id_salt("playlist")
            .show(ui, |ui| self.playlist_canvas(ui));
    }

    fn playlist_canvas(&mut self, ui: &mut Ui) {
        let bar_w = self.playlist.bar_w;
        let sw = bar_w / STEPS_PER_BAR as f32;
        let bars = (self.project.song_length() / STEPS_PER_BAR + 16).max(64);
        let size = vec2(
            HEADER_W + bars as f32 * bar_w,
            RULER_H + PLAYLIST_TRACKS as f32 * TRACK_H,
        );
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let gx = rect.left() + HEADER_W;
        let gy = rect.top() + RULER_H;
        let snap = if self.playlist.snap_bar {
            STEPS_PER_BAR as f32
        } else {
            4.0
        };

        painter.rect_filled(
            Rect::from_min_max(pos2(gx, rect.top()), pos2(rect.right(), gy)),
            0.0,
            BG_DARK,
        );
        for b in 0..=bars {
            let x = gx + b as f32 * bar_w;
            painter.vline(x, rect.top()..=gy, Stroke::new(1.0, Color32::from_gray(70)));
            if bar_w >= 40.0 || b % 4 == 0 {
                painter.text(
                    pos2(x + 3.0, rect.top() + 4.0),
                    Align2::LEFT_TOP,
                    (b + 1).to_string(),
                    FontId::monospace(11.0),
                    TEXT_DIM,
                );
            }
        }
        for t in 0..PLAYLIST_TRACKS {
            let y = gy + t as f32 * TRACK_H;
            let row = Rect::from_min_max(pos2(gx, y), pos2(rect.right(), y + TRACK_H));
            painter.rect_filled(
                row,
                0.0,
                if t % 2 == 0 {
                    Color32::from_rgb(48, 54, 58)
                } else {
                    Color32::from_rgb(44, 50, 54)
                },
            );
            let head =
                Rect::from_min_size(pos2(rect.left(), y), vec2(HEADER_W - 2.0, TRACK_H - 1.0));
            painter.rect_filled(head, 2.0, BG_LIGHT);
            painter.text(
                head.left_center() + vec2(8.0, 0.0),
                Align2::LEFT_CENTER,
                format!("Pista {}", t + 1),
                FontId::proportional(12.0),
                Color32::from_gray(220),
            );
            painter.hline(
                gx..=rect.right(),
                y + TRACK_H,
                Stroke::new(1.0, Color32::from_gray(35)),
            );
        }
        for b in 0..=bars {
            let x = gx + b as f32 * bar_w;
            painter.vline(
                x,
                gy..=rect.bottom(),
                Stroke::new(1.0, Color32::from_gray(30)),
            );
            if bar_w >= 48.0 {
                for q in 1..4 {
                    let xq = x + q as f32 * bar_w / 4.0;
                    painter.vline(
                        xq,
                        gy..=rect.bottom(),
                        Stroke::new(1.0, Color32::from_rgb(56, 62, 66)),
                    );
                }
            }
        }

        let pointer = ui.input(|i| i.pointer.interact_pos());
        let (pressed, down, sec_down, dbl) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.secondary_down(),
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary),
            )
        });
        let over = resp.hovered() || resp.is_pointer_button_down_on();
        if let Some(p) = pointer {
            let track = (((p.y - gy) / TRACK_H).floor().max(0.0) as usize).min(PLAYLIST_TRACKS - 1);
            let s = ((p.x - gx) / sw).max(0.0);
            let hit = self.project.clips.iter().rposition(|c| {
                let l = self.project.patterns[c.pattern].length as f32;
                c.track == track && s >= c.start as f32 && s < c.start as f32 + l
            });
            if over && pressed && p.x > gx {
                if p.y < gy {
                    let pos = (s / 4.0).floor() as f64 * 4.0;
                    if self.mode != PlayMode::Song {
                        self.set_mode(PlayMode::Song);
                    }
                    self.audio.send(Cmd::Seek(pos));
                } else if let Some(i) = hit {
                    let c = self.project.clips[i];
                    self.playlist.drag = Some((i, s - c.start as f32));
                    self.set_pattern(c.pattern);
                    if dbl {
                        self.show_rack = true;
                    }
                } else {
                    let start = ((s / snap).floor() * snap) as u32;
                    self.project.clips.push(Clip {
                        track,
                        pattern: self.pattern,
                        start,
                    });
                    self.playlist.drag = Some((self.project.clips.len() - 1, s - start as f32));
                }
            }
            if down {
                if let Some((i, grab)) = self.playlist.drag
                    && let Some(c) = self.project.clips.get_mut(i)
                {
                    c.start = (((s - grab) / snap).round() * snap).max(0.0) as u32;
                    c.track = track;
                }
            } else {
                self.playlist.drag = None;
            }
            if over
                && sec_down
                && p.x > gx
                && p.y > gy
                && let Some(i) = hit
            {
                self.project.clips.remove(i);
                self.playlist.drag = None;
            }
        }

        for c in &self.project.clips {
            let pat = &self.project.patterns[c.pattern];
            let r = Rect::from_min_size(
                pos2(
                    gx + c.start as f32 * sw + 1.0,
                    gy + c.track as f32 * TRACK_H + 2.0,
                ),
                vec2(pat.length as f32 * sw - 2.0, TRACK_H - 4.0),
            );
            let col = rgb(pat.color);
            painter.rect(
                r,
                3.0,
                shade(col, 0.35),
                Stroke::new(1.0, shade(col, 0.8)),
                StrokeKind::Inside,
            );
            let title = Rect::from_min_size(r.min, vec2(r.width(), 13.0));
            painter.rect_filled(title, 3.0, shade(col, 0.7));
            painter.with_clip_rect(r).text(
                title.left_center() + vec2(4.0, 0.0),
                Align2::LEFT_CENTER,
                &pat.name,
                FontId::proportional(10.0),
                Color32::from_gray(15),
            );
            let body = Rect::from_min_max(
                pos2(r.left(), title.bottom() + 2.0),
                pos2(r.right(), r.bottom() - 2.0),
            );
            let keys = pat
                .channels
                .values()
                .flat_map(|cp| cp.notes.iter().map(|n| n.key));
            let (lo, hi) = keys.fold((127u8, 0u8), |(a, b), k| (a.min(k), b.max(k)));
            let span = (hi.saturating_sub(lo) as f32 + 1.0).max(1.0);
            for cp in pat.channels.values() {
                for (i, on) in cp.steps.iter().enumerate().take(pat.length as usize) {
                    if *on {
                        let x = r.left() + i as f32 * sw;
                        painter.rect_filled(
                            Rect::from_min_size(
                                pos2(x + 1.0, body.bottom() - 4.0),
                                vec2((sw - 2.0).max(1.0), 3.0),
                            ),
                            0.0,
                            shade(col, 1.2),
                        );
                    }
                }
                for n in &cp.notes {
                    let y =
                        body.bottom() - 6.0 - (n.key - lo) as f32 / span * (body.height() - 8.0);
                    let x0 = r.left() + n.start * sw;
                    painter.hline(
                        x0..=(x0 + n.len * sw - 1.0).min(r.right()),
                        y,
                        Stroke::new(1.5, shade(col, 1.25)),
                    );
                }
            }
        }

        let end = self.project.song_length() as f32;
        if end > 0.0 {
            let x = gx + end * sw;
            painter.vline(
                x,
                rect.top()..=gy,
                Stroke::new(2.0, Color32::from_rgb(232, 96, 112)),
            );
        }
        if self.mode == PlayMode::Song {
            let x = gx + self.pos() as f32 * sw;
            painter.vline(x, rect.y_range(), Stroke::new(2.0, ACCENT));
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(x - 6.0, rect.top()),
                    pos2(x + 6.0, rect.top()),
                    pos2(x, rect.top() + 8.0),
                ],
                ACCENT,
                Stroke::NONE,
            ));
        }
    }
}
