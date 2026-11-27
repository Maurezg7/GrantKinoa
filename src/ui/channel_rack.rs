use super::GrantKinoa;
use super::widgets::*;
use crate::engine::PlayMode;
use crate::project::*;
use egui::{Button, Color32, Context, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

#[derive(Default)]
pub struct RackState {
    paint: Option<bool>,
}

enum RowAction {
    Remove(u32),
    Clone(u32),
    Move(u32, i32),
}

const STEP_W: f32 = 18.0;
const STEP_GAP: f32 = 2.0;

fn step_x(i: usize) -> f32 {
    i as f32 * (STEP_W + STEP_GAP) + (i / 4) as f32 * 4.0
}

impl GrantKinoa {
    pub(super) fn channel_rack_window(&mut self, ctx: &Context) {
        let mut open = self.show_rack;
        egui::Window::new("Channel Rack")
            .open(&mut open)
            .default_pos([230.0, 120.0])
            .default_width(640.0)
            .show(ctx, |ui| self.channel_rack(ui));
        self.show_rack = open;
    }

    fn channel_rack(&mut self, ui: &mut Ui) {
        if !ui.input(|i| i.pointer.primary_down() || i.pointer.secondary_down()) {
            self.rack.paint = None;
        }
        let pat_idx = self.pattern;
        ui.horizontal(|ui| {
            let p = &self.project.patterns[pat_idx];
            ui.label(RichText::new(&p.name).strong().color(rgb(p.color)));
            ui.label("Largo");
            let mut len = p.length;
            egui::ComboBox::from_id_salt("rack_len")
                .width(70.0)
                .selected_text(format!("{len} pasos"))
                .show_ui(ui, |ui| {
                    for l in [16, 32, 48, 64] {
                        ui.selectable_value(&mut len, l, format!("{l} pasos"));
                    }
                });
            if len != p.length {
                self.project.patterns[pat_idx].length = len;
            }
            ui.label(
                RichText::new("Click: paso · arrastrar: pintar · click derecho: borrar")
                    .small()
                    .color(TEXT_DIM),
            );
        });
        ui.separator();
        let mut action = None;
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .show(ui, |ui| {
                let ids: Vec<u32> = self.project.channels.iter().map(|c| c.id).collect();
                for id in ids {
                    if let Some(a) = self.rack_row(ui, id) {
                        action = Some(a);
                    }
                }
            });
        match action {
            Some(RowAction::Remove(id)) => {
                self.project.remove_channel(id);
                if self.selected == Some(id) {
                    self.selected = self.project.channels.first().map(|c| c.id);
                }
            }
            Some(RowAction::Clone(id)) => {
                if let Some(ch) = self.project.channel(id).cloned() {
                    let new = self
                        .project
                        .add_channel(format!("{} 2", ch.name), ch.instrument.clone());
                    for p in &mut self.project.patterns {
                        if let Some(cp) = p.channels.get(&id).cloned() {
                            p.channels.insert(new, cp);
                        }
                    }
                    self.selected = Some(new);
                }
            }
            Some(RowAction::Move(id, dir)) => {
                let chs = &mut self.project.channels;
                if let Some(i) = chs.iter().position(|c| c.id == id) {
                    let j = i as i32 + dir;
                    if j >= 0 && (j as usize) < chs.len() {
                        chs.swap(i, j as usize);
                    }
                }
            }
            None => {}
        }
        ui.separator();
        ui.menu_button("+ Agregar canal", |ui| {
            for kind in InstrumentKind::ALL {
                if ui.button(kind.label()).clicked() {
                    self.add_instrument(kind.label(), Instrument::new(kind));
                    ui.close();
                }
            }
        });
    }

    fn rack_row(&mut self, ui: &mut Ui, id: u32) -> Option<RowAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            let Some(ch) = self.project.channel_mut(id) else {
                return;
            };
            let color = rgb(ch.color);
            if led(ui, !ch.mute, Color32::from_rgb(120, 230, 90))
                .on_hover_text("Silenciar / activar")
                .clicked()
            {
                ch.mute = !ch.mute;
            }
            knob(ui, &mut ch.pan, -1.0, 1.0, 0.0, "Paneo");
            knob(ui, &mut ch.volume, 0.0, 1.0, 0.78, "Volumen");
            ui.add(
                egui::DragValue::new(&mut ch.mixer)
                    .range(0..=MIXER_INSERTS)
                    .speed(0.05),
            )
            .on_hover_text("Insert del mixer (0 = master)");
            let name = ch.name.clone();
            let sel = self.selected == Some(id);
            let fill = if sel { shade(color, 0.55) } else { BG_LIGHT };
            let b = ui.add_sized(
                [128.0, 22.0],
                Button::new(RichText::new(name).color(Color32::WHITE))
                    .fill(fill)
                    .selected(sel),
            );
            if b.clicked() {
                self.selected = Some(id);
                self.instrument_win = Some(id);
            }
            if b.double_clicked() {
                self.show_piano = true;
            }
            b.on_hover_text(
                "Click: ajustes del canal · doble click: piano roll · click derecho: menú",
            )
            .context_menu(|ui| {
                if ui.button("Abrir en Piano Roll").clicked() {
                    self.selected = Some(id);
                    self.show_piano = true;
                    ui.close();
                }
                if ui.button("Ajustes del canal").clicked() {
                    self.instrument_win = Some(id);
                    ui.close();
                }
                if ui.button("Limpiar en este pattern").clicked() {
                    let p = &mut self.project.patterns[self.pattern];
                    p.channels.remove(&id);
                    ui.close();
                }
                ui.separator();
                if ui.button("Subir").clicked() {
                    action = Some(RowAction::Move(id, -1));
                    ui.close();
                }
                if ui.button("Bajar").clicked() {
                    action = Some(RowAction::Move(id, 1));
                    ui.close();
                }
                if ui.button("Clonar").clicked() {
                    action = Some(RowAction::Clone(id));
                    ui.close();
                }
                if ui.button("Eliminar").clicked() {
                    action = Some(RowAction::Remove(id));
                    ui.close();
                }
            });
            self.step_grid(ui, id, color);
        });
        action
    }

    fn step_grid(&mut self, ui: &mut Ui, id: u32, color: Color32) {
        let pat_idx = self.pattern;
        let len = self.project.patterns[pat_idx].length as usize;
        let playing_step =
            (self.playing() && self.mode == PlayMode::Pattern).then(|| self.pos() as usize);
        let size = vec2(
            step_x(len) - STEP_GAP - 4.0 * (len.is_multiple_of(4) as u8 as f32),
            22.0,
        );
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let cp = self.project.patterns[pat_idx].channel_mut(id);

        if !cp.notes.is_empty() {
            painter.rect_filled(rect, 3.0, BG_DARK);
            let lo = cp.notes.iter().map(|n| n.key).min().unwrap_or(60) as f32;
            let hi = cp.notes.iter().map(|n| n.key).max().unwrap_or(60) as f32 + 1.0;
            let sw = rect.width() / len as f32;
            for n in &cp.notes {
                let y =
                    rect.bottom() - 3.0 - (n.key as f32 - lo) / (hi - lo) * (rect.height() - 6.0);
                let r = Rect::from_min_max(
                    pos2(rect.left() + n.start * sw, y - 2.0),
                    pos2(rect.left() + (n.start + n.len) * sw - 1.0, y),
                );
                painter.rect_filled(r, 1.0, color);
            }
            if let Some(s) = playing_step {
                let x = rect.left() + s as f32 * sw;
                painter.vline(x, rect.y_range(), Stroke::new(1.0, ACCENT));
            }
            if resp
                .on_hover_text("Tiene notas del piano roll: click para editar")
                .clicked()
            {
                self.selected = Some(id);
                self.show_piano = true;
            }
            return;
        }

        let pointer = ui.input(|i| i.pointer.interact_pos());
        let (pressed, sec_pressed, down) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.secondary_pressed(),
                i.pointer.any_down(),
            )
        });
        let hit = pointer.filter(|p| rect.contains(*p)).and_then(|p| {
            (0..len).find(|&i| {
                let x = rect.left() + step_x(i);
                p.x >= x && p.x < x + STEP_W + STEP_GAP
            })
        });
        if let Some(i) = hit {
            if pressed {
                self.rack.paint = Some(!cp.steps[i]);
            } else if sec_pressed {
                self.rack.paint = Some(false);
            }
            if let (Some(v), true) = (self.rack.paint, down) {
                cp.steps[i] = v;
                self.selected = Some(id);
            }
        }

        for i in 0..len {
            let x = rect.left() + step_x(i);
            let r = Rect::from_min_size(pos2(x, rect.top()), vec2(STEP_W, rect.height()));
            let base = if (i / 4) % 2 == 0 {
                Color32::from_rgb(70, 76, 81)
            } else {
                Color32::from_rgb(96, 66, 70)
            };
            let fill = if cp.steps[i] {
                Color32::from_rgb(222, 226, 230)
            } else {
                base
            };
            painter.rect_filled(r, 3.0, fill);
            if cp.steps[i] {
                painter.rect_filled(
                    Rect::from_min_size(r.left_top() + vec2(3.0, 3.0), vec2(STEP_W - 6.0, 3.0)),
                    1.0,
                    color,
                );
            }
            if playing_step == Some(i) {
                painter.rect_stroke(r, 3.0, Stroke::new(2.0, ACCENT), StrokeKind::Inside);
            }
        }
    }
}
