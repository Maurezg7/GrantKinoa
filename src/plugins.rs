//! Efectos de inserción: EQ, compresor, saturación, bitcrusher, chorus, phaser y limitador.

use std::f32::consts::{PI, TAU};

fn db(x: f32) -> f32 {
    10f32.powf(x / 20.0)
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}

impl Biquad {
    fn set(&mut self, kind: u8, f: f32, gain_db: f32, sr: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w = TAU * f.min(sr * 0.45) / sr;
        let (s, c) = w.sin_cos();
        let (b, a2) = match kind {
            0 | 2 => {
                let al = s / 2.0 * 2f32.sqrt();
                let sa = 2.0 * a.sqrt() * al;
                let sg = if kind == 0 { -1.0 } else { 1.0 };
                let (p, m) = (a + 1.0, a - 1.0);
                (
                    [
                        a * (p + sg * m * c + sa),
                        -2.0 * sg * a * (m + sg * p * c),
                        a * (p + sg * m * c - sa),
                    ],
                    [
                        p - sg * m * c + sa,
                        2.0 * sg * (m - sg * p * c),
                        p - sg * m * c - sa,
                    ],
                )
            }
            _ => {
                let al = s / (2.0 * 0.9);
                (
                    [1.0 + al * a, -2.0 * c, 1.0 - al * a],
                    [1.0 + al / a, -2.0 * c, 1.0 - al / a],
                )
            }
        };
        let n = a2[0];
        self.b = [b[0] / n, b[1] / n, b[2] / n];
        self.a = [a2[1] / n, a2[2] / n];
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// EQ de 3 bandas: shelf grave (120 Hz), campana media (1 kHz) y shelf agudo (8 kHz).
#[derive(Default)]
pub struct Eq3 {
    f: [[Biquad; 3]; 2],
    key: Option<[f32; 3]>,
}

impl Eq3 {
    pub fn process(&mut self, x: [f32; 2], g: [f32; 3], sr: f32) -> [f32; 2] {
        if self.key != Some(g) {
            self.key = Some(g);
            for ch in &mut self.f {
                ch[0].set(0, 120.0, g[0], sr);
                ch[1].set(1, 1000.0, g[1], sr);
                ch[2].set(2, 8000.0, g[2], sr);
            }
        }
        [0, 1].map(|c| self.f[c].iter_mut().fold(x[c], |s, b| b.process(s)))
    }
}

#[derive(Default)]
pub struct Follower(f32);

impl Follower {
    pub fn process(&mut self, level: f32, att_ms: f32, rel_ms: f32, sr: f32) -> f32 {
        let ms = if level > self.0 { att_ms } else { rel_ms };
        let k = (-1.0 / (ms.max(0.1) * 0.001 * sr)).exp();
        self.0 = level + (self.0 - level) * k;
        self.0
    }
}

/// Compresor estéreo con detección de pico.
#[derive(Default)]
pub struct Compressor(Follower);

impl Compressor {
    #[allow(clippy::too_many_arguments)]
    pub fn process(
        &mut self,
        x: [f32; 2],
        thresh_db: f32,
        ratio: f32,
        att: f32,
        rel: f32,
        makeup_db: f32,
        sr: f32,
    ) -> [f32; 2] {
        let lvl = self.0.process(x[0].abs().max(x[1].abs()), att, rel, sr);
        let l_db = 20.0 * lvl.max(1e-6).log10();
        let over = (l_db - thresh_db).max(0.0);
        let g = db(-over * (1.0 - 1.0 / ratio.max(1.0)) + makeup_db);
        [x[0] * g, x[1] * g]
    }
}

pub fn saturate(x: f32, drive: f32) -> f32 {
    let k = 1.0 + drive * 9.0;
    (x * k).tanh() / k.sqrt()
}

#[derive(Default)]
pub struct Crusher {
    hold: [f32; 2],
    n: u32,
}

impl Crusher {
    pub fn process(&mut self, x: [f32; 2], bits: f32, down: f32) -> [f32; 2] {
        if self.n == 0 {
            let q = 2f32.powf(bits.clamp(2.0, 16.0) - 1.0);
            self.hold = x.map(|v| (v * q).round() / q);
        }
        self.n = (self.n + 1) % (down.max(1.0) as u32);
        self.hold
    }
}

pub struct Chorus {
    buf: Vec<f32>,
    idx: usize,
    phase: f32,
}

impl Chorus {
    pub fn new(sr: f32) -> Self {
        Self {
            buf: vec![0.0; (sr * 0.05) as usize],
            idx: 0,
            phase: 0.0,
        }
    }

    fn tap(&self, d: f32) -> f32 {
        let n = self.buf.len();
        let p = self.idx as f32 + n as f32 - d;
        let i = p as usize;
        let f = p.fract();
        let a = self.buf[i % n];
        let b = self.buf[(i + 1) % n];
        a + (b - a) * f
    }

    pub fn process(&mut self, x: [f32; 2], rate: f32, depth: f32, mix: f32, sr: f32) -> [f32; 2] {
        let n = self.buf.len();
        self.buf[self.idx] = (x[0] + x[1]) * 0.5;
        self.phase = (self.phase + rate / sr).fract();
        let base = 0.012 * sr;
        let m = depth * 0.006 * sr;
        let l = self.tap(base + m * (self.phase * TAU).sin());
        let r = self.tap(base + m * (self.phase * TAU + PI * 0.5).sin());
        self.idx = (self.idx + 1) % n;
        [x[0] + (l - x[0]) * mix * 0.7, x[1] + (r - x[1]) * mix * 0.7]
    }
}

/// Phaser de 6 etapas pasa-todo.
#[derive(Default)]
pub struct Phaser {
    z: [[f32; 6]; 2],
    fb: [f32; 2],
    phase: f32,
}

impl Phaser {
    pub fn process(&mut self, x: [f32; 2], rate: f32, depth: f32, mix: f32, sr: f32) -> [f32; 2] {
        self.phase = (self.phase + rate / sr).fract();
        let mut out = [0.0; 2];
        for c in 0..2 {
            let lfo = 0.5 + 0.5 * (self.phase * TAU + c as f32 * PI * 0.5).sin();
            let f = 200.0 * (1.0 + lfo * depth * 15.0);
            let t = (PI * f / sr).tan();
            let a = (t - 1.0) / (t + 1.0);
            let mut s = x[c] + self.fb[c] * 0.6;
            for z in &mut self.z[c] {
                let y = a * s + *z;
                *z = s - a * y;
                s = y;
            }
            self.fb[c] = s;
            out[c] = x[c] + (s - x[c]) * mix * 0.5 + s * mix * 0.5;
        }
        out
    }
}

/// Limitador brickwall con release suave.
pub struct Limiter(f32);

impl Default for Limiter {
    fn default() -> Self {
        Self(1.0)
    }
}

impl Limiter {
    pub fn process(&mut self, x: [f32; 2], ceiling_db: f32, sr: f32) -> [f32; 2] {
        let c = db(ceiling_db);
        let peak = x[0].abs().max(x[1].abs());
        let target = if peak > c { c / peak } else { 1.0 };
        if target < self.0 {
            self.0 = target;
        } else {
            self.0 += (target - self.0) * (1.0 - (-1.0 / (0.08 * sr)).exp());
        }
        x.map(|v| (v * self.0).clamp(-c, c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_respects_ceiling() {
        let mut l = Limiter::default();
        for i in 0..1000 {
            let v = (i as f32 * 0.1).sin() * 3.0;
            let y = l.process([v, -v], -1.0, 44100.0);
            assert!(y[0].abs() <= db(-1.0) + 1e-6);
        }
    }

    #[test]
    fn eq_flat_is_transparent() {
        let mut e = Eq3::default();
        let mut last = [0.0; 2];
        for _ in 0..2000 {
            last = e.process([0.5, 0.5], [0.0; 3], 44100.0);
        }
        assert!((last[0] - 0.5).abs() < 1e-3);
    }

    #[test]
    fn eq_low_shelf_boosts_dc() {
        let mut e = Eq3::default();
        let mut last = [0.0; 2];
        for _ in 0..20000 {
            last = e.process([0.25, 0.25], [6.0, 0.0, 0.0], 44100.0);
        }
        assert!((last[0] / 0.25 - db(6.0)).abs() < 0.05, "{}", last[0] / 0.25);
    }
}
