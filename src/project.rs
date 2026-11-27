use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const STEPS_PER_BAR: u32 = 16;
pub const PLAYLIST_TRACKS: usize = 16;
pub const MIXER_INSERTS: usize = 8;
pub const DEFAULT_KEY: u8 = 60;

pub type SynthPreset = (&'static str, fn() -> SynthParams);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentKind {
    Kick,
    Snare,
    HiHat,
    OpenHat,
    Clap,
    Synth,
    Sampler,
}

impl InstrumentKind {
    pub const ALL: [InstrumentKind; 7] = [
        InstrumentKind::Kick,
        InstrumentKind::Snare,
        InstrumentKind::HiHat,
        InstrumentKind::OpenHat,
        InstrumentKind::Clap,
        InstrumentKind::Synth,
        InstrumentKind::Sampler,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InstrumentKind::Kick => "Bombo",
            InstrumentKind::Snare => "Caja",
            InstrumentKind::HiHat => "Hi-hat cerrado",
            InstrumentKind::OpenHat => "Hi-hat abierto",
            InstrumentKind::Clap => "Palmas",
            InstrumentKind::Synth => "Sintetizador",
            InstrumentKind::Sampler => "Sampler",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wave {
    Sine,
    Saw,
    Square,
    Triangle,
    Noise,
}

impl Wave {
    pub const ALL: [Wave; 5] = [
        Wave::Sine,
        Wave::Saw,
        Wave::Square,
        Wave::Triangle,
        Wave::Noise,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Wave::Sine => "Seno",
            Wave::Saw => "Sierra",
            Wave::Square => "Cuadrada",
            Wave::Triangle => "Triángulo",
            Wave::Noise => "Ruido",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OscParams {
    pub wave: Wave,
    pub octave: i32,
    pub fine: f32,
    pub level: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SynthParams {
    pub osc: [OscParams; 3],
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub cutoff: f32,
    pub resonance: f32,
    pub env_amount: f32,
}

impl Default for SynthParams {
    fn default() -> Self {
        Self {
            osc: [
                OscParams {
                    wave: Wave::Saw,
                    octave: 0,
                    fine: 0.0,
                    level: 0.8,
                },
                OscParams {
                    wave: Wave::Square,
                    octave: -1,
                    fine: 6.0,
                    level: 0.4,
                },
                OscParams {
                    wave: Wave::Saw,
                    octave: 0,
                    fine: -7.0,
                    level: 0.4,
                },
            ],
            attack: 0.005,
            decay: 0.25,
            sustain: 0.6,
            release: 0.2,
            cutoff: 2400.0,
            resonance: 0.25,
            env_amount: 0.4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrumParams {
    pub tune: f32,
    pub decay: f32,
    pub tone: f32,
}

impl Default for DrumParams {
    fn default() -> Self {
        Self {
            tune: 0.0,
            decay: 1.0,
            tone: 0.5,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SamplerParams {
    pub path: Option<String>,
    pub root: u8,
    pub attack: f32,
    pub release: f32,
}

impl Default for SamplerParams {
    fn default() -> Self {
        Self {
            path: None,
            root: DEFAULT_KEY,
            attack: 0.001,
            release: 0.05,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instrument {
    pub kind: InstrumentKind,
    #[serde(default)]
    pub drum: DrumParams,
    #[serde(default)]
    pub synth: SynthParams,
    #[serde(default)]
    pub sampler: SamplerParams,
}

impl Instrument {
    pub fn new(kind: InstrumentKind) -> Self {
        Self {
            kind,
            drum: DrumParams::default(),
            synth: SynthParams::default(),
            sampler: SamplerParams::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub id: u32,
    pub name: String,
    pub color: [u8; 3],
    pub instrument: Instrument,
    pub volume: f32,
    pub pan: f32,
    pub mute: bool,
    pub mixer: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub start: f32,
    pub len: f32,
    pub key: u8,
    pub vel: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelPattern {
    pub steps: Vec<bool>,
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub name: String,
    pub color: [u8; 3],
    pub length: u32,
    pub channels: BTreeMap<u32, ChannelPattern>,
}

impl Pattern {
    pub fn new(name: impl Into<String>, color: [u8; 3]) -> Self {
        Self {
            name: name.into(),
            color,
            length: STEPS_PER_BAR,
            channels: BTreeMap::new(),
        }
    }

    pub fn channel_mut(&mut self, id: u32) -> &mut ChannelPattern {
        let len = self.length as usize;
        let cp = self.channels.entry(id).or_default();
        if cp.steps.len() < len {
            cp.steps.resize(len, false);
        }
        cp
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub track: usize,
    pub pattern: usize,
    pub start: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FxParams {
    pub filter_on: bool,
    pub cutoff: f32,
    pub resonance: f32,
    pub delay_on: bool,
    pub delay_steps: f32,
    pub delay_feedback: f32,
    pub delay_mix: f32,
    pub reverb_on: bool,
    pub reverb_size: f32,
    pub reverb_damp: f32,
    pub reverb_mix: f32,
}

impl Default for FxParams {
    fn default() -> Self {
        Self {
            filter_on: false,
            cutoff: 8000.0,
            resonance: 0.1,
            delay_on: false,
            delay_steps: 3.0,
            delay_feedback: 0.35,
            delay_mix: 0.25,
            reverb_on: false,
            reverb_size: 0.7,
            reverb_damp: 0.4,
            reverb_mix: 0.25,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MixerTrack {
    pub name: String,
    pub volume: f32,
    pub pan: f32,
    pub mute: bool,
    #[serde(default)]
    pub fx: FxParams,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub bpm: f32,
    pub channels: Vec<Channel>,
    pub patterns: Vec<Pattern>,
    pub clips: Vec<Clip>,
    pub mixer: Vec<MixerTrack>,
    pub next_channel_id: u32,
}

pub const PALETTE: [[u8; 3]; 8] = [
    [255, 157, 46],
    [110, 196, 98],
    [86, 160, 232],
    [232, 96, 112],
    [176, 120, 230],
    [240, 210, 80],
    [80, 210, 200],
    [230, 130, 190],
];

impl Default for Project {
    fn default() -> Self {
        Self::empty()
    }
}

impl Project {
    pub fn empty() -> Self {
        let mut mixer = vec![MixerTrack {
            name: "Master".into(),
            volume: 0.85,
            pan: 0.0,
            mute: false,
            fx: FxParams::default(),
        }];
        for i in 1..=MIXER_INSERTS {
            mixer.push(MixerTrack {
                name: format!("Insert {i}"),
                volume: 0.8,
                pan: 0.0,
                mute: false,
                fx: FxParams::default(),
            });
        }
        Self {
            name: "Sin título".into(),
            bpm: 128.0,
            channels: Vec::new(),
            patterns: vec![Pattern::new("Pattern 1", PALETTE[0])],
            clips: Vec::new(),
            mixer,
            next_channel_id: 1,
        }
    }

    pub fn add_channel(&mut self, name: impl Into<String>, instrument: Instrument) -> u32 {
        let id = self.next_channel_id;
        self.next_channel_id += 1;
        let color = PALETTE[(id as usize - 1) % PALETTE.len()];
        let mixer = if (id as usize) <= MIXER_INSERTS {
            id as usize
        } else {
            0
        };
        self.channels.push(Channel {
            id,
            name: name.into(),
            color,
            instrument,
            volume: 0.78,
            pan: 0.0,
            mute: false,
            mixer,
        });
        id
    }

    pub fn remove_channel(&mut self, id: u32) {
        self.channels.retain(|c| c.id != id);
        for p in &mut self.patterns {
            p.channels.remove(&id);
        }
    }

    pub fn channel(&self, id: u32) -> Option<&Channel> {
        self.channels.iter().find(|c| c.id == id)
    }

    pub fn channel_mut(&mut self, id: u32) -> Option<&mut Channel> {
        self.channels.iter_mut().find(|c| c.id == id)
    }

    pub fn add_pattern(&mut self) -> usize {
        let n = self.patterns.len();
        self.patterns.push(Pattern::new(
            format!("Pattern {}", n + 1),
            PALETTE[n % PALETTE.len()],
        ));
        n
    }

    pub fn remove_pattern(&mut self, idx: usize) {
        if self.patterns.len() <= 1 || idx >= self.patterns.len() {
            return;
        }
        self.patterns.remove(idx);
        self.clips.retain(|c| c.pattern != idx);
        for c in &mut self.clips {
            if c.pattern > idx {
                c.pattern -= 1;
            }
        }
    }

    /// Song length in steps, rounded up to whole bars.
    pub fn song_length(&self) -> u32 {
        let end = self
            .clips
            .iter()
            .filter_map(|c| self.patterns.get(c.pattern).map(|p| c.start + p.length))
            .max()
            .unwrap_or(0);
        end.div_ceil(STEPS_PER_BAR) * STEPS_PER_BAR
    }

    pub fn sanitize(&mut self) {
        while self.mixer.len() < MIXER_INSERTS + 1 {
            let i = self.mixer.len();
            self.mixer.push(MixerTrack {
                name: format!("Insert {i}"),
                volume: 0.8,
                pan: 0.0,
                mute: false,
                fx: FxParams::default(),
            });
        }
        if self.patterns.is_empty() {
            self.patterns.push(Pattern::new("Pattern 1", PALETTE[0]));
        }
        let n = self.patterns.len();
        self.clips
            .retain(|c| c.pattern < n && c.track < PLAYLIST_TRACKS);
        for ch in &mut self.channels {
            ch.mixer = ch.mixer.min(MIXER_INSERTS);
        }
        let max_id = self.channels.iter().map(|c| c.id).max().unwrap_or(0);
        self.next_channel_id = self.next_channel_id.max(max_id + 1);
        self.bpm = self.bpm.clamp(20.0, 300.0);
    }

    pub fn demo() -> Self {
        let mut p = Self::empty();
        p.name = "Demo GrantKinoa".into();
        p.bpm = 124.0;

        let kick = p.add_channel("Bombo", Instrument::new(InstrumentKind::Kick));
        let clap = p.add_channel("Palmas", Instrument::new(InstrumentKind::Clap));
        let hat = p.add_channel("Hi-hat", Instrument::new(InstrumentKind::HiHat));
        let ohat = p.add_channel("Hi-hat abierto", Instrument::new(InstrumentKind::OpenHat));

        let mut bass = Instrument::new(InstrumentKind::Synth);
        bass.synth = preset_bass();
        let bass = p.add_channel("Bajo", bass);

        let mut lead = Instrument::new(InstrumentKind::Synth);
        lead.synth = preset_pluck();
        let lead = p.add_channel("Pluck", lead);

        let mut pad = Instrument::new(InstrumentKind::Synth);
        pad.synth = preset_pad();
        let pad = p.add_channel("Pad", pad);

        p.mixer[1].name = "Bombo".into();
        p.mixer[2].name = "Palmas".into();
        p.mixer[2].fx.reverb_on = true;
        p.mixer[2].fx.reverb_mix = 0.18;
        p.mixer[3].name = "Hats".into();
        p.mixer[3].volume = 0.6;
        p.mixer[4].name = "Hats 2".into();
        p.mixer[4].volume = 0.5;
        p.mixer[5].name = "Bajo".into();
        p.mixer[5].fx.filter_on = true;
        p.mixer[5].fx.cutoff = 1800.0;
        p.mixer[6].name = "Pluck".into();
        p.mixer[6].fx.delay_on = true;
        p.mixer[6].fx.reverb_on = true;
        p.mixer[7].name = "Pad".into();
        p.mixer[7].volume = 0.55;
        p.mixer[7].fx.reverb_on = true;
        p.mixer[7].fx.reverb_mix = 0.4;

        let beat = &mut p.patterns[0];
        beat.name = "Ritmo".into();
        for s in [0, 4, 8, 12] {
            beat.channel_mut(kick).steps[s] = true;
        }
        for s in [4, 12] {
            beat.channel_mut(clap).steps[s] = true;
        }
        for s in [2, 6, 10, 14] {
            beat.channel_mut(hat).steps[s] = true;
        }
        beat.channel_mut(ohat).steps[14] = true;

        let mel = p.add_pattern();
        let mel = &mut p.patterns[mel];
        mel.name = "Bajo y melodía".into();
        mel.length = 32;
        // A minor: Am - F - C - G
        let roots = [45u8, 41, 48, 43];
        for (bar_half, root) in roots.iter().enumerate() {
            let base = bar_half as f32 * 8.0;
            for (i, off) in [0.0, 3.0, 6.0].iter().enumerate() {
                let key = if i == 1 { root + 12 } else { *root };
                mel.channel_mut(bass).notes.push(Note {
                    start: base + off,
                    len: 1.5,
                    key,
                    vel: 0.85,
                });
            }
        }
        let melody = [
            (0.0, 69),
            (2.0, 72),
            (4.0, 76),
            (6.0, 74),
            (8.0, 72),
            (10.0, 69),
            (12.0, 65),
            (14.0, 69),
            (16.0, 72),
            (18.0, 76),
            (20.0, 79),
            (22.0, 76),
            (24.0, 74),
            (26.0, 71),
            (28.0, 67),
            (30.0, 71),
        ];
        for (start, key) in melody {
            mel.channel_mut(lead).notes.push(Note {
                start,
                len: 1.5,
                key,
                vel: 0.7,
            });
        }
        let chords: [[u8; 3]; 4] = [[57, 60, 64], [53, 57, 60], [55, 60, 64], [55, 59, 62]];
        for (i, chord) in chords.iter().enumerate() {
            for key in chord {
                mel.channel_mut(pad).notes.push(Note {
                    start: i as f32 * 8.0,
                    len: 8.0,
                    key: *key,
                    vel: 0.6,
                });
            }
        }

        for bar in 0..8 {
            p.clips.push(Clip {
                track: 0,
                pattern: 0,
                start: bar * STEPS_PER_BAR,
            });
        }
        for bar in [2, 4, 6] {
            p.clips.push(Clip {
                track: 1,
                pattern: 1,
                start: bar * STEPS_PER_BAR,
            });
        }
        p
    }
}

pub fn preset_bass() -> SynthParams {
    SynthParams {
        osc: [
            OscParams {
                wave: Wave::Saw,
                octave: 0,
                fine: 0.0,
                level: 0.7,
            },
            OscParams {
                wave: Wave::Square,
                octave: -1,
                fine: 0.0,
                level: 0.6,
            },
            OscParams {
                wave: Wave::Sine,
                octave: -1,
                fine: 0.0,
                level: 0.5,
            },
        ],
        attack: 0.002,
        decay: 0.18,
        sustain: 0.5,
        release: 0.08,
        cutoff: 700.0,
        resonance: 0.35,
        env_amount: 0.55,
    }
}

pub fn preset_pluck() -> SynthParams {
    SynthParams {
        osc: [
            OscParams {
                wave: Wave::Saw,
                octave: 0,
                fine: 0.0,
                level: 0.6,
            },
            OscParams {
                wave: Wave::Saw,
                octave: 0,
                fine: 9.0,
                level: 0.5,
            },
            OscParams {
                wave: Wave::Square,
                octave: 1,
                fine: -5.0,
                level: 0.2,
            },
        ],
        attack: 0.001,
        decay: 0.22,
        sustain: 0.0,
        release: 0.25,
        cutoff: 1200.0,
        resonance: 0.3,
        env_amount: 0.7,
    }
}

pub fn preset_pad() -> SynthParams {
    SynthParams {
        osc: [
            OscParams {
                wave: Wave::Saw,
                octave: 0,
                fine: -12.0,
                level: 0.5,
            },
            OscParams {
                wave: Wave::Saw,
                octave: 0,
                fine: 12.0,
                level: 0.5,
            },
            OscParams {
                wave: Wave::Triangle,
                octave: 1,
                fine: 0.0,
                level: 0.3,
            },
        ],
        attack: 0.6,
        decay: 0.8,
        sustain: 0.8,
        release: 1.2,
        cutoff: 1800.0,
        resonance: 0.15,
        env_amount: 0.2,
    }
}

pub fn preset_lead() -> SynthParams {
    SynthParams {
        osc: [
            OscParams {
                wave: Wave::Square,
                octave: 0,
                fine: 0.0,
                level: 0.6,
            },
            OscParams {
                wave: Wave::Saw,
                octave: 0,
                fine: 7.0,
                level: 0.5,
            },
            OscParams {
                wave: Wave::Sine,
                octave: 1,
                fine: 0.0,
                level: 0.25,
            },
        ],
        attack: 0.01,
        decay: 0.3,
        sustain: 0.7,
        release: 0.3,
        cutoff: 3500.0,
        resonance: 0.3,
        env_amount: 0.3,
    }
}

pub fn note_name(key: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[(key % 12) as usize], key / 12)
}

pub fn is_black_key(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_roundtrip() {
        let p = Project::demo();
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
        assert_eq!(p.song_length(), 8 * STEPS_PER_BAR);
    }

    #[test]
    fn removing_pattern_reindexes_clips() {
        let mut p = Project::demo();
        p.remove_pattern(0);
        assert!(p.clips.iter().all(|c| c.pattern == 0));
        assert_eq!(p.clips.len(), 3);
    }

    #[test]
    fn note_names() {
        assert_eq!(note_name(60), "C5");
        assert_eq!(note_name(69), "A5");
    }
}
