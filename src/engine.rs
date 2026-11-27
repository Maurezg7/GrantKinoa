use crate::dsp::{self, Delay, Reverb, Rng, Svf};
use crate::project::{DEFAULT_KEY, InstrumentKind, MIXER_INSERTS, Project};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

pub const BLOCK: usize = 64;
const MAX_VOICES: usize = 128;

pub struct SampleData {
    pub sr: f32,
    pub frames: Vec<[f32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    Pattern,
    Song,
}

pub enum Cmd {
    Project(Arc<Project>),
    Sample(u32, Option<Arc<SampleData>>),
    Play,
    Stop,
    Mode(PlayMode),
    CurrentPattern(usize),
    Seek(f64),
    NoteOn { ch: u32, key: u8, vel: f32 },
    NoteOff { ch: u32, key: u8 },
}

/// State the audio thread publishes for the UI.
pub struct Shared {
    pub playing: AtomicBool,
    pos: AtomicU64,
    meters: Vec<AtomicU32>,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            playing: AtomicBool::new(false),
            pos: AtomicU64::new(0),
            meters: (0..(MIXER_INSERTS + 1) * 2)
                .map(|_| AtomicU32::new(0))
                .collect(),
        }
    }
}

impl Shared {
    pub fn pos(&self) -> f64 {
        f64::from_bits(self.pos.load(Ordering::Relaxed))
    }

    fn set_pos(&self, p: f64) {
        self.pos.store(p.to_bits(), Ordering::Relaxed);
    }

    fn push_meter(&self, track: usize, peak: [f32; 2]) {
        for (i, v) in peak.iter().enumerate() {
            self.meters[track * 2 + i].fetch_max(v.min(4.0).to_bits(), Ordering::Relaxed);
        }
    }

    /// Peak since the last call, per side.
    pub fn take_meter(&self, track: usize) -> [f32; 2] {
        [0, 1].map(|i| f32::from_bits(self.meters[track * 2 + i].swap(0, Ordering::Relaxed)))
    }
}

struct Voice {
    ch: u32,
    kind: InstrumentKind,
    key: u8,
    vel: f32,
    t: u32,
    gate: Option<u32>,
    live: bool,
    released: bool,
    rel_t: u32,
    rel_level: f32,
    env: f32,
    phases: [f32; 3],
    filt: Svf,
    rng: Rng,
    spos: f64,
    done: bool,
}

struct Insert {
    lp: [Svf; 2],
    delay: Delay,
    reverb: Reverb,
}

pub struct Engine {
    sr: f32,
    project: Arc<Project>,
    samples: HashMap<u32, Arc<SampleData>>,
    voices: Vec<Voice>,
    inserts: Vec<Insert>,
    buses: Vec<[f32; 2]>,
    pos: f64,
    playing: bool,
    looping: bool,
    mode: PlayMode,
    pattern: usize,
    shared: Arc<Shared>,
    seed: u32,
}

impl Engine {
    pub fn new(sr: f32, project: Arc<Project>, shared: Arc<Shared>) -> Self {
        Self {
            sr,
            project,
            samples: HashMap::new(),
            voices: Vec::with_capacity(MAX_VOICES),
            inserts: (0..=MIXER_INSERTS)
                .map(|_| Insert {
                    lp: [Svf::default(); 2],
                    delay: Delay::new(sr),
                    reverb: Reverb::new(sr),
                })
                .collect(),
            buses: vec![[0.0; 2]; (MIXER_INSERTS + 1) * BLOCK],
            pos: 0.0,
            playing: false,
            looping: true,
            mode: PlayMode::Pattern,
            pattern: 0,
            shared,
            seed: 0x9e37_79b9,
        }
    }

    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    pub fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Project(p) => {
                self.project = p;
                self.pattern = self
                    .pattern
                    .min(self.project.patterns.len().saturating_sub(1));
            }
            Cmd::Sample(ch, data) => match data {
                Some(d) => {
                    self.samples.insert(ch, d);
                }
                None => {
                    self.samples.remove(&ch);
                }
            },
            Cmd::Play => self.playing = true,
            Cmd::Stop => {
                self.playing = false;
                self.pos = 0.0;
                for v in &mut self.voices {
                    if !v.live {
                        v.gate = Some(0);
                    }
                }
            }
            Cmd::Mode(m) => {
                if m != self.mode {
                    self.mode = m;
                    self.pos = 0.0;
                }
            }
            Cmd::CurrentPattern(p) => self.pattern = p,
            Cmd::Seek(p) => self.pos = p.max(0.0),
            Cmd::NoteOn { ch, key, vel } => self.trigger(ch, key, vel, None),
            Cmd::NoteOff { ch, key } => {
                for v in &mut self.voices {
                    if v.live && v.ch == ch && v.key == key {
                        v.gate = Some(0);
                        v.live = false;
                    }
                }
            }
        }
        self.shared.playing.store(self.playing, Ordering::Relaxed);
        self.shared.set_pos(self.pos);
    }

    fn steps_per_sample(&self) -> f64 {
        self.project.bpm as f64 / 60.0 * 4.0 / self.sr as f64
    }

    fn loop_len(&self) -> f64 {
        match self.mode {
            PlayMode::Pattern => self
                .project
                .patterns
                .get(self.pattern)
                .map(|p| p.length)
                .unwrap_or(16) as f64,
            PlayMode::Song => self.project.song_length().max(16) as f64,
        }
    }

    fn trigger(&mut self, ch: u32, key: u8, vel: f32, len_steps: Option<f64>) {
        let Some(channel) = self.project.channel(ch) else {
            return;
        };
        let kind = channel.instrument.kind;
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let gate = len_steps.map(|l| (l / self.steps_per_sample()) as u32);
        let mut filt = Svf::default();
        let r = 2f32.powf((channel.instrument.drum.tune + key as f32 - DEFAULT_KEY as f32) / 12.0);
        match kind {
            InstrumentKind::Snare => filt.set(1800.0 * r, 0.1, self.sr),
            InstrumentKind::HiHat | InstrumentKind::OpenHat => filt.set(
                (6500.0 + 3000.0 * channel.instrument.drum.tone) * r,
                0.2,
                self.sr,
            ),
            InstrumentKind::Clap => filt.set(1100.0 * r, 0.45, self.sr),
            _ => {}
        }
        self.voices.push(Voice {
            ch,
            kind,
            key,
            vel,
            t: 0,
            gate,
            live: len_steps.is_none(),
            released: false,
            rel_t: 0,
            rel_level: 0.0,
            env: 0.0,
            phases: [0.0; 3],
            filt,
            rng: Rng::new(self.seed),
            spos: 0.0,
            done: false,
        });
    }

    fn sequence(&mut self, frames: usize) {
        let sps = self.steps_per_sample();
        let from = self.pos;
        let to = from + frames as f64 * sps;
        let len = self.loop_len();
        if to >= len {
            self.collect(from, len);
            if self.looping {
                self.collect(0.0, to - len);
                self.pos = to - len;
            } else {
                self.playing = false;
                self.pos = len;
            }
        } else {
            self.collect(from, to);
            self.pos = to;
        }
    }

    fn collect(&mut self, from: f64, to: f64) {
        let project = self.project.clone();
        match self.mode {
            PlayMode::Pattern => self.collect_pattern(&project, self.pattern, from, to),
            PlayMode::Song => {
                for clip in &project.clips {
                    let Some(p) = project.patterns.get(clip.pattern) else {
                        continue;
                    };
                    let s = clip.start as f64;
                    let a = (from - s).max(0.0);
                    let b = (to - s).min(p.length as f64);
                    if a < b {
                        self.collect_pattern(&project, clip.pattern, a, b);
                    }
                }
            }
        }
    }

    fn collect_pattern(&mut self, project: &Project, idx: usize, from: f64, to: f64) {
        let Some(p) = project.patterns.get(idx) else {
            return;
        };
        let len = p.length as f64;
        for (&ch, cp) in &p.channels {
            let mut i = from.ceil() as usize;
            while (i as f64) < to && (i as f64) < len {
                if cp.steps.get(i).copied().unwrap_or(false) {
                    self.trigger(ch, DEFAULT_KEY, 0.8, Some(1.0));
                }
                i += 1;
            }
            for n in &cp.notes {
                let s = n.start as f64;
                if s >= from && s < to && s < len {
                    self.trigger(ch, n.key, n.vel, Some(n.len as f64));
                }
            }
        }
    }

    /// Renders interleaved stereo frames.
    pub fn render(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(BLOCK * 2) {
            let n = chunk.len() / 2;
            if self.playing {
                self.sequence(n);
            }
            self.render_block(chunk, n);
        }
        self.shared.playing.store(self.playing, Ordering::Relaxed);
        self.shared.set_pos(self.pos);
    }

    fn render_block(&mut self, out: &mut [f32], n: usize) {
        self.buses.fill([0.0; 2]);
        let project = self.project.clone();
        let sr = self.sr;
        let sps = self.steps_per_sample();

        for v in &mut self.voices {
            let Some(ch) = project.channel(v.ch) else {
                v.done = true;
                continue;
            };
            let gains = if ch.mute {
                [0.0; 2]
            } else {
                dsp::pan_gains(ch.volume, ch.pan)
            };
            let bus = ch.mixer.min(MIXER_INSERTS) * BLOCK;
            let sample = self.samples.get(&v.ch).cloned();
            for i in 0..n {
                let s = render_voice(v, &ch.instrument, sample.as_deref(), sr) * v.vel;
                let b = &mut self.buses[bus + i];
                b[0] += s * gains[0];
                b[1] += s * gains[1];
                if v.done {
                    break;
                }
            }
        }
        self.voices.retain(|v| !v.done);

        for track in (0..=MIXER_INSERTS).rev() {
            let mt = &project.mixer[track];
            let fx = &mt.fx;
            let ins = &mut self.inserts[track];
            if fx.filter_on {
                for f in &mut ins.lp {
                    f.set(fx.cutoff, fx.resonance, sr);
                }
            }
            let delay_samples = (fx.delay_steps as f64 / sps) as usize;
            let gains = if mt.mute {
                [0.0; 2]
            } else {
                dsp::pan_gains(mt.volume, mt.pan)
            };
            let mut peak = [0.0f32; 2];
            for i in 0..n {
                let mut x = self.buses[track * BLOCK + i];
                if fx.filter_on {
                    x = [ins.lp[0].process(x[0]).low, ins.lp[1].process(x[1]).low];
                }
                if fx.delay_on {
                    let w = ins
                        .delay
                        .process(x, delay_samples, fx.delay_feedback.min(0.95));
                    x = [x[0] + w[0] * fx.delay_mix, x[1] + w[1] * fx.delay_mix];
                }
                if fx.reverb_on {
                    let w = ins.reverb.process(x, fx.reverb_size, fx.reverb_damp);
                    x = [
                        x[0] + w[0] * fx.reverb_mix * 3.0,
                        x[1] + w[1] * fx.reverb_mix * 3.0,
                    ];
                }
                x = [x[0] * gains[0], x[1] * gains[1]];
                if track == 0 {
                    x = [soft_clip(x[0]), soft_clip(x[1])];
                    out[i * 2] = x[0];
                    out[i * 2 + 1] = x[1];
                } else {
                    let m = &mut self.buses[i];
                    m[0] += x[0];
                    m[1] += x[1];
                }
                peak[0] = peak[0].max(x[0].abs());
                peak[1] = peak[1].max(x[1].abs());
            }
            self.shared.push_meter(track, peak);
        }
    }
}

fn soft_clip(x: f32) -> f32 {
    if x.abs() < 0.8 {
        x
    } else {
        x.signum() * (0.8 + 0.2 * ((x.abs() - 0.8) / 0.2).tanh())
    }
}

fn render_voice(
    v: &mut Voice,
    inst: &crate::project::Instrument,
    sample: Option<&SampleData>,
    sr: f32,
) -> f32 {
    let t = v.t as f32 / sr;
    v.t += 1;
    if let Some(g) = v.gate.as_mut() {
        if *g == 0 {
            if !v.released {
                v.released = true;
                v.rel_level = v.env;
            }
        } else {
            *g -= 1;
        }
    }
    let d = inst.drum.decay.max(0.05);
    let tone = inst.drum.tone;
    let r = 2f32.powf((inst.drum.tune + v.key as f32 - DEFAULT_KEY as f32) / 12.0);
    match v.kind {
        InstrumentKind::Kick => {
            let f = r * (48.0 + (90.0 + 220.0 * tone) * (-t / 0.035).exp());
            v.phases[0] = (v.phases[0] + f / sr).fract();
            let amp = (-t / (0.3 * d)).exp();
            let click = v.rng.next() * (-t / 0.003).exp() * 0.3 * tone;
            if t > 2.5 * d {
                v.done = true;
            }
            ((v.phases[0] * std::f32::consts::TAU).sin() * amp * 1.5 + click).tanh() * 0.9
        }
        InstrumentKind::Snare => {
            v.phases[0] = (v.phases[0] + 185.0 * r / sr).fract();
            let body = (v.phases[0] * std::f32::consts::TAU).sin() * (-t / 0.05).exp();
            let noise = v.filt.process(v.rng.next()).high * (-t / (0.13 * d)).exp();
            if t > 1.2 * d {
                v.done = true;
            }
            body * (1.0 - tone) * 0.8 + noise * (0.4 + tone * 0.6)
        }
        InstrumentKind::HiHat | InstrumentKind::OpenHat => {
            let tau = if v.kind == InstrumentKind::HiHat {
                0.04
            } else {
                0.25
            };
            let n = v.filt.process(v.rng.next()).high * (-t / (tau * d)).exp();
            if t > tau * d * 10.0 {
                v.done = true;
            }
            n * 0.55
        }
        InstrumentKind::Clap => {
            let mut env = 0.0;
            for off in [0.0, 0.011, 0.022] {
                if t >= off {
                    env += (-(t - off) / 0.006).exp();
                }
            }
            if t >= 0.022 {
                env += 0.7 * (-(t - 0.022) / (0.12 * d)).exp();
            }
            if t > 1.0 * d + 0.03 {
                v.done = true;
            }
            v.filt.process(v.rng.next()).band * env * 1.6
        }
        InstrumentKind::Synth => {
            let p = &inst.synth;
            let env = adsr(v, t, sr, p.attack, p.decay, p.sustain, p.release);
            let mut s = 0.0;
            for (i, o) in p.osc.iter().enumerate() {
                if o.level <= 0.0 {
                    continue;
                }
                let f = dsp::key_to_freq(v.key as f32 + o.octave as f32 * 12.0 + o.fine / 100.0);
                let dt = (f / sr).min(0.5);
                s += dsp::osc(o.wave, v.phases[i], dt, &mut v.rng) * o.level;
                v.phases[i] = (v.phases[i] + dt).fract();
            }
            if v.t % 16 == 1 {
                let c = p.cutoff * 2f32.powf(p.env_amount * env * 5.0);
                v.filt.set(c, p.resonance, sr);
            }
            v.filt.process(s).low * env * 0.3
        }
        InstrumentKind::Sampler => {
            let Some(smp) = sample.filter(|s| !s.frames.is_empty()) else {
                v.done = true;
                return 0.0;
            };
            let p = &inst.sampler;
            let env = adsr(v, t, sr, p.attack, 0.0, 1.0, p.release);
            let rate = 2f64.powf((v.key as f64 - p.root as f64) / 12.0) * smp.sr as f64 / sr as f64;
            let i = v.spos as usize;
            if i + 1 >= smp.frames.len() {
                v.done = true;
                return 0.0;
            }
            let frac = (v.spos - i as f64) as f32;
            let a = smp.frames[i];
            let b = smp.frames[i + 1];
            v.spos += rate;
            let l = a[0] + (b[0] - a[0]) * frac;
            let rr = a[1] + (b[1] - a[1]) * frac;
            (l + rr) * 0.5 * env
        }
    }
}

fn adsr(v: &mut Voice, t: f32, sr: f32, a: f32, d: f32, s: f32, r: f32) -> f32 {
    let env = if v.released {
        let rt = v.rel_t as f32 / (r.max(0.001) * sr);
        v.rel_t += 1;
        let e = v.rel_level * (1.0 - rt);
        if e <= 0.0 {
            v.done = true;
        }
        e.max(0.0)
    } else if t < a {
        t / a.max(0.0001)
    } else if t < a + d {
        1.0 - (1.0 - s) * (t - a) / d.max(0.0001)
    } else {
        s
    };
    if !v.released && s <= 0.0 && t >= a + d {
        v.done = true;
    }
    v.env = env;
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(buf: &[f32]) -> f32 {
        (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt()
    }

    #[test]
    fn demo_song_is_audible_and_finite() {
        let shared = Arc::new(Shared::default());
        let mut e = Engine::new(44100.0, Arc::new(Project::demo()), shared.clone());
        e.handle(Cmd::Mode(PlayMode::Song));
        e.handle(Cmd::Play);
        let mut buf = vec![0.0; 44100 * 2 * 4];
        e.render(&mut buf);
        assert!(buf.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        assert!(rms(&buf) > 0.02, "rms {}", rms(&buf));
        assert!(shared.pos() > 0.0);
    }

    #[test]
    fn pattern_loops() {
        let shared = Arc::new(Shared::default());
        let mut e = Engine::new(48000.0, Arc::new(Project::demo()), shared.clone());
        e.handle(Cmd::Play);
        // 124 bpm: one 16-step bar is ~1.94 s; render 3 s.
        let mut buf = vec![0.0; 48000 * 2 * 3];
        e.render(&mut buf);
        assert!(shared.pos() < 16.0);
        assert!(e.playing);
    }

    #[test]
    fn stop_silences() {
        let shared = Arc::new(Shared::default());
        let mut e = Engine::new(44100.0, Arc::new(Project::demo()), shared);
        e.handle(Cmd::Play);
        let mut buf = vec![0.0; 44100];
        e.render(&mut buf);
        e.handle(Cmd::Stop);
        let mut buf = vec![0.0; 44100 * 2 * 3];
        e.render(&mut buf);
        assert!(rms(&buf[buf.len() - 4410..]) < 1e-3);
    }
}
