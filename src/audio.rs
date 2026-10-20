//! Sound from the SWF into the DAW.
//!
//! Two routes end up in the plugin output:
//! - Ruffle's own sounds (SWF sounds, MP3, streams): an `AudioMixer` created at the host
//!   sample rate. The engine thread owns the backend, the audio thread mixes through
//!   the proxy, exactly like Ruffle desktop does with cpal.
//! - gb2swf games: they generate their samples themselves and hand them over through
//!   `ExternalInterface.call("gb2swfAudio", samples)` (Ruffle has no SampleDataEvent).
//!   The engine pushes those into a ring buffer, `StreamResampler` plays them at the host
//!   rate and slowly bends the ratio so the buffer neither runs dry nor grows.

use ruffle_core::backend::audio::{
    AudioBackend, AudioMixer, DecodeError, RegisterError, SoundHandle, SoundInstanceHandle,
    SoundStreamInfo, SoundTransform, swf,
};
use ruffle_core::impl_audio_mixer_backend;

/// gb2swf always produces 44.1 kHz stereo.
pub const STREAM_RATE: f64 = 44100.0;
/// Interleaved stereo samples the gb2swf ring buffer holds (about 1 second).
pub const STREAM_CAPACITY: usize = 44100 * 2;

pub struct PluginAudioBackend {
    mixer: AudioMixer,
}

impl PluginAudioBackend {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            mixer: AudioMixer::new(2, sample_rate),
        }
    }

    pub fn proxy(&self) -> ruffle_core::backend::audio::AudioMixerProxy {
        self.mixer.proxy()
    }
}

impl AudioBackend for PluginAudioBackend {
    impl_audio_mixer_backend!(mixer);

    // The DAW drives the output, there is no device to start or stop
    fn play(&mut self) {}
    fn pause(&mut self) {}
}

/// Plays the 44.1 kHz gb2swf stream at the host rate (4-point Hermite interpolation).
pub struct StreamResampler {
    rx: rtrb::Consumer<f32>,
    out_rate: f64,
    /// Position between hist[1] and hist[2], 0..1
    frac: f64,
    /// Last four input frames
    hist: [[f32; 2]; 4],
    /// Waiting for enough samples before (re)starting
    priming: bool,
}

impl StreamResampler {
    /// Buffer fill (in frames) we aim for: ~60 ms, enough for a late game frame.
    const TARGET: f64 = STREAM_RATE * 0.06;

    pub fn new(rx: rtrb::Consumer<f32>) -> Self {
        Self {
            rx,
            out_rate: 48000.0,
            frac: 0.0,
            hist: [[0.0; 2]; 4],
            priming: true,
        }
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.out_rate = rate as f64;
    }

    fn pop_frame(&mut self) -> Option<[f32; 2]> {
        if self.rx.slots() < 2 {
            return None;
        }
        let l = self.rx.pop().ok()?;
        let r = self.rx.pop().ok()?;
        Some([l, r])
    }

    /// Adds the stream to `left`/`right`.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let fill = (self.rx.slots() / 2) as f64;
        if self.priming {
            if fill < Self::TARGET {
                return;
            }
            self.priming = false;
        }
        // Way too much (e.g. after the editor or the DAW hung): skip ahead
        if fill > Self::TARGET * 4.0 {
            let skip = ((fill - Self::TARGET) as usize) * 2;
            if let Ok(chunk) = self.rx.read_chunk(skip) {
                chunk.commit_all();
            }
        }
        // Up to 0.5% faster or slower depending on how full the buffer is
        let fill = (self.rx.slots() / 2) as f64;
        let bend = ((fill - Self::TARGET) / Self::TARGET * 0.005).clamp(-0.005, 0.005);
        let step = STREAM_RATE / self.out_rate * (1.0 + bend);

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.frac += step;
            while self.frac >= 1.0 {
                self.frac -= 1.0;
                match self.pop_frame() {
                    Some(f) => {
                        self.hist.rotate_left(1);
                        self.hist[3] = f;
                    }
                    None => {
                        // Ran dry: fade to silence and wait for the buffer to fill up again
                        self.priming = true;
                        self.hist = [[0.0; 2]; 4];
                        return;
                    }
                }
            }
            let t = self.frac as f32;
            *l += hermite(self.hist[0][0], self.hist[1][0], self.hist[2][0], self.hist[3][0], t);
            *r += hermite(self.hist[0][1], self.hist[1][1], self.hist[2][1], self.hist[3][1], t);
        }
    }
}

fn hermite(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c0 = y1;
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + c0
}
