use super::GrantKinoa;
use super::widgets::*;
use crate::project::*;
use egui::{Color32, Context, RichText, Sense, Ui, vec2};

impl GrantKinoa {
    pub(super) fn mixer_window(&mut self, ctx: &Context) {
        let mut open = self.show_mixer;
        egui::Window::new("Mixer")
            .open(&mut open)
            .default_pos([200.0, 420.0])
            .default_width(900.0)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    for t in 0..=MIXER_INSERTS {
                        self.strip(ui, t);
                    }
                    ui.separator();
                    self.fx_panel(ui);
                });
            });
        self.show_mixer = open;
    }

    fn strip(&mut self, ui: &mut Ui, t: usize) {
        let sel = self.mixer_sel == t;
        let peak = self.meters[t];
        let routed: Vec<String> = self
            .project
            .channels
            .iter()
            .filter(|c| c.mixer == t && t > 0)
            .map(|c| c.name.clone())
            .collect();
        let mt = &mut self.project.mixer[t];
        let fill = if sel { BG_LIGHT } else { BG_DARK };
        let r = egui::Frame::new()
            .fill(fill)
            .inner_margin(4)
            .corner_radius(3)
            .show(ui, |ui| {
                ui.set_width(58.0);
                ui.vertical_centered(|ui| {
                    let label = if t == 0 {
                        "Master".to_string()
                    } else {
                        format!("{t}")
                    };
                    ui.label(RichText::new(label).small().color(if sel {
                        ACCENT
                    } else {
                        TEXT_DIM
                    }));
                    ui.add(
                        egui::Label::new(RichText::new(&mt.name).small().color(Color32::WHITE))
                            .truncate(),
                    );
                    ui.horizontal(|ui| {
                        meter(ui, peak, vec2(12.0, 120.0));
                        ui.spacing_mut().slider_width = 120.0;
                        ui.add(
                            egui::Slider::new(&mut mt.volume, 0.0..=1.25)
                                .vertical()
                                .show_value(false),
                        );
                    });
                    let db = 20.0 * mt.volume.max(1e-4).log10();
                    ui.label(RichText::new(format!("{db:+.1} dB")).small().monospace());
                    knob(ui, &mut mt.pan, -1.0, 1.0, 0.0, "Paneo");
                    let on = !mt.mute;
                    if led(ui, on, Color32::from_rgb(120, 230, 90))
                        .on_hover_text("Silenciar / activar")
                        .clicked()
                    {
                        mt.mute = !mt.mute;
                    }
                });
            });
        let resp = r.response.interact(Sense::click());
        let resp = if routed.is_empty() {
            resp
        } else {
            resp.on_hover_text(format!("Canales: {}", routed.join(", ")))
        };
        if resp.clicked() {
            self.mixer_sel = t;
        }
    }

    fn fx_panel(&mut self, ui: &mut Ui) {
        let t = self.mixer_sel.min(MIXER_INSERTS);
        let mt = &mut self.project.mixer[t];
        ui.vertical(|ui| {
            ui.set_width(250.0);
            ui.label(
                RichText::new(if t == 0 {
                    "Master".to_string()
                } else {
                    format!("Insert {t}")
                })
                .strong()
                .color(ACCENT),
            );
            ui.text_edit_singleline(&mut mt.name);
            let fx = &mut mt.fx;
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show(ui, |ui| {
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.eq_on, "Kinoa EQ (3 bandas)");
                        ui.horizontal(|ui| {
                            for (i, n) in ["Graves", "Medios", "Agudos"].iter().enumerate() {
                                labeled(ui, n, |ui| {
                                    knob(ui, &mut fx.eq[i], -15.0, 15.0, 0.0, "dB")
                                });
                            }
                        });
                    });
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.comp_on, "Kinoa Comp (compresor)");
                        ui.horizontal(|ui| {
                            labeled(ui, "Umbral", |ui| {
                                knob(ui, &mut fx.comp_thresh, -48.0, 0.0, -18.0, "Umbral (dB)")
                            });
                            labeled(ui, "Ratio", |ui| {
                                knob(ui, &mut fx.comp_ratio, 1.0, 20.0, 4.0, "Ratio")
                            });
                            labeled(ui, "Ataque", |ui| {
                                knob(ui, &mut fx.comp_attack, 0.1, 100.0, 5.0, "Ataque (ms)")
                            });
                            labeled(ui, "Release", |ui| {
                                knob(
                                    ui,
                                    &mut fx.comp_release,
                                    10.0,
                                    1000.0,
                                    120.0,
                                    "Release (ms)",
                                )
                            });
                            labeled(ui, "Ganancia", |ui| {
                                knob(ui, &mut fx.comp_makeup, 0.0, 24.0, 3.0, "Ganancia (dB)")
                            });
                        });
                    });
                    if t > 0 {
                        ui.group(|ui| {
                            ui.label("Sidechain (ducking)");
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt("sc_src")
                                    .width(90.0)
                                    .selected_text(if fx.sc_source == 0 {
                                        "Apagado".to_string()
                                    } else {
                                        format!("Insert {}", fx.sc_source)
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut fx.sc_source, 0, "Apagado");
                                        for i in (1..=MIXER_INSERTS).filter(|&i| i != t) {
                                            ui.selectable_value(
                                                &mut fx.sc_source,
                                                i,
                                                format!("Insert {i}"),
                                            );
                                        }
                                    });
                                labeled(ui, "Profund.", |ui| {
                                    knob(ui, &mut fx.sc_depth, 0.0, 1.0, 0.6, "Profundidad")
                                });
                            });
                        });
                    }
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.sat_on, "Kinoa Saturator");
                        labeled(ui, "Drive", |ui| {
                            knob(ui, &mut fx.sat_drive, 0.0, 1.0, 0.3, "Drive")
                        });
                    });
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.crush_on, "Bitcrusher (lo-fi)");
                        ui.horizontal(|ui| {
                            labeled(ui, "Bits", |ui| {
                                knob(ui, &mut fx.crush_bits, 2.0, 16.0, 8.0, "Bits")
                            });
                            labeled(ui, "Reducción", |ui| {
                                knob(
                                    ui,
                                    &mut fx.crush_down,
                                    1.0,
                                    16.0,
                                    2.0,
                                    "Reducción de muestreo",
                                )
                            });
                        });
                    });
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.chorus_on, "Chorus");
                        ui.horizontal(|ui| {
                            labeled(ui, "Velocidad", |ui| {
                                knob(ui, &mut fx.chorus_rate, 0.05, 5.0, 0.8, "Hz")
                            });
                            labeled(ui, "Profund.", |ui| {
                                knob(ui, &mut fx.chorus_depth, 0.0, 1.0, 0.5, "Profundidad")
                            });
                            labeled(ui, "Mezcla", |ui| {
                                knob(ui, &mut fx.chorus_mix, 0.0, 1.0, 0.5, "Mezcla")
                            });
                        });
                    });
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.phaser_on, "Phaser");
                        ui.horizontal(|ui| {
                            labeled(ui, "Velocidad", |ui| {
                                knob(ui, &mut fx.phaser_rate, 0.05, 5.0, 0.3, "Hz")
                            });
                            labeled(ui, "Profund.", |ui| {
                                knob(ui, &mut fx.phaser_depth, 0.0, 1.0, 0.6, "Profundidad")
                            });
                            labeled(ui, "Mezcla", |ui| {
                                knob(ui, &mut fx.phaser_mix, 0.0, 1.0, 0.5, "Mezcla")
                            });
                        });
                    });
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.filter_on, "Filtro pasa-bajos");
                        ui.horizontal(|ui| {
                            labeled(ui, "Corte", |ui| {
                                knob_log(ui, &mut fx.cutoff, 40.0, 18000.0, 8000.0, "Corte")
                            });
                            labeled(ui, "Resonancia", |ui| {
                                knob(ui, &mut fx.resonance, 0.0, 0.95, 0.1, "Resonancia")
                            });
                        });
                    });
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.delay_on, "Delay ping-pong");
                        ui.horizontal(|ui| {
                            let times = [
                                ("1/16", 1.0),
                                ("1/8", 2.0),
                                ("1/8 punt.", 3.0),
                                ("1/4", 4.0),
                                ("1/2", 8.0),
                            ];
                            let cur = times
                                .iter()
                                .find(|x| x.1 == fx.delay_steps)
                                .map(|x| x.0)
                                .unwrap_or("1/8 punt.");
                            egui::ComboBox::from_id_salt("delay_time")
                                .width(70.0)
                                .selected_text(cur)
                                .show_ui(ui, |ui| {
                                    for (n, v) in times {
                                        ui.selectable_value(&mut fx.delay_steps, v, n);
                                    }
                                });
                            labeled(ui, "Feedback", |ui| {
                                knob(ui, &mut fx.delay_feedback, 0.0, 0.9, 0.35, "Feedback")
                            });
                            labeled(ui, "Mezcla", |ui| {
                                knob(ui, &mut fx.delay_mix, 0.0, 1.0, 0.25, "Mezcla")
                            });
                        });
                    });
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.reverb_on, "Reverb");
                        ui.horizontal(|ui| {
                            labeled(ui, "Tamaño", |ui| {
                                knob(ui, &mut fx.reverb_size, 0.0, 1.0, 0.7, "Tamaño")
                            });
                            labeled(ui, "Amortig.", |ui| {
                                knob(ui, &mut fx.reverb_damp, 0.0, 1.0, 0.4, "Amortiguación")
                            });
                            labeled(ui, "Mezcla", |ui| {
                                knob(ui, &mut fx.reverb_mix, 0.0, 1.0, 0.25, "Mezcla")
                            });
                        });
                    });
                    ui.group(|ui| {
                        ui.checkbox(&mut fx.limiter_on, "Kinoa Limiter");
                        labeled(ui, "Techo", |ui| {
                            knob(ui, &mut fx.limiter_ceiling, -12.0, 0.0, -0.3, "Techo (dB)")
                        });
                    });
                });
        });
    }
}
