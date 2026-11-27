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
        });
    }
}
