use std::f32::consts::PI;

/// Topology-preserving state variable filter (Simper).
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

pub struct SvfOut {
    pub low: f32,
    pub band: f32,
    pub high: f32,
}

impl Svf {
    pub fn set(&mut self, cutoff: f32, resonance: f32, sr: f32) {
        let fc = cutoff.clamp(20.0, sr * 0.45);
        let g = (PI * fc / sr).tan();
        self.k = 2.0 - 1.95 * resonance.clamp(0.0, 1.0);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    pub fn process(&mut self, v0: f32) -> SvfOut {
        let v3 = v0 - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        if !self.ic1.is_finite() || !self.ic2.is_finite() {
            self.ic1 = 0.0;
            self.ic2 = 0.0;
        }
        SvfOut {
            low: v2,
            band: v1,
            high: v0 - self.k * v1 - v2,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

pub fn osc(wave: crate::project::Wave, phase: f32, dt: f32, rng: &mut Rng) -> f32 {
    use crate::project::Wave;
    match wave {
        Wave::Sine => (phase * 2.0 * PI).sin(),
        Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        Wave::Square => {
            let mut v = if phase < 0.5 { 1.0 } else { -1.0 };
            v += poly_blep(phase, dt);
            v -= poly_blep((phase + 0.5).fract(), dt);
            v
        }
        Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        Wave::Noise => rng.next(),
    }
}

pub fn key_to_freq(key: f32) -> f32 {
    440.0 * 2f32.powf((key - 69.0) / 12.0)
}

pub struct Delay {
    buf: [Vec<f32>; 2],
    pos: usize,
}

impl Delay {
    pub fn new(sr: f32) -> Self {
        let len = (sr * 4.0) as usize;
        Self {
            buf: [vec![0.0; len], vec![0.0; len]],
            pos: 0,
        }
    }

    pub fn process(&mut self, input: [f32; 2], delay_samples: usize, feedback: f32) -> [f32; 2] {
        let len = self.buf[0].len();
        let d = delay_samples.clamp(1, len - 1);
        let read = (self.pos + len - d) % len;
        let out = [self.buf[0][read], self.buf[1][read]];
        // Ping-pong: each side feeds the other.
        self.buf[0][self.pos] = input[0] + out[1] * feedback;
        self.buf[1][self.pos] = input[1] + out[0] * feedback;
        self.pos = (self.pos + 1) % len;
        out
    }
}

struct Comb {
    buf: Vec<f32>,
    pos: usize,
    store: f32,
}

impl Comb {
    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.buf[self.pos];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.buf[self.pos] = input + self.store * feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    fn process(&mut self, input: f32) -> f32 {
        let b = self.buf[self.pos];
        let out = b - input;
        self.buf[self.pos] = input + b * 0.5;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

/// Freeverb-style stereo reverb.
pub struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
}

impl Reverb {
    pub fn new(sr: f32) -> Self {
        const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        const ALLPASS: [usize; 4] = [556, 441, 341, 225];
        let scale = sr / 44100.0;
        let mk = |spread: usize| {
            (
                COMBS
                    .iter()
                    .map(|&l| Comb {
                        buf: vec![0.0; ((l + spread) as f32 * scale) as usize],
                        pos: 0,
                        store: 0.0,
                    })
                    .collect::<Vec<_>>(),
                ALLPASS
                    .iter()
                    .map(|&l| Allpass {
                        buf: vec![0.0; ((l + spread) as f32 * scale) as usize],
                        pos: 0,
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let (cl, al) = mk(0);
        let (cr, ar) = mk(23);
        Self {
            combs: [cl, cr],
            allpasses: [al, ar],
        }
    }

    pub fn process(&mut self, input: [f32; 2], size: f32, damp: f32) -> [f32; 2] {
        let feedback = 0.7 + 0.28 * size.clamp(0.0, 1.0);
        let damp = damp.clamp(0.0, 1.0) * 0.4;
        let mono = (input[0] + input[1]) * 0.015;
        let mut out = [0.0; 2];
        for (ch, o) in out.iter_mut().enumerate() {
            let mut acc = 0.0;
            for c in &mut self.combs[ch] {
                acc += c.process(mono, feedback, damp);
            }
            for a in &mut self.allpasses[ch] {
                acc = a.process(acc);
            }
            *o = acc;
        }
        out
    }
}

pub fn pan_gains(volume: f32, pan: f32) -> [f32; 2] {
    let pan = pan.clamp(-1.0, 1.0);
    [volume * (1.0 - pan).min(1.0), volume * (1.0 + pan).min(1.0)]
}
