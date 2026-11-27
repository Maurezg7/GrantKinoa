//! Plantillas de proyecto por género musical.

use crate::project::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre {
    HipHop,
    Trap,
    Rnb,
    Pop,
    PopRock,
    Reggaeton,
    Reggae,
}

impl Genre {
    pub const ALL: [Genre; 7] = [
        Genre::HipHop,
        Genre::Trap,
        Genre::Rnb,
        Genre::Pop,
        Genre::PopRock,
        Genre::Reggaeton,
        Genre::Reggae,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Genre::HipHop => "Hip hop (boom bap)",
            Genre::Trap => "Trap",
            Genre::Rnb => "R&B",
            Genre::Pop => "Pop",
            Genre::PopRock => "Pop-rock",
            Genre::Reggaeton => "Reggaetón (dembow)",
            Genre::Reggae => "Reggae (one drop)",
        }
    }
}

const LEN: u32 = 4 * STEPS_PER_BAR;

/// Repite un patrón de un compás en los 4 compases.
fn bars(steps: &[usize]) -> Vec<usize> {
    (0..4)
        .flat_map(|b| steps.iter().map(move |s| b * 16 + s))
        .collect()
}

/// Acordes, uno por compás, tocados en los pasos `hits` con duración `len`.
fn chords(prog: &[&[u8]; 4], hits: &[usize], len: f32) -> Vec<(f32, f32, u8)> {
    let mut out = Vec::new();
    for (b, keys) in prog.iter().enumerate() {
        for &h in hits {
            for &k in keys.iter() {
                out.push(((b * 16 + h) as f32, len, k));
            }
        }
    }
    out
}

struct Builder {
    p: Project,
}

impl Builder {
    fn new(name: &str, bpm: f32, swing: f32) -> Self {
        let mut p = Project::empty();
        p.name = name.into();
        p.bpm = bpm;
        p.swing = swing;
        p.patterns[0].name = "Loop".into();
        p.patterns[0].length = LEN;
        p.mixer[0].fx.limiter_on = true;
        Self { p }
    }

    fn add(&mut self, name: &str, inst: Instrument) -> u32 {
        let id = self.p.add_channel(name, inst);
        if let Some(ch) = self.p.channels.iter().find(|c| c.id == id)
            && ch.mixer > 0
        {
            self.p.mixer[ch.mixer].name = name.into();
        }
        id
    }

    fn kind(&mut self, name: &str, kind: InstrumentKind) -> u32 {
        self.add(name, Instrument::new(kind))
    }

    fn tone(&mut self, name: &str, kind: InstrumentKind, f: impl FnOnce(&mut ToneParams)) -> u32 {
        let mut inst = Instrument::new(kind);
        f(&mut inst.tone);
        self.add(name, inst)
    }

    fn synth(&mut self, name: &str, preset: fn() -> SynthParams) -> u32 {
        let mut inst = Instrument::new(InstrumentKind::Synth);
        inst.synth = preset();
        self.add(name, inst)
    }

    fn steps(&mut self, ch: u32, steps: &[usize]) {
        let cp = self.p.patterns[0].channel_mut(ch);
        for &s in steps {
            if let Some(v) = cp.steps.get_mut(s) {
                *v = true;
            }
        }
    }

    fn notes(&mut self, ch: u32, notes: &[(f32, f32, u8)], vel: f32) {
        let cp = self.p.patterns[0].channel_mut(ch);
        cp.notes.extend(notes.iter().map(|&(start, len, key)| Note {
            start,
            len,
            key,
            vel,
        }));
    }

    fn fx(&mut self, ch: u32, f: impl FnOnce(&mut FxParams)) {
        if let Some(i) = self.p.channels.iter().find(|c| c.id == ch).map(|c| c.mixer) {
            f(&mut self.p.mixer[i].fx);
        }
    }

    fn insert_of(&self, ch: u32) -> usize {
        self.p
            .channels
            .iter()
            .find(|c| c.id == ch)
            .map(|c| c.mixer)
            .unwrap_or(0)
    }

    fn duck(&mut self, ch: u32, kick: u32, depth: f32) {
        let src = self.insert_of(kick);
        self.fx(ch, |f| {
            f.sc_source = src;
            f.sc_depth = depth;
        });
    }

    fn finish(mut self) -> Project {
        for k in 0..4 {
            self.p.clips.push(Clip {
                track: 0,
                pattern: 0,
                start: k * LEN,
            });
        }
        self.p.sanitize();
        self.p
    }
}

fn drum_bus(f: &mut FxParams) {
    f.comp_on = true;
    f.comp_thresh = -14.0;
    f.comp_ratio = 4.0;
    f.comp_makeup = 2.0;
}

fn roots(keys: [u8; 4], hits: &[(usize, f32)]) -> Vec<(f32, f32, u8)> {
    let mut out = Vec::new();
    for (b, k) in keys.iter().enumerate() {
        for &(h, len) in hits {
            out.push(((b * 16 + h) as f32, len, *k));
        }
    }
    out
}

impl Project {
    pub fn template(genre: Genre) -> Project {
        use InstrumentKind as K;
        match genre {
            Genre::HipHop => {
                let mut b = Builder::new("Hip hop", 90.0, 0.3);
                let kick = b.kind("Bombo", K::Kick);
                let snare = b.kind("Caja", K::Snare);
                let hat = b.kind("Hi-hat", K::HiHat);
                let keys = b.tone("Keys", K::EPiano, |t| t.bright = 0.35);
                let bass = b.tone("808", K::Bass808, |t| t.glide = 0.0);
                b.steps(kick, &bars(&[0, 7, 10]));
                b.steps(snare, &bars(&[4, 12]));
                b.steps(hat, &bars(&[0, 2, 4, 6, 8, 10, 12, 14]));
                let prog: [&[u8]; 4] = [
                    &[60, 63, 67, 70, 74],
                    &[56, 60, 63, 67],
                    &[53, 56, 60, 63, 67],
                    &[55, 59, 62, 65],
                ];
                b.notes(keys, &chords(&prog, &[0], 14.0), 0.6);
                b.notes(bass, &roots([36, 32, 41, 43], &[(0, 6.0), (10, 4.0)]), 0.8);
                b.fx(kick, drum_bus);
                b.fx(snare, |f| {
                    f.sat_on = true;
                    f.reverb_on = true;
                    f.reverb_mix = 0.12;
                });
                b.fx(hat, |f| {
                    f.crush_on = true;
                    f.crush_bits = 10.0;
                    f.crush_down = 2.0;
                });
                b.fx(keys, |f| {
                    f.chorus_on = true;
                    f.reverb_on = true;
                    f.reverb_mix = 0.25;
                });
                b.fx(bass, |f| f.sat_on = true);
                b.finish()
            }
            Genre::Trap => {
                let mut b = Builder::new("Trap", 140.0, 0.0);
                let kick = b.kind("Bombo", K::Kick);
                let clap = b.kind("Clap", K::Clap);
                let hat = b.kind("Hi-hat", K::HiHat);
                let ohat = b.kind("Hi-hat abierto", K::OpenHat);
                let bass = b.tone("808", K::Bass808, |t| {
                    t.glide = 0.08;
                    t.drive = 0.45;
                });
                let pad = b.tone("Pad", K::Strings, |t| t.bright = 0.25);
                let bell = b.tone("Bell", K::EPiano, |t| {
                    t.bright = 0.95;
                    t.decay = 0.8;
                    t.sustain = 0.0;
                });
                b.steps(kick, &[0, 10, 16, 19, 26, 32, 42, 48, 51, 58]);
                b.steps(clap, &bars(&[8]));
                b.steps(hat, &(0..64).collect::<Vec<_>>());
                b.steps(ohat, &bars(&[14]));
                b.notes(
                    bass,
                    &[
                        (0.0, 7.0, 33),
                        (6.0, 4.0, 45),
                        (16.0, 10.0, 29),
                        (32.0, 7.0, 36),
                        (38.0, 4.0, 43),
                        (48.0, 10.0, 31),
                    ],
                    0.85,
                );
                let prog: [&[u8]; 4] = [&[57, 60, 64], &[53, 57, 60], &[55, 60, 64], &[55, 59, 62]];
                b.notes(pad, &chords(&prog, &[0], 16.0), 0.45);
                let mel: [&[u8]; 4] = [&[81], &[77], &[79], &[74]];
                b.notes(bell, &chords(&mel, &[0, 3, 6, 10], 2.0), 0.5);
                b.fx(kick, drum_bus);
                b.fx(bass, |f| f.sat_on = true);
                b.duck(pad, kick, 0.4);
                b.fx(pad, |f| {
                    f.reverb_on = true;
                    f.reverb_mix = 0.35;
                });
                b.fx(bell, |f| {
                    f.delay_on = true;
                    f.reverb_on = true;
                });
                b.finish()
            }
            Genre::Rnb => {
                let mut b = Builder::new("R&B", 72.0, 0.2);
                let kick = b.kind("Bombo", K::Kick);
                let snare = b.kind("Caja", K::Snare);
                let rim = b.kind("Rimshot", K::Rim);
                let shaker = b.kind("Shaker", K::Shaker);
                let keys = b.tone("Keys", K::EPiano, |t| t.spread = 0.6);
                let bass = b.tone("808", K::Bass808, |t| t.glide = 0.05);
                let pad = b.kind("Cuerdas", K::Strings);
                b.steps(kick, &bars(&[0, 10]));
                b.steps(snare, &bars(&[12]));
                b.steps(rim, &bars(&[4, 7]));
                b.steps(shaker, &bars(&[0, 2, 4, 6, 8, 10, 12, 14]));
                let prog: [&[u8]; 4] = [
                    &[53, 57, 60, 64, 67],
                    &[52, 55, 59, 62],
                    &[50, 53, 57, 60, 64],
                    &[48, 52, 55, 59, 62],
                ];
                b.notes(keys, &chords(&prog, &[0, 10], 6.0), 0.55);
                b.notes(pad, &chords(&prog, &[0], 16.0), 0.3);
                b.notes(bass, &roots([41, 40, 38, 36], &[(0, 8.0), (10, 5.0)]), 0.75);
                b.fx(kick, drum_bus);
                b.fx(keys, |f| {
                    f.eq_on = true;
                    f.eq = [-2.0, 0.0, 2.0];
                    f.chorus_on = true;
                    f.reverb_on = true;
                    f.reverb_mix = 0.3;
                });
                b.duck(pad, kick, 0.3);
                b.fx(pad, |f| {
                    f.reverb_on = true;
                    f.reverb_mix = 0.4;
                });
                b.finish()
            }
            Genre::Pop => {
                let mut b = Builder::new("Pop", 118.0, 0.0);
                let kick = b.kind("Bombo", K::Kick);
                let clap = b.kind("Clap", K::Clap);
                let hat = b.kind("Hi-hat", K::HiHat);
                let saw = b.kind("Supersaw", K::Supersaw);
                let bass = b.synth("Bajo", preset_bass);
                let lead = b.synth("Pluck", preset_pluck);
                b.steps(kick, &bars(&[0, 4, 8, 12]));
                b.steps(clap, &bars(&[4, 12]));
                b.steps(hat, &bars(&[2, 6, 10, 14]));
                let prog: [&[u8]; 4] = [&[60, 64, 67], &[59, 62, 67], &[57, 60, 64], &[57, 60, 65]];
                b.notes(saw, &chords(&prog, &[0, 6, 10], 3.5), 0.55);
                b.notes(
                    bass,
                    &roots(
                        [36, 43, 45, 41],
                        &[
                            (0, 1.5),
                            (2, 1.5),
                            (4, 1.5),
                            (6, 1.5),
                            (8, 1.5),
                            (10, 1.5),
                            (12, 1.5),
                            (14, 1.5),
                        ],
                    ),
                    0.8,
                );
                let mel: [&[u8]; 4] = [&[72], &[74], &[76], &[72]];
                b.notes(lead, &chords(&mel, &[0, 3, 6, 8, 11], 1.5), 0.6);
                b.fx(kick, drum_bus);
                b.duck(saw, kick, 0.6);
                b.duck(bass, kick, 0.5);
                b.fx(saw, |f| {
                    f.reverb_on = true;
                    f.reverb_mix = 0.2;
                });
                b.fx(lead, |f| f.delay_on = true);
                b.finish()
            }
            Genre::PopRock => {
                let mut b = Builder::new("Pop-rock", 128.0, 0.0);
                let kick = b.kind("Bombo", K::Kick);
                let snare = b.kind("Caja", K::Snare);
                let hat = b.kind("Hi-hat", K::HiHat);
                let tom = b.kind("Tom", K::Tom);
                let gtr = b.tone("Guitarra", K::Guitar, |t| t.drive = 0.7);
                let bass = b.synth("Bajo", preset_bass);
                let organ = b.kind("Órgano", K::Organ);
                b.steps(kick, &bars(&[0, 6, 8]));
                b.steps(snare, &bars(&[4, 12]));
                b.steps(hat, &bars(&[0, 2, 4, 6, 8, 10, 12, 14]));
                b.steps(tom, &[60, 61, 62, 63]);
                let power: [&[u8]; 4] =
                    [&[40, 47, 52], &[36, 43, 48], &[43, 50, 55], &[38, 45, 50]];
                b.notes(gtr, &chords(&power, &[0, 2, 4, 6, 8, 10, 12, 14], 1.8), 0.7);
                b.notes(
                    bass,
                    &roots(
                        [28, 24, 31, 26],
                        &[
                            (0, 1.8),
                            (2, 1.8),
                            (4, 1.8),
                            (6, 1.8),
                            (8, 1.8),
                            (10, 1.8),
                            (12, 1.8),
                            (14, 1.8),
                        ],
                    ),
                    0.8,
                );
                let prog: [&[u8]; 4] = [&[64, 67, 71], &[60, 64, 67], &[62, 67, 71], &[62, 66, 69]];
                b.notes(organ, &chords(&prog, &[0], 16.0), 0.35);
                b.fx(kick, drum_bus);
                b.fx(snare, |f| {
                    f.reverb_on = true;
                    f.reverb_mix = 0.2;
                });
                b.fx(gtr, |f| {
                    f.eq_on = true;
                    f.eq = [-4.0, 2.0, 1.0];
                    f.comp_on = true;
                });
                b.finish()
            }
            Genre::Reggaeton => {
                let mut b = Builder::new("Reggaetón", 95.0, 0.0);
                let kick = b.kind("Bombo", K::Kick);
                let snare = b.kind("Caja", K::Snare);
                let hat = b.kind("Hi-hat", K::HiHat);
                let rim = b.kind("Rimshot", K::Rim);
                let shaker = b.kind("Shaker", K::Shaker);
                let bass = b.tone("808", K::Bass808, |t| t.drive = 0.35);
                let keys = b.synth("Pluck", preset_pluck);
                let pad = b.kind("Cuerdas", K::Strings);
                b.steps(kick, &bars(&[0, 4, 8, 12]));
                b.steps(snare, &bars(&[3, 6, 11, 14]));
                b.steps(hat, &bars(&[2, 6, 10, 14]));
                b.steps(rim, &bars(&[7, 15]));
                b.steps(shaker, &bars(&[0, 2, 4, 6, 8, 10, 12, 14]));
                b.notes(
                    bass,
                    &roots([45, 41, 36, 43], &[(0, 3.0), (3, 3.0), (8, 3.0), (11, 3.0)]),
                    0.8,
                );
                let prog: [&[u8]; 4] = [&[69, 72, 76], &[65, 69, 72], &[67, 72, 76], &[67, 71, 74]];
                b.notes(keys, &chords(&prog, &[0, 3, 6, 8, 11, 14], 1.5), 0.55);
                b.notes(pad, &chords(&prog, &[0], 16.0), 0.25);
                b.fx(kick, drum_bus);
                b.fx(bass, |f| f.sat_on = true);
                b.duck(pad, kick, 0.5);
                b.fx(keys, |f| {
                    f.delay_on = true;
                    f.delay_mix = 0.15;
                });
                b.finish()
            }
            Genre::Reggae => {
                let mut b = Builder::new("Reggae", 76.0, 0.2);
                let kick = b.kind("Bombo", K::Kick);
                let rim = b.kind("Rimshot", K::Rim);
                let hat = b.kind("Hi-hat", K::HiHat);
                let gtr = b.tone("Skank", K::Guitar, |t| {
                    t.bright = 0.85;
                    t.decay = 0.6;
                });
                let organ = b.kind("Órgano", K::Organ);
                let bass = b.synth("Bajo", preset_bass);
                b.steps(kick, &bars(&[8]));
                b.steps(rim, &bars(&[8]));
                b.steps(hat, &bars(&[0, 2, 4, 6, 8, 10, 12, 14]));
                let prog: [&[u8]; 4] = [&[57, 60, 64], &[55, 59, 62], &[53, 57, 60], &[55, 59, 62]];
                b.notes(gtr, &chords(&prog, &[4, 12], 1.0), 0.6);
                b.notes(organ, &chords(&prog, &[2, 6, 10, 14], 0.8), 0.35);
                b.notes(
                    bass,
                    &roots([45, 43, 41, 43], &[(0, 3.0), (6, 2.0), (10, 4.0)]),
                    0.85,
                );
                b.fx(gtr, |f| {
                    f.delay_on = true;
                    f.delay_mix = 0.3;
                    f.reverb_on = true;
                });
                b.fx(rim, |f| {
                    f.reverb_on = true;
                    f.reverb_mix = 0.3;
                });
                b.finish()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::PlayMode;
    use std::collections::HashMap;

    #[test]
    fn every_template_renders_audio() {
        for g in Genre::ALL {
            let p = Project::template(g);
            assert!(p.channels.len() <= MIXER_INSERTS, "{g:?}");
            let buf = crate::export::render(&p, &HashMap::new(), 44100, PlayMode::Pattern, 0);
            let peak = buf.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(buf.iter().all(|v| v.is_finite()), "{g:?}");
            assert!(peak > 0.05 && peak <= 1.0, "{g:?} peak {peak}");
        }
    }
}
