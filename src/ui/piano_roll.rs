use super::GrantKinoa;
use super::widgets::*;
use crate::engine::PlayMode;
use crate::project::*;
use egui::{
    Align2, Color32, Context, CursorIcon, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui,
    pos2, vec2,
};

const LOW: u8 = 24;
const HIGH: u8 = 107;
const KEY_H: f32 = 14.0;
const KB_W: f32 = 52.0;

enum Drag {
    Move { idx: usize, grab: f32 },
    Resize { idx: usize },
}

pub struct PianoState {
    drag: Option<Drag>,
    snap: f32,
    last_len: f32,
    preview: Option<(u32, u8)>,
    scrolled: bool,
}

impl Default for PianoState {
    fn default() -> Self {
        Self {
            drag: None,
            snap: 1.0,
            last_len: 2.0,
            preview: None,
            scrolled: false,
        }
    }
}

impl GrantKinoa {
    pub(super) fn piano_roll_window(&mut self, ctx: &Context) {
        let mut open = self.show_piano;
        let title = match self.selected.and_then(|id| self.project.channel(id)) {
            Some(ch) => format!("Piano Roll — {}", ch.name),
            None => "Piano Roll".into(),
        };
        egui::Window::new(title)
            .id(egui::Id::unique("piano_roll"))
            .open(&mut open)
            .default_pos([300.0, 160.0])
            .default_size([820.0, 480.0])
            .show(ctx, |ui| self.piano_roll(ui));
        self.show_piano = open;
        if !open {
            self.piano.scrolled = false;
        }
    }

    fn preview(&mut self, note: Option<(u32, u8)>) {
        if note != self.piano.preview {
            if let Some((c, k)) = self.piano.preview {
                self.note_off(c, k);
            }
            if let Some((c, k)) = note {
                self.note_on(c, k);
            }
            self.piano.preview = note;
        }
    }

    fn piano_roll(&mut self, ui: &mut Ui) {
        let Some(id) = self
            .selected
            .filter(|id| self.project.channel(*id).is_some())
        else {
            ui.label("Seleccioná un canal en el Channel Rack.");
            return;
        };
        let color = rgb(self.project.channel(id).unwrap().color);
        let pat_idx = self.pattern;
        ui.horizontal(|ui| {
            ui.label("Snap");
            let snaps = [
                ("1/4 paso", 0.25),
                ("1/2 paso", 0.5),
                ("Paso", 1.0),
                ("Pulso", 4.0),
                ("Compás", 16.0),
            ];
            let cur = snaps
                .iter()
                .find(|s| s.1 == self.piano.snap)
                .map(|s| s.0)
                .unwrap_or("Paso");
            egui::ComboBox::from_id_salt("pr_snap")
                .width(80.0)
                .selected_text(cur)
                .show_ui(ui, |ui| {
                    for (n, v) in snaps {
                        ui.selectable_value(&mut self.piano.snap, v, n);
                    }
                });
            let p = &mut self.project.patterns[pat_idx];
            ui.label("Largo");
            egui::ComboBox::from_id_salt("pr_len")
                .width(80.0)
                .selected_text(format!("{} pasos", p.length))
                .show_ui(ui, |ui| {
                    for l in [16, 32, 48, 64] {
                        ui.selectable_value(&mut p.length, l, format!("{l} pasos"));
                    }
                });
            if ui.button("Limpiar").clicked() {
                p.channel_mut(id).notes.clear();
            }
            ui.label(
                RichText::new(
                    "Click: nota · arrastrar: mover · borde derecho: largo · click derecho: borrar",
                )
                .small()
                .color(TEXT_DIM),
            );
        });

        let lane_h = 60.0;
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("pr_scroll")
            .max_height((ui.available_height() - lane_h - 8.0).max(360.0));
        if !self.piano.scrolled {
            scroll = scroll.vertical_scroll_offset((HIGH - 79) as f32 * KEY_H);
            self.piano.scrolled = true;
        }
        let mut grid_x = (0.0, 1.0);
        scroll.show(ui, |ui| {
            grid_x = self.piano_canvas(ui, id, color);
        });
        self.velocity_lane(ui, id, color, grid_x, lane_h);
    }

    /// Returns (grid left x, step width) for the velocity lane.
    fn piano_canvas(&mut self, ui: &mut Ui, id: u32, color: Color32) -> (f32, f32) {
        let pat_idx = self.pattern;
        let len = self.project.patterns[pat_idx].length as f32;
        let w = ui.available_width().max(400.0);
        let h = (HIGH - LOW + 1) as f32 * KEY_H;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let grid = Rect::from_min_max(pos2(rect.left() + KB_W, rect.top()), rect.max);
        let sw = grid.width() / len;
        let key_y = |k: u8| rect.top() + (HIGH - k) as f32 * KEY_H;
        let key_at = |y: f32| {
            HIGH.saturating_sub(((y - rect.top()) / KEY_H).floor().max(0.0) as u8)
                .clamp(LOW, HIGH)
        };
        let step_at = |x: f32| ((x - grid.left()) / sw).max(0.0);

        for k in LOW..=HIGH {
            let r = Rect::from_min_size(pos2(grid.left(), key_y(k)), vec2(grid.width(), KEY_H));
            let fill = if is_black_key(k) {
                Color32::from_rgb(40, 45, 49)
            } else {
                Color32::from_rgb(52, 58, 63)
            };
            painter.rect_filled(r, 0.0, fill);
            let line = if k % 12 == 0 {
                Color32::from_gray(25)
            } else {
                Color32::from_rgb(45, 50, 54)
            };
            painter.hline(grid.x_range(), r.bottom(), Stroke::new(1.0, line));
        }
        for s in 0..=len as usize {
            let x = grid.left() + s as f32 * sw;
            let (wd, c) = if s % 16 == 0 {
                (1.5, Color32::from_gray(20))
            } else if s % 4 == 0 {
                (1.0, Color32::from_gray(32))
            } else {
                (1.0, Color32::from_rgb(46, 51, 55))
            };
            painter.vline(x, rect.y_range(), Stroke::new(wd, c));
        }

        let pointer = ui.input(|i| i.pointer.interact_pos());
        let (pressed, down, sec_down) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.secondary_down(),
            )
        });
        let snap = self.piano.snap;
        let mut preview = self.piano.preview.filter(|_| down);

        if let Some(p) = pointer {
            let notes = &mut self.project.patterns[pat_idx].channel_mut(id).notes;
            let over = resp.hovered() || resp.is_pointer_button_down_on();
            if over && pressed {
                if p.x < grid.left() {
                    preview = Some((id, key_at(p.y)));
                } else {
                    let s = step_at(p.x);
                    let key = key_at(p.y);
                    let hit = notes
                        .iter()
                        .rposition(|n| n.key == key && s >= n.start && s < n.start + n.len);
                    self.piano.drag = Some(match hit {
                        Some(idx) => {
                            let n = notes[idx];
                            preview = Some((id, n.key));
                            if grid.left() + (n.start + n.len) * sw - p.x < 6.0 {
                                Drag::Resize { idx }
                            } else {
                                Drag::Move {
                                    idx,
                                    grab: s - n.start,
                                }
                            }
                        }
                        None => {
                            let start = ((s / snap).floor() * snap).min(len - snap.min(len));
                            let nl = self.piano.last_len.min(len - start).max(0.25);
                            notes.push(Note {
                                start,
                                len: nl,
                                key,
                                vel: 0.8,
                            });
                            preview = Some((id, key));
                            Drag::Move {
                                idx: notes.len() - 1,
                                grab: s - start,
                            }
                        }
                    });
                }
            }
            if down {
                match self.piano.drag {
                    Some(Drag::Move { idx, grab }) if idx < notes.len() => {
                        let n = &mut notes[idx];
                        n.start = (((step_at(p.x) - grab) / snap).round() * snap)
                            .clamp(0.0, (len - n.len).max(0.0));
                        n.key = key_at(p.y);
                        preview = Some((id, n.key));
                    }
                    Some(Drag::Resize { idx }) if idx < notes.len() => {
                        let n = &mut notes[idx];
                        let l =
                            (((step_at(p.x) - n.start) / snap).round() * snap).max(snap.min(1.0));
                        n.len = l.min(len - n.start);
                        self.piano.last_len = n.len;
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                    }
                    _ => {}
                }
            } else {
                self.piano.drag = None;
            }
            if over && sec_down && p.x >= grid.left() {
                let s = step_at(p.x);
                let key = key_at(p.y);
                notes.retain(|n| !(n.key == key && s >= n.start && s < n.start + n.len));
                self.piano.drag = None;
            }
            if over && self.piano.drag.is_none() && p.x > grid.left() {
                let s = step_at(p.x);
                let key = key_at(p.y);
                if notes.iter().any(|n| {
                    n.key == key
                        && s < n.start + n.len
                        && grid.left() + (n.start + n.len) * sw - p.x < 6.0
                        && s >= n.start
                }) {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
            }
        }
        if preview.map(|(_, k)| k) != self.piano.preview.map(|(_, k)| k) || preview.is_none() {
            self.preview(preview);
        }

        let notes = &self.project.patterns[pat_idx].channels[&id].notes;
        for n in notes {
            let r = Rect::from_min_size(
                pos2(grid.left() + n.start * sw + 1.0, key_y(n.key) + 1.0),
                vec2((n.len * sw - 2.0).max(3.0), KEY_H - 2.0),
            );
            let fill = shade(color, 0.55 + 0.45 * n.vel);
            painter.rect(
                r,
                3.0,
                fill,
                Stroke::new(1.0, shade(color, 0.35)),
                StrokeKind::Inside,
            );
            if r.width() > 28.0 {
                painter.text(
                    r.left_center() + vec2(3.0, 0.0),
                    Align2::LEFT_CENTER,
                    note_name(n.key),
                    FontId::proportional(10.0),
                    Color32::from_gray(20),
                );
            }
        }

        for k in LOW..=HIGH {
            let r = Rect::from_min_size(pos2(rect.left(), key_y(k)), vec2(KB_W, KEY_H));
            let held = self.piano.preview.map(|(_, pk)| pk) == Some(k);
            let fill = if held {
                ACCENT
            } else if is_black_key(k) {
                Color32::from_gray(30)
            } else {
                Color32::from_gray(222)
            };
            painter.rect_filled(r.shrink2(vec2(0.0, 0.5)), 0.0, fill);
            if k % 12 == 0 {
                painter.text(
                    r.right_center() - vec2(4.0, 0.0),
                    Align2::RIGHT_CENTER,
                    note_name(k),
                    FontId::proportional(10.0),
                    Color32::from_gray(40),
                );
            }
        }

        if self.playing() && self.mode == PlayMode::Pattern {
            let x = grid.left() + self.pos() as f32 * sw;
            painter.vline(x, rect.y_range(), Stroke::new(2.0, ACCENT));
        }
        (grid.left(), sw)
    }

    fn velocity_lane(
        &mut self,
        ui: &mut Ui,
        id: u32,
        color: Color32,
        (gx, sw): (f32, f32),
        h: f32,
    ) {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, BG_DARK);
        painter.text(
            rect.left_top() + vec2(4.0, 2.0),
            Align2::LEFT_TOP,
            "Velocidad",
            FontId::proportional(10.0),
            TEXT_DIM,
        );
        let pat_idx = self.pattern;
        let notes = &mut self.project.patterns[pat_idx].channel_mut(id).notes;
        if let Some(p) = resp
            .interact_pointer_pos()
            .filter(|_| resp.is_pointer_button_down_on())
        {
            let vel = (1.0 - (p.y - rect.top() - 4.0) / (h - 8.0)).clamp(0.05, 1.0);
            let near = notes
                .iter()
                .enumerate()
                .map(|(i, n)| (i, (gx + n.start * sw - p.x).abs()))
                .filter(|(_, d)| *d < sw.max(6.0))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
            if let Some(i) = near {
                let start = notes[i].start;
                for n in notes.iter_mut().filter(|n| n.start == start) {
                    n.vel = vel;
                }
            }
        }
        for n in notes.iter() {
            let x = gx + n.start * sw + 2.0;
            let y = rect.bottom() - 4.0 - n.vel * (h - 8.0);
            painter.vline(x, y..=rect.bottom() - 2.0, Stroke::new(2.0, color));
            painter.circle_filled(pos2(x, y), 3.0, color);
        }
    }
}
