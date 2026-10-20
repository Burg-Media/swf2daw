//! The sound options: a strip per bus (3-band EQ, level, pan, mute, solo) and the master
//! rack after the mix (10-band EQ, Mega Bass, filter and the effects from the skin mock).
//! Everything runs on the audio thread, allocation-free once `MasterFx::new` is done.

use std::f32::consts::TAU;

/// RBJ cookbook biquad, stereo, transposed direct form II.
#[derive(Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z: [[f32; 2]; 2],
}

pub enum Kind {
    Peak,
    LowShelf,
    HighShelf,
    LowPass,
}

impl Default for Biquad {
    fn default() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, z: [[0.0; 2]; 2] }
    }
}

impl Biquad {
    /// Sets the coefficients, keeps the state (so changes don't click).
    pub fn set(&mut self, kind: Kind, rate: f32, freq: f32, q: f32, db: f32) {
        let w = TAU * (freq / rate).min(0.49);
        let (sin, cos) = w.sin_cos();
        let a = 10f32.powf(db / 40.0);
        let alpha = sin / (2.0 * q);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            Kind::Peak => (1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a),
            Kind::LowShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + s),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - s),
                    (a + 1.0) + (a - 1.0) * cos + s,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - s,
                )
            }
            Kind::HighShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + s),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - s),
                    (a + 1.0) - (a - 1.0) * cos + s,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - s,
                )
            }
            Kind::LowPass => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    #[inline]
    pub fn run(&mut self, ch: usize, x: f32) -> f32 {
        let y = self.b0 * x + self.z[0][ch];
        self.z[0][ch] = self.b1 * x - self.a1 * y + self.z[1][ch];
        self.z[1][ch] = self.b2 * x - self.a2 * y;
        y
    }
}

/// Settings of one bus strip, read from the parameters once per block.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct StripEq {
    pub low: f32,
    pub mid: f32,
    pub high: f32,
}

/// 3-band EQ of one bus. Bypassed while flat.
#[derive(Default)]
pub struct Strip {
    bands: [Biquad; 3],
    eq: Option<StripEq>,
    flat: bool,
}

impl Strip {
    pub fn update(&mut self, rate: f32, eq: StripEq) {
        if self.eq == Some(eq) {
            return;
        }
        self.eq = Some(eq);
        self.flat = eq.low == 0.0 && eq.mid == 0.0 && eq.high == 0.0;
        self.bands[0].set(Kind::LowShelf, rate, 220.0, 0.7, eq.low);
        self.bands[1].set(Kind::Peak, rate, 1200.0, 0.8, eq.mid);
        self.bands[2].set(Kind::HighShelf, rate, 4500.0, 0.7, eq.high);
    }

    #[inline]
    pub fn run(&mut self, ch: usize, x: f32) -> f32 {
        if self.flat {
            return x;
        }
        let x = self.bands[0].run(ch, x);
        let x = self.bands[1].run(ch, x);
        self.bands[2].run(ch, x)
    }
}

pub const EQ_FREQS: [f32; 10] = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

/// The master rack settings for one block.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct Rack {
    pub eq: [f32; 10],
    pub mega: bool,
    pub filter: Option<(f32, f32)>,
    pub crush: Option<f32>,
    pub wobble: Option<f32>,
    pub ring: Option<f32>,
    pub gate: Option<f32>,
    pub echo: Option<f32>,
    pub space: Option<f32>,
}

/// Where the DAW is, for the tempo-synced effects.
#[derive(Clone, Copy)]
pub struct Clock {
    pub bpm: f32,
    /// Position in quarter notes at the start of the block, if the transport runs
    pub beats: Option<f64>,
}

pub struct MasterFx {
    rate: f32,
    rack: Option<Rack>,
    eq: [Biquad; 10],
    eq_flat: bool,
    mega: Biquad,
    filter: Biquad,
    // crush
    hold: [f32; 2],
    hold_count: u32,
    // wobble
    wob_buf: Vec<[f32; 2]>,
    wob_pos: usize,
    wob_phase: f32,
    // ring
    ring_phase: f32,
    // gate
    gate_beats: f64,
    gate_gain: f32,
    // echo
    echo_buf: Vec<[f32; 2]>,
    echo_pos: usize,
    echo_tone: [f32; 2],
    // space
    reverb: Reverb,
}

impl MasterFx {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            rack: None,
            eq: [Biquad::default(); 10],
            eq_flat: true,
            mega: Biquad::default(),
            filter: Biquad::default(),
            hold: [0.0; 2],
            hold_count: 0,
            wob_buf: vec![[0.0; 2]; (rate * 0.05) as usize + 4],
            wob_pos: 0,
            wob_phase: 0.0,
            ring_phase: 0.0,
            gate_beats: 0.0,
            gate_gain: 1.0,
            echo_buf: vec![[0.0; 2]; (rate * 2.5) as usize + 4],
            echo_pos: 0,
            echo_tone: [0.0; 2],
            reverb: Reverb::new(rate),
        }
    }

    fn update(&mut self, rack: Rack) {
        if self.rack == Some(rack) {
            return;
        }
        let rate = self.rate;
        if self.rack.is_none_or(|r| r.eq != rack.eq) {
            for (b, (f, db)) in self.eq.iter_mut().zip(EQ_FREQS.iter().zip(rack.eq)) {
                b.set(Kind::Peak, rate, *f, 1.3, db);
            }
            self.eq_flat = rack.eq.iter().all(|&g| g == 0.0);
        }
        self.mega.set(Kind::LowShelf, rate, 110.0, 0.7, if rack.mega { 12.0 } else { 0.0 });
        if let Some((cut, q)) = rack.filter {
            self.filter.set(Kind::LowPass, rate, cut, q, 0.0);
        }
        if rack.space.is_none() && self.rack.is_some_and(|r| r.space.is_some()) {
            self.reverb.clear();
        }
        self.rack = Some(rack);
    }

    /// Runs the rack over the mix in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], rack: Rack, clock: Clock) {
        self.update(rack);
        let rate = self.rate;
        let n = left.len();
        let beats_per_sample = clock.bpm as f64 / 60.0 / rate as f64;
        if let Some(b) = clock.beats {
            self.gate_beats = b;
        }

        for i in 0..n {
            let mut s = [left[i], right[i]];

            if !self.eq_flat {
                for (ch, x) in s.iter_mut().enumerate() {
                    for b in &mut self.eq {
                        *x = b.run(ch, *x);
                    }
                }
            }
            if rack.mega {
                for (ch, x) in s.iter_mut().enumerate() {
                    *x = self.mega.run(ch, *x);
                }
            }

            // BITCRUSH: fewer bits and sample-and-hold
            if let Some(a) = rack.crush {
                let bits = (12.0 - a * 10.0).round().max(2.0);
                let step = 2f32.powf(bits - 1.0);
                let every = ((1.0 + (a * 24.0).floor()) * rate / 48000.0).max(1.0) as u32;
                if self.hold_count % every == 0 {
                    self.hold = s.map(|x| (x * step).round() / step);
                }
                self.hold_count = self.hold_count.wrapping_add(1);
                s = self.hold;
            }

            // TAPE WOBBLE: a short delay line swept by an LFO
            if let Some(a) = rack.wobble {
                let len = self.wob_buf.len();
                self.wob_buf[self.wob_pos] = s;
                self.wob_phase = (self.wob_phase + (0.6 + a * 7.0) / rate).fract();
                let depth = 0.001 + a * 0.009;
                let delay = (0.02 + depth * (TAU * self.wob_phase).sin()).clamp(0.001, 0.045) * rate;
                let read = self.wob_pos as f32 + len as f32 - delay;
                let (i0, t) = (read.floor() as usize, read.fract());
                let (p, q) = (self.wob_buf[i0 % len], self.wob_buf[(i0 + 1) % len]);
                s = [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
                self.wob_pos = (self.wob_pos + 1) % len;
            }

            // RING MOD
            if let Some(a) = rack.ring {
                self.ring_phase = (self.ring_phase + (30.0 + a * a * 1400.0) / rate).fract();
                let c = (TAU * self.ring_phase).sin();
                s = s.map(|x| x * (0.1 + 0.9 * c));
            }

            // FILTER (the XY pad)
            if rack.filter.is_some() {
                for (ch, x) in s.iter_mut().enumerate() {
                    *x = self.filter.run(ch, *x);
                }
            }

            // TRANCE GATE: every other 1/16 down by the amount, with short ramps
            let beat = self.gate_beats;
            self.gate_beats += beats_per_sample;
            if let Some(a) = rack.gate {
                let target = if ((beat * 4.0).floor() as i64) % 2 == 0 { 1.0 } else { 1.0 - a };
                self.gate_gain += (target - self.gate_gain) * (1.0 - (-1.0 / (0.002 * rate)).exp());
                s = s.map(|x| x * self.gate_gain);
            } else {
                self.gate_gain = 1.0;
            }

            let dry = s;

            // ECHO 3/16, ping-pong, darker every repeat
            if let Some(a) = rack.echo {
                let len = self.echo_buf.len();
                let delay = ((60.0 / clock.bpm.max(20.0)) * 0.75 * rate).min(len as f32 - 2.0) as usize;
                let tap = self.echo_buf[(self.echo_pos + len - delay) % len];
                let fb = 0.2 + a * 0.62;
                for ch in 0..2 {
                    self.echo_tone[ch] += (tap[ch] - self.echo_tone[ch]) * 0.35;
                }
                self.echo_buf[self.echo_pos] = [
                    (dry[0] + dry[1]) * 0.5 + self.echo_tone[1] * fb,
                    self.echo_tone[0] * fb,
                ];
                self.echo_pos = (self.echo_pos + 1) % len;
                let wet = 0.15 + a * 0.5;
                s[0] += tap[0] * wet;
                s[1] += tap[1] * wet;
            }

            // SPACE
            if let Some(a) = rack.space {
                let w = self.reverb.run(dry);
                s[0] += w[0] * a * 0.9;
                s[1] += w[1] * a * 0.9;
            }

            left[i] = s[0];
            right[i] = s[1];
        }
    }
}

/// Freeverb (Jezar's public domain design), tuned for a big, soft room.
struct Reverb {
    combs: [[Comb; 8]; 2],
    allpasses: [[Allpass; 4]; 2],
}

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;

impl Reverb {
    fn new(rate: f32) -> Self {
        let k = rate / 44100.0;
        let sz = |n: usize, ch: usize| (((n + ch * SPREAD) as f32) * k) as usize;
        Self {
            combs: [0, 1].map(|ch| COMBS.map(|n| Comb::new(sz(n, ch)))),
            allpasses: [0, 1].map(|ch| ALLPASSES.map(|n| Allpass::new(sz(n, ch)))),
        }
    }

    fn clear(&mut self) {
        for c in self.combs.iter_mut().flatten() {
            c.buf.fill(0.0);
            c.store = 0.0;
        }
        for a in self.allpasses.iter_mut().flatten() {
            a.buf.fill(0.0);
        }
    }

    fn run(&mut self, x: [f32; 2]) -> [f32; 2] {
        let input = (x[0] + x[1]) * 0.015;
        [0, 1].map(|ch| {
            let mut out = 0.0;
            for c in &mut self.combs[ch] {
                out += c.run(input);
            }
            for a in &mut self.allpasses[ch] {
                out = a.run(out);
            }
            out
        })
    }
}

struct Comb {
    buf: Vec<f32>,
    pos: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], pos: 0, store: 0.0 }
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let y = self.buf[self.pos];
        // feedback 0.86, damping 0.3
        self.store = y * 0.7 + self.store * 0.3;
        self.buf[self.pos] = x + self.store * 0.86;
        self.pos = (self.pos + 1) % self.buf.len();
        y
    }
}

struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], pos: 0 }
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let b = self.buf[self.pos];
        self.buf[self.pos] = x + b * 0.5;
        self.pos = (self.pos + 1) % self.buf.len();
        b - x
    }
}

/// Balance, -1 (left) .. 1 (right), as (left, right) gains. The Game Boy pans in hard
/// left/right steps already, so this only turns the far side down.
#[inline]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let pan = pan.clamp(-1.0, 1.0);
    ((1.0 - pan).min(1.0), (1.0 + pan).min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48000.0;

    /// One second of a 440 Hz tone with some 80 Hz under it.
    fn tone() -> (Vec<f32>, Vec<f32>) {
        let l: Vec<f32> = (0..RATE as usize)
            .map(|i| {
                let t = i as f32 / RATE;
                0.3 * (TAU * 440.0 * t).sin() + 0.3 * (TAU * 80.0 * t).sin()
            })
            .collect();
        (l.clone(), l)
    }

    /// RMS difference between the rack output and the input, over the last half second.
    fn change(rack: Rack) -> f32 {
        let (mut l, mut r) = tone();
        let (dry, _) = tone();
        let mut fx = MasterFx::new(RATE);
        let clock = Clock { bpm: 130.0, beats: Some(0.0) };
        for (cl, cr) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
            fx.process(cl, cr, rack, Clock { beats: None, ..clock });
        }
        let half = l.len() / 2;
        let sum: f32 = l[half..].iter().zip(&dry[half..]).map(|(a, b)| (a - b).powi(2)).sum();
        (sum / half as f32).sqrt()
    }

    #[test]
    fn neutral_rack_is_transparent() {
        assert!(change(Rack::default()) < 1e-6);
    }

    #[test]
    fn every_effect_changes_the_sound() {
        let mut eq = [0.0; 10];
        eq[0] = -12.0;
        eq[1] = -12.0;
        eq[2] = -12.0;
        let cases = [
            ("eq", Rack { eq, ..Rack::default() }),
            ("mega", Rack { mega: true, ..Rack::default() }),
            ("filter", Rack { filter: Some((200.0, 4.0)), ..Rack::default() }),
            ("crush", Rack { crush: Some(0.6), ..Rack::default() }),
            ("wobble", Rack { wobble: Some(0.5), ..Rack::default() }),
            ("ring", Rack { ring: Some(0.4), ..Rack::default() }),
            ("gate", Rack { gate: Some(0.7), ..Rack::default() }),
            ("echo", Rack { echo: Some(0.5), ..Rack::default() }),
            ("space", Rack { space: Some(0.6), ..Rack::default() }),
        ];
        for (name, rack) in cases {
            let c = change(rack);
            println!("{name}: {c:.4}");
            assert!(c > 0.02, "{name} barely changes the sound ({c})");
        }
    }
}
