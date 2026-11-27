use super::widgets::*;
use super::{DialogPurpose, GrantKinoa};
use crate::project::*;
use egui::{Context, RichText, Ui};

impl GrantKinoa {
    pub(super) fn instrument_window(&mut self, ctx: &Context) {
        let Some(id) = self.instrument_win else {
            return;
        };
        let Some(name) = self.project.channel(id).map(|c| c.name.clone()) else {
            self.instrument_win = None;
            return;
        };
        let mut open = true;
        egui::Window::new(format!("Canal — {name}"))
            .id(egui::Id::unique("instrument"))
            .open(&mut open)
            .default_pos([520.0, 110.0])
            .resizable(false)
            .show(ctx, |ui| self.instrument_ui(ui, id));
        if !open {
            self.instrument_win = None;
            self.hold_key(None);
        }
    }

    fn instrument_ui(&mut self, ui: &mut Ui, id: u32) {
        let mut load_sample = false;
        {
            let Some(ch) = self.project.channel_mut(id) else {
                return;
            };
            egui::Grid::new("inst_head").num_columns(2).show(ui, |ui| {
                ui.label("Nombre");
                ui.text_edit_singleline(&mut ch.name);
                ui.end_row();
                ui.label("Color");
                ui.color_edit_button_srgb(&mut ch.color);
                ui.end_row();
                ui.label("Instrumento");
                egui::ComboBox::from_id_salt("inst_kind")
                    .selected_text(ch.instrument.kind.label())
                    .show_ui(ui, |ui| {
                        for k in InstrumentKind::ALL {
                            ui.selectable_value(&mut ch.instrument.kind, k, k.label());
                        }
                    });
                ui.end_row();
                ui.label("Insert del mixer");
                ui.add(
                    egui::DragValue::new(&mut ch.mixer)
                        .range(0..=MIXER_INSERTS)
                        .speed(0.05),
                );
                ui.end_row();
            });
            ui.horizontal(|ui| {
                labeled(ui, "Volumen", |ui| {
                    knob(ui, &mut ch.volume, 0.0, 1.0, 0.78, "Volumen")
                });
                labeled(ui, "Paneo", |ui| {
                    knob(ui, &mut ch.pan, -1.0, 1.0, 0.0, "Paneo")
                });
            });
            ui.separator();
            let inst = &mut ch.instrument;
            match inst.kind {
                InstrumentKind::Synth => synth_ui(ui, &mut inst.synth),
                InstrumentKind::Sampler => {
                    let s = &mut inst.sampler;
                    ui.horizontal(|ui| {
                        if ui.button("Cargar WAV…").clicked() {
                            load_sample = true;
                        }
                        let p = s
                            .path
                            .as_deref()
                            .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p))
                            .unwrap_or("(sin sample)");
                        ui.label(RichText::new(p).color(TEXT_DIM));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Nota raíz");
                        ui.add(
                            egui::DragValue::new(&mut s.root)
                                .range(0..=127)
                                .speed(0.2)
                                .custom_formatter(|v, _| note_name(v as u8)),
                        );
                        labeled(ui, "Ataque", |ui| {
                            knob(ui, &mut s.attack, 0.0, 1.0, 0.001, "Ataque (s)")
                        });
                        labeled(ui, "Release", |ui| {
                            knob(ui, &mut s.release, 0.0, 2.0, 0.05, "Release (s)")
                        });
                    });
                }
                _ => {
                    let d = &mut inst.drum;
                    ui.horizontal(|ui| {
                        labeled(ui, "Afinación", |ui| {
                            knob(ui, &mut d.tune, -24.0, 24.0, 0.0, "Afinación (semitonos)")
                        });
                        labeled(ui, "Decaimiento", |ui| {
                            knob(ui, &mut d.decay, 0.2, 3.0, 1.0, "Decaimiento")
                        });
                        labeled(ui, "Tono", |ui| {
                            knob(ui, &mut d.tone, 0.0, 1.0, 0.5, "Tono")
                        });
                    });
                }
            }
        }
        if load_sample {
            self.open_dialog(DialogPurpose::Sample(id));
        }
        ui.separator();
        ui.label(RichText::new("Teclado de prueba").small().color(TEXT_DIM));
        let held = mini_keyboard(ui, 48, 3);
        self.hold_key(held.map(|k| (id, k)));
    }
}

fn synth_ui(ui: &mut Ui, p: &mut SynthParams) {
    ui.horizontal(|ui| {
        ui.label("Presets:");
        let presets: [SynthPreset; 4] = [
            ("Bajo", preset_bass),
            ("Pluck", preset_pluck),
            ("Pad", preset_pad),
            ("Lead", preset_lead),
        ];
        for (n, f) in presets {
            if ui.small_button(n).clicked() {
                *p = f();
            }
        }
    });
    for (i, o) in p.osc.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("OSC {}", i + 1))
                    .strong()
                    .color(ACCENT),
            );
            egui::ComboBox::from_id_salt(("osc_wave", i))
                .width(80.0)
                .selected_text(o.wave.label())
                .show_ui(ui, |ui| {
                    for w in Wave::ALL {
                        ui.selectable_value(&mut o.wave, w, w.label());
                    }
                });
            ui.add(
                egui::DragValue::new(&mut o.octave)
                    .range(-3..=3)
                    .prefix("oct "),
            );
            labeled(ui, "Fino", |ui| {
                knob(
                    ui,
                    &mut o.fine,
                    -100.0,
                    100.0,
                    0.0,
                    "Afinación fina (cents)",
                )
            });
            labeled(ui, "Nivel", |ui| {
                knob(ui, &mut o.level, 0.0, 1.0, 0.5, "Nivel")
            });
        });
    }
    ui.horizontal(|ui| {
        labeled(ui, "Ataque", |ui| {
            knob(ui, &mut p.attack, 0.0, 2.0, 0.005, "Ataque (s)")
        });
        labeled(ui, "Decay", |ui| {
            knob(ui, &mut p.decay, 0.0, 2.0, 0.25, "Decay (s)")
        });
        labeled(ui, "Sustain", |ui| {
            knob(ui, &mut p.sustain, 0.0, 1.0, 0.6, "Sustain")
        });
        labeled(ui, "Release", |ui| {
            knob(ui, &mut p.release, 0.0, 3.0, 0.2, "Release (s)")
        });
    });
    ui.horizontal(|ui| {
        labeled(ui, "Corte", |ui| {
            knob_log(ui, &mut p.cutoff, 40.0, 18000.0, 2400.0, "Corte del filtro")
        });
        labeled(ui, "Resonancia", |ui| {
            knob(ui, &mut p.resonance, 0.0, 0.95, 0.25, "Resonancia")
        });
        labeled(ui, "Envolvente", |ui| {
            knob(ui, &mut p.env_amount, 0.0, 1.0, 0.4, "Envolvente → filtro")
        });
    });
}
