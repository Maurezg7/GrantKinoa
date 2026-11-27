mod channel_rack;
mod instrument;
mod mixer;
mod piano_roll;
mod playlist;
pub mod widgets;

use crate::audio::Audio;
use crate::engine::{Cmd, PlayMode, SampleData};
use crate::export;
use crate::project::*;
use crossbeam_channel::Receiver;
use egui::{Context, Key, RichText};
use egui_file_dialog::FileDialog;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use widgets::*;

#[derive(Clone, Copy, PartialEq)]
enum DialogPurpose {
    Open,
    Save,
    Export,
    Sample(u32),
}

#[derive(Clone, Copy)]
enum Pending {
    New,
    Open,
    Demo,
    Quit,
}

pub struct GrantKinoa {
    project: Project,
    sent: Arc<Project>,
    audio: Audio,
    samples: HashMap<u32, Arc<SampleData>>,
    loaded_paths: HashMap<u32, Option<String>>,
    mode: PlayMode,
    pattern: usize,
    selected: Option<u32>,
    show_rack: bool,
    show_piano: bool,
    show_mixer: bool,
    show_browser: bool,
    instrument_win: Option<u32>,
    dialog: FileDialog,
    purpose: Option<DialogPurpose>,
    path: Option<PathBuf>,
    status: String,
    title: String,
    rack: channel_rack::RackState,
    piano: piano_roll::PianoState,
    playlist: playlist::PlaylistState,
    mixer_sel: usize,
    meters: Vec<[f32; 2]>,
    held_key: Option<(u32, u8)>,
    export_rx: Option<Receiver<Result<PathBuf, String>>>,
    saved: String,
    pending: Option<Pending>,
    quit_ok: bool,
    ctx: Context,
}

impl GrantKinoa {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        setup_style(&cc.egui_ctx);
        let project = Project::demo();
        let sent = Arc::new(project.clone());
        let audio = Audio::start(sent.clone());
        let status = format!("Audio: {}", audio.status);
        let mut app = Self {
            selected: project.channels.first().map(|c| c.id),
            project,
            sent,
            audio,
            samples: HashMap::new(),
            loaded_paths: HashMap::new(),
            mode: PlayMode::Pattern,
            pattern: 0,
            show_rack: true,
            show_piano: false,
            show_mixer: false,
            show_browser: true,
            instrument_win: None,
            dialog: FileDialog::new(),
            purpose: None,
            path: None,
            status,
            title: String::new(),
            rack: Default::default(),
            piano: Default::default(),
            playlist: Default::default(),
            mixer_sel: 1,
            meters: vec![[0.0; 2]; MIXER_INSERTS + 1],
            held_key: None,
            export_rx: None,
            saved: String::new(),
            pending: None,
            quit_ok: false,
            ctx: cc.egui_ctx.clone(),
        };
        if let Some(p) = open {
            app.open_project(&p);
        }
        app.mark_saved();
        app
    }

    fn playing(&self) -> bool {
        self.audio.shared.playing.load(Ordering::Relaxed)
    }

    fn pos(&self) -> f64 {
        self.audio.shared.pos()
    }

    fn toggle_play(&mut self) {
        self.sync();
        self.audio
            .send(if self.playing() { Cmd::Stop } else { Cmd::Play });
    }

    fn set_mode(&mut self, mode: PlayMode) {
        self.mode = mode;
        self.audio.send(Cmd::Mode(mode));
    }

    fn set_pattern(&mut self, idx: usize) {
        self.pattern = idx.min(self.project.patterns.len() - 1);
        self.audio.send(Cmd::CurrentPattern(self.pattern));
    }

    fn note_on(&mut self, ch: u32, key: u8) {
        self.sync();
        self.audio.send(Cmd::NoteOn { ch, key, vel: 0.8 });
    }

    fn note_off(&mut self, ch: u32, key: u8) {
        self.audio.send(Cmd::NoteOff { ch, key });
    }

    /// Plays the key held on an on-screen keyboard, releasing the previous one.
    fn hold_key(&mut self, held: Option<(u32, u8)>) {
        if held != self.held_key {
            if let Some((c, k)) = self.held_key {
                self.note_off(c, k);
            }
            if let Some((c, k)) = held {
                self.note_on(c, k);
            }
            self.held_key = held;
        }
    }

    fn add_instrument(&mut self, name: &str, inst: Instrument) {
        let id = self.project.add_channel(name, inst);
        self.selected = Some(id);
        self.status = format!("Canal «{name}» agregado");
    }

    fn new_project(&mut self) {
        self.replace_project(Project::empty(), None);
        self.status = "Proyecto nuevo".into();
    }

    fn replace_project(&mut self, mut project: Project, path: Option<PathBuf>) {
        project.sanitize();
        self.audio.send(Cmd::Stop);
        for id in self.loaded_paths.keys() {
            self.audio.send(Cmd::Sample(*id, None));
        }
        self.loaded_paths.clear();
        self.samples.clear();
        self.selected = project.channels.first().map(|c| c.id);
        self.instrument_win = None;
        self.project = project;
        self.path = path;
        self.set_pattern(0);
        self.sync();
        self.mark_saved();
    }

    fn mark_saved(&mut self) {
        self.saved = serde_json::to_string(&self.project).unwrap_or_default();
    }

    fn is_dirty(&self) -> bool {
        serde_json::to_string(&self.project).is_ok_and(|s| s != self.saved)
    }

    fn guard(&mut self, action: Pending) {
        if self.is_dirty() {
            self.pending = Some(action);
        } else {
            self.run_pending(action);
        }
    }

    fn run_pending(&mut self, action: Pending) {
        match action {
            Pending::New => self.new_project(),
            Pending::Open => self.open_dialog(DialogPurpose::Open),
            Pending::Demo => self.replace_project(Project::demo(), None),
            Pending::Quit => {
                self.quit_ok = true;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn unsaved_modal(&mut self, ctx: &Context) {
        let Some(action) = self.pending else { return };
        let mut choice = None;
        egui::Modal::new(egui::Id::unique("unsaved")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading("Cambios sin guardar");
            ui.label(format!(
                "¿Guardar los cambios de \"{}\"?",
                self.project.name
            ));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Guardar").clicked() {
                    choice = Some(0);
                }
                if ui.button("No guardar").clicked() {
                    choice = Some(1);
                }
                if ui.button("Cancelar").clicked() {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                self.pending = None;
                self.save();
                if !self.is_dirty() {
                    self.run_pending(action);
                }
            }
            Some(1) => {
                self.pending = None;
                self.run_pending(action);
            }
            Some(2) => self.pending = None,
            _ => {}
        }
    }

    fn open_project(&mut self, path: &Path) {
        let res = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Project>(&s).map_err(|e| e.to_string()));
        match res {
            Ok(p) => {
                self.replace_project(p, Some(path.to_path_buf()));
                self.status = format!("Abierto {}", path.display());
            }
            Err(e) => self.status = format!("No se pudo abrir {}: {e}", path.display()),
        }
    }

    fn save_project(&mut self, path: &Path) {
        let path = with_ext(path, "gkp");
        let res = serde_json::to_string_pretty(&self.project)
            .map_err(|e| e.to_string())
            .and_then(|s| write_atomic(&path, s.as_bytes()).map_err(|e| e.to_string()));
        match res {
            Ok(()) => {
                self.status = format!("Guardado {}", path.display());
                self.path = Some(path);
                self.mark_saved();
            }
            Err(e) => self.status = format!("No se pudo guardar: {e}"),
        }
    }

    fn save(&mut self) {
        match self.path.clone() {
            Some(p) => self.save_project(&p),
            None => self.open_dialog(DialogPurpose::Save),
        }
    }

    fn start_export(&mut self, path: &Path) {
        if self.export_rx.is_some() {
            self.status = "Ya hay una exportación en curso".into();
            return;
        }
        let path = with_ext(path, "wav");
        let project = self.project.clone();
        let samples = self.samples.clone();
        let (mode, pattern) = (self.mode, self.pattern);
        let (tx, rx) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            let data = export::render(&project, &samples, 44100, mode, pattern);
            let res = export::write_wav(&path, &data, 44100)
                .map(|_| path)
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
        self.export_rx = Some(rx);
        self.status = "Exportando WAV…".into();
    }

    fn poll_export(&mut self) {
        if let Some(rx) = &self.export_rx
            && let Ok(res) = rx.try_recv()
        {
            self.status = match res {
                Ok(p) => format!("Exportado {}", p.display()),
                Err(e) => format!("Error al exportar: {e}"),
            };
            self.export_rx = None;
        }
    }

    fn open_dialog(&mut self, purpose: DialogPurpose) {
        let mut d = FileDialog::new();
        d = match purpose {
            DialogPurpose::Open => d
                .add_file_filter_extensions("Proyecto GrantKinoa (.gkp)", vec!["gkp", "GKP"])
                .default_file_filter("Proyecto GrantKinoa (.gkp)"),
            DialogPurpose::Save => d
                .add_save_extension("Proyecto GrantKinoa", "gkp")
                .default_save_extension("Proyecto GrantKinoa")
                .default_file_name(&format!("{}.gkp", self.project.name)),
            DialogPurpose::Export => d
                .add_save_extension("Audio WAV", "wav")
                .default_save_extension("Audio WAV")
                .default_file_name(&format!("{}.wav", self.project.name)),
            DialogPurpose::Sample(_) => d
                .add_file_filter_extensions("Audio WAV", vec!["wav", "WAV"])
                .default_file_filter("Audio WAV"),
        };
        match purpose {
            DialogPurpose::Open | DialogPurpose::Sample(_) => d.pick_file(),
            DialogPurpose::Save | DialogPurpose::Export => d.save_file(),
        }
        self.dialog = d;
        self.purpose = Some(purpose);
    }

    fn handle_dialog(&mut self, ctx: &Context) {
        self.dialog.update(ctx);
        if let Some(path) = self.dialog.take_picked() {
            match self.purpose.take() {
                Some(DialogPurpose::Open) => self.open_project(&path),
                Some(DialogPurpose::Save) => self.save_project(&path),
                Some(DialogPurpose::Export) => self.start_export(&path),
                Some(DialogPurpose::Sample(id)) => {
                    if let Some(ch) = self.project.channel_mut(id) {
                        ch.instrument.sampler.path = Some(path.to_string_lossy().into_owned());
                        if ch.name == "Sampler" {
                            ch.name = path
                                .file_stem()
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_default();
                        }
                    }
                }
                None => {}
            }
        }
    }

    /// Pushes project edits and newly assigned samples to the audio thread.
    fn sync(&mut self) {
        let mut wanted = Vec::new();
        for ch in &self.project.channels {
            if ch.instrument.kind == InstrumentKind::Sampler {
                let path = ch.instrument.sampler.path.clone();
                if self.loaded_paths.get(&ch.id) != Some(&path) {
                    wanted.push((ch.id, path));
                }
            }
        }
        for (id, path) in wanted {
            let data = match &path {
                Some(p) => match export::load_wav(Path::new(p)) {
                    Ok(d) => Some(Arc::new(d)),
                    Err(e) => {
                        self.status = format!("No se pudo cargar {p}: {e}");
                        None
                    }
                },
                None => None,
            };
            match &data {
                Some(d) => {
                    self.samples.insert(id, d.clone());
                }
                None => {
                    self.samples.remove(&id);
                }
            }
            self.audio.send(Cmd::Sample(id, data));
            self.loaded_paths.insert(id, path);
        }
        if self.project != *self.sent {
            let p = Arc::new(self.project.clone());
            self.audio.send(Cmd::Project(p.clone()));
            self.sent = p;
        }
    }

    fn handle_keys(&mut self, ctx: &Context) {
        if ctx.egui_wants_keyboard_input()
            || *self.dialog.state() == egui_file_dialog::DialogState::Open
        {
            return;
        }
        let (space, f5, f6, f7, f9, l, ctrl_s, ctrl_o, ctrl_n) = ctx.input(|i| {
            let c = i.modifiers.command;
            (
                i.key_pressed(Key::Space),
                i.key_pressed(Key::F5),
                i.key_pressed(Key::F6),
                i.key_pressed(Key::F7),
                i.key_pressed(Key::F9),
                !c && i.key_pressed(Key::L),
                c && i.key_pressed(Key::S),
                c && i.key_pressed(Key::O),
                c && i.key_pressed(Key::N),
            )
        });
        if space {
            self.toggle_play();
        }
        if f5 {
            self.show_browser = !self.show_browser;
        }
        if f6 {
            self.show_rack = !self.show_rack;
        }
        if f7 {
            self.show_piano = !self.show_piano;
        }
        if f9 {
            self.show_mixer = !self.show_mixer;
        }
        if l {
            self.set_mode(if self.mode == PlayMode::Pattern {
                PlayMode::Song
            } else {
                PlayMode::Pattern
            });
        }
        if ctrl_s {
            self.save();
        }
        if ctrl_o {
            self.guard(Pending::Open);
        }
        if ctrl_n {
            self.guard(Pending::New);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Archivo", |ui| {
                if ui.button("Nuevo proyecto   Ctrl+N").clicked() {
                    self.guard(Pending::New);
                    ui.close();
                }
                if ui.button("Abrir…   Ctrl+O").clicked() {
                    self.guard(Pending::Open);
                    ui.close();
                }
                if ui.button("Abrir demo").clicked() {
                    self.guard(Pending::Demo);
                    ui.close();
                }
                ui.separator();
                if ui.button("Guardar   Ctrl+S").clicked() {
                    self.save();
                    ui.close();
                }
                if ui.button("Guardar como…").clicked() {
                    self.open_dialog(DialogPurpose::Save);
                    ui.close();
                }
                ui.separator();
                if ui.button("Exportar WAV…").clicked() {
                    self.open_dialog(DialogPurpose::Export);
                    ui.close();
                }
                ui.separator();
                if ui.button("Salir").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Ver", |ui| {
                ui.checkbox(&mut self.show_browser, "Navegador   F5");
                ui.checkbox(&mut self.show_rack, "Channel Rack   F6");
                ui.checkbox(&mut self.show_piano, "Piano Roll   F7");
                ui.checkbox(&mut self.show_mixer, "Mixer   F9");
            });
            ui.menu_button("Ayuda", |ui| {
                ui.label(RichText::new("GrantKinoa").strong().color(ACCENT));
                ui.label(format!(
                    "Versión {} · DAW libre y gratuito hecho en Rust",
                    env!("CARGO_PKG_VERSION")
                ));
                ui.separator();
                ui.label("Espacio: reproducir / detener");
                ui.label("L: alternar Pattern / Song");
                ui.label("F5 Navegador · F6 Channel Rack · F7 Piano Roll · F9 Mixer");
                ui.label("Perillas: arrastrar · Shift = fino · doble click = valor inicial");
            });
        });
    }

    fn transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("GrantKinoa")
                    .strong()
                    .size(20.0)
                    .color(ACCENT),
            );
            ui.separator();
            if ui
                .selectable_label(self.mode == PlayMode::Pattern, "PAT")
                .on_hover_text("Reproducir el pattern actual (L)")
                .clicked()
            {
                self.set_mode(PlayMode::Pattern);
            }
            if ui
                .selectable_label(self.mode == PlayMode::Song, "SONG")
                .on_hover_text("Reproducir la playlist (L)")
                .clicked()
            {
                self.set_mode(PlayMode::Song);
            }
            let playing = self.playing();
            let icon = if playing { Icon::Pause } else { Icon::Play };
            if icon_button(ui, icon, playing, "Reproducir / detener (Espacio)").clicked() {
                self.toggle_play();
            }
            if icon_button(ui, Icon::Stop, false, "Detener").clicked() {
                self.audio.send(Cmd::Stop);
            }
            let pos = self.pos();
            let bar = (pos / 16.0) as u32 + 1;
            let beat = ((pos % 16.0) / 4.0) as u32 + 1;
            let step = (pos % 4.0) as u32 + 1;
            egui::Frame::new()
                .fill(BG_DARK)
                .inner_margin(egui::Margin::symmetric(8, 2))
                .corner_radius(3)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{bar:>3}:{beat}:{step}"))
                            .monospace()
                            .size(16.0)
                            .color(ACCENT),
                    );
                });
            ui.separator();
            ui.label("BPM");
            ui.add(
                egui::DragValue::new(&mut self.project.bpm)
                    .range(20.0..=300.0)
                    .speed(0.5)
                    .fixed_decimals(1),
            );
            ui.separator();
            ui.label("Pattern");
            let mut pat = self.pattern;
            egui::ComboBox::from_id_salt("pattern_sel")
                .width(140.0)
                .selected_text(self.project.patterns[pat].name.clone())
                .show_ui(ui, |ui| {
                    for (i, p) in self.project.patterns.iter().enumerate() {
                        ui.selectable_value(&mut pat, i, p.name.clone());
                    }
                });
            if pat != self.pattern {
                self.set_pattern(pat);
            }
            if ui.button("+").on_hover_text("Nuevo pattern").clicked() {
                let i = self.project.add_pattern();
                self.set_pattern(i);
            }
            if ui
                .button("Clonar")
                .on_hover_text("Duplicar el pattern actual")
                .clicked()
            {
                let mut p = self.project.patterns[self.pattern].clone();
                p.name = format!("{} (copia)", p.name);
                self.project.patterns.push(p);
                let i = self.project.patterns.len() - 1;
                self.set_pattern(i);
            }
            ui.separator();
            labeled(ui, "Master", |ui| {
                knob(
                    ui,
                    &mut self.project.mixer[0].volume,
                    0.0,
                    1.25,
                    0.85,
                    "Volumen master",
                )
            });
            meter(ui, self.meters[0], egui::vec2(14.0, 30.0));
            ui.separator();
            ui.toggle_value(&mut self.show_browser, "Navegador");
            ui.toggle_value(&mut self.show_rack, "Channel Rack");
            ui.toggle_value(&mut self.show_piano, "Piano Roll");
            ui.toggle_value(&mut self.show_mixer, "Mixer");
        });
    }

    fn browser(&mut self, ui: &mut egui::Ui) {
        ui.heading("Navegador");
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::CollapsingHeader::new("Instrumentos")
                .default_open(true)
                .show(ui, |ui| {
                    for kind in InstrumentKind::ALL {
                        if kind == InstrumentKind::Synth {
                            continue;
                        }
                        if ui
                            .button(kind.label())
                            .on_hover_text("Agregar al Channel Rack")
                            .clicked()
                        {
                            self.add_instrument(kind.label(), Instrument::new(kind));
                            if kind == InstrumentKind::Sampler {
                                let id = self.selected.unwrap();
                                self.instrument_win = Some(id);
                            }
                        }
                    }
                });
            egui::CollapsingHeader::new("Sintetizadores")
                .default_open(true)
                .show(ui, |ui| {
                    let presets: [SynthPreset; 5] = [
                        ("Synth básico", SynthParams::default),
                        ("Bajo", preset_bass),
                        ("Pluck", preset_pluck),
                        ("Pad", preset_pad),
                        ("Lead", preset_lead),
                    ];
                    for (name, f) in presets {
                        if ui
                            .button(name)
                            .on_hover_text("Agregar al Channel Rack")
                            .clicked()
                        {
                            let mut inst = Instrument::new(InstrumentKind::Synth);
                            inst.synth = f();
                            self.add_instrument(name, inst);
                        }
                    }
                });
            egui::CollapsingHeader::new("Patterns")
                .default_open(true)
                .show(ui, |ui| {
                    let mut clicked = None;
                    for (i, p) in self.project.patterns.iter().enumerate() {
                        let text = RichText::new(&p.name).color(rgb(p.color));
                        if ui.selectable_label(i == self.pattern, text).clicked() {
                            clicked = Some(i);
                        }
                    }
                    if let Some(i) = clicked {
                        self.set_pattern(i);
                    }
                    ui.separator();
                    let idx = self.pattern;
                    let pat = &mut self.project.patterns[idx];
                    ui.horizontal(|ui| {
                        ui.label("Nombre");
                        ui.text_edit_singleline(&mut pat.name);
                    });
                    let mut remove = false;
                    ui.horizontal(|ui| {
                        ui.label("Color");
                        ui.color_edit_button_srgb(&mut pat.color);
                        remove = ui.button("Eliminar").clicked();
                    });
                    if remove {
                        self.project.remove_pattern(idx);
                        self.set_pattern(idx.saturating_sub(1));
                    }
                });
            egui::CollapsingHeader::new("Proyecto")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Nombre");
                        ui.text_edit_singleline(&mut self.project.name);
                    });
                    ui.label(format!(
                        "{} canales · {} patterns",
                        self.project.channels.len(),
                        self.project.patterns.len()
                    ));
                    let bars = self.project.song_length() / STEPS_PER_BAR;
                    let secs = self.project.song_length() as f32 * 15.0 / self.project.bpm;
                    ui.label(format!(
                        "Canción: {bars} compases ({:.0}:{:02.0})",
                        (secs / 60.0).floor(),
                        secs % 60.0
                    ));
                });
        });
    }
}

impl eframe::App for GrantKinoa {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_ok && self.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
        self.unsaved_modal(&ctx);
        self.handle_keys(&ctx);
        self.poll_export();
        for (i, m) in self.meters.iter_mut().enumerate() {
            let p = self.audio.shared.take_meter(i);
            *m = [p[0].max(m[0] * 0.85), p[1].max(m[1] * 0.85)];
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("transport").show(ui, |ui| self.transport(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).small().color(TEXT_DIM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("Audio: {}", self.audio.status))
                            .small()
                            .color(TEXT_DIM),
                    );
                });
            });
        });
        if self.show_browser {
            egui::Panel::left("browser")
                .resizable(true)
                .default_size(190.0)
                .show(ui, |ui| self.browser(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| self.playlist_ui(ui));

        self.channel_rack_window(&ctx);
        self.piano_roll_window(&ctx);
        self.mixer_window(&ctx);
        self.instrument_window(&ctx);
        self.handle_dialog(&ctx);
        self.sync();

        let title = format!("GrantKinoa — {}", self.project.name);
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        if self.playing() || self.meters.iter().any(|m| m[0] > 1e-3 || m[1] > 1e-3) {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

fn with_ext(path: &Path, ext: &str) -> PathBuf {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    {
        path.to_path_buf()
    } else {
        path.with_extension(ext)
    }
}

fn setup_style(ctx: &Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = egui::Color32::from_rgb(44, 49, 53);
    v.extreme_bg_color = BG_DARK;
    v.faint_bg_color = egui::Color32::from_rgb(58, 64, 69);
    v.selection.bg_fill = shade(ACCENT, 0.7);
    v.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    v.hyperlink_color = ACCENT;
    v.widgets.inactive.weak_bg_fill = BG_LIGHT;
    v.widgets.inactive.bg_fill = BG_LIGHT;
    v.widgets.hovered.weak_bg_fill = shade(BG_LIGHT, 1.25);
    v.widgets.active.weak_bg_fill = shade(ACCENT, 0.8);
    v.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(25));
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
    });
}

fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
