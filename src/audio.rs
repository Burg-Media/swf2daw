//! Sound from the SWF into the DAW.
//!
//! Two routes end up in the plugin output:
//! - Ruffle's own sounds (SWF sounds, MP3, streams): two `AudioMixer`s created at the host
//!   sample rate, one for music and one for sound effects. The engine thread owns the
//!   backend, the audio thread mixes through the proxies, like Ruffle desktop does with cpal.
//! - gb2swf games: they generate their samples themselves and hand them over through
//!   `ExternalInterface.call("gb2swfAudio", samples)` (Ruffle has no SampleDataEvent).
//!   The engine pushes those into a ring buffer, `StreamResampler` plays them at the host
//!   rate and slowly bends the ratio so the buffer neither runs dry nor grows.
//!   A gb2swf game that supports stems sends its four channels apart
//!   (`gb2swfAudioStems`); the ring buffer always carries all `BUSES` buses.

use ruffle_core::FloatDuration;
use ruffle_core::backend::audio::{
    AudioBackend, AudioMixer, AudioMixerProxy, DecodeError, RegisterError, SoundHandle,
    SoundInstanceHandle, SoundStreamInfo, SoundTransform, Substream, swf,
};
use ruffle_core::tag_utils::SwfSlice;
use slotmap::SlotMap;
use std::collections::HashMap;

/// gb2swf always produces 44.1 kHz stereo.
pub const STREAM_RATE: f64 = 44100.0;
/// Stereo buses: Pulse 1, Pulse 2, Wave, Noise (gb2swf stems), Music and SFX (Ruffle's
/// sounds; Music also gets the plain mix of older gb2swf games).
pub const BUSES: usize = 6;
pub const MUSIC_BUS: usize = 4;
pub const SFX_BUS: usize = 5;
/// Values per stream frame.
pub const FRAME: usize = BUSES * 2;
/// Values the gb2swf ring buffer holds (about 1 second).
pub const STREAM_CAPACITY: usize = 44100 * FRAME;

/// Event sounds at least this long count as music.
const MUSIC_SECONDS: f64 = 8.0;

const MUSIC: usize = 0;
const SFX: usize = 1;

/// Ruffle's audio, split in music and effects: every sound is registered in both mixers
/// and plays in one of them. Streams, looping sounds and long sounds are music.
pub struct PluginAudioBackend {
    mixers: [AudioMixer; 2],
    /// A sound's handle in the music mixer (the one Ruffle gets) → its handle in the SFX mixer
    sfx_sounds: HashMap<SoundHandle, SoundHandle>,
    /// Our instance handles → (mixer, the mixer's own handle)
    instances: SlotMap<SoundInstanceHandle, (usize, SoundInstanceHandle)>,
}

impl PluginAudioBackend {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            mixers: [AudioMixer::new(2, sample_rate), AudioMixer::new(2, sample_rate)],
            sfx_sounds: HashMap::new(),
            instances: SlotMap::with_key(),
        }
    }

    /// [music, sfx]
    pub fn proxies(&self) -> [AudioMixerProxy; 2] {
        [self.mixers[MUSIC].proxy(), self.mixers[SFX].proxy()]
    }

    fn add(&mut self, mixer: usize, inner: Result<SoundInstanceHandle, DecodeError>) -> Result<SoundInstanceHandle, DecodeError> {
        Ok(self.instances.insert((mixer, inner?)))
    }

    fn get(&self, instance: SoundInstanceHandle) -> Option<(&AudioMixer, SoundInstanceHandle)> {
        self.instances.get(instance).map(|&(m, h)| (&self.mixers[m], h))
    }
}

impl AudioBackend for PluginAudioBackend {
    // The DAW drives the output, there is no device to start or stop
    fn play(&mut self) {}
    fn pause(&mut self) {}

    fn register_sound(&mut self, swf_sound: &swf::Sound) -> Result<SoundHandle, RegisterError> {
        let handle = self.mixers[MUSIC].register_sound(swf_sound)?;
        let sfx = self.mixers[SFX].register_sound(swf_sound)?;
        self.sfx_sounds.insert(handle, sfx);
        Ok(handle)
    }

    fn register_mp3(&mut self, data: &[u8]) -> Result<SoundHandle, DecodeError> {
        let handle = self.mixers[MUSIC].register_mp3(data)?;
        let sfx = self.mixers[SFX].register_mp3(data)?;
        self.sfx_sounds.insert(handle, sfx);
        Ok(handle)
    }

    fn start_sound(&mut self, sound: SoundHandle, settings: &swf::SoundInfo) -> Result<SoundInstanceHandle, DecodeError> {
        let long = self.mixers[MUSIC]
            .get_sound_duration(sound)
            .is_some_and(|d| d.as_millis() >= MUSIC_SECONDS * 1000.0);
        let sfx = self.sfx_sounds.get(&sound).copied();
        match sfx {
            Some(sfx) if settings.num_loops <= 1 && !long => {
                let inner = self.mixers[SFX].start_sound(sfx, settings);
                self.add(SFX, inner)
            }
            _ => {
                let inner = self.mixers[MUSIC].start_sound(sound, settings);
                self.add(MUSIC, inner)
            }
        }
    }

    fn start_stream(&mut self, clip_data: SwfSlice, handle: &swf::SoundStreamHead) -> Result<SoundInstanceHandle, DecodeError> {
        let inner = self.mixers[MUSIC].start_stream(clip_data, handle);
        self.add(MUSIC, inner)
    }

    fn start_substream(&mut self, stream_data: Substream, stream_info: &SoundStreamInfo) -> Result<SoundInstanceHandle, DecodeError> {
        let inner = self.mixers[MUSIC].start_substream(stream_data, stream_info);
        self.add(MUSIC, inner)
    }

    fn stop_sound(&mut self, sound: SoundInstanceHandle) {
        if let Some((m, h)) = self.instances.remove(sound) {
            self.mixers[m].stop_sound(h);
        }
    }

    fn stop_all_sounds(&mut self) {
        for m in &mut self.mixers {
            m.stop_all_sounds();
        }
        self.instances.clear();
    }

    fn get_sound_position(&self, instance: SoundInstanceHandle) -> Option<f64> {
        self.get(instance).and_then(|(m, h)| m.get_sound_position(h))
    }

    fn get_sound_duration(&self, sound: SoundHandle) -> Option<FloatDuration> {
        self.mixers[MUSIC].get_sound_duration(sound)
    }

    fn get_sound_size(&self, sound: SoundHandle) -> Option<u32> {
        self.mixers[MUSIC].get_sound_size(sound)
    }

    fn get_sound_format(&self, sound: SoundHandle) -> Option<&swf::SoundFormat> {
        self.mixers[MUSIC].get_sound_format(sound)
    }

    fn set_sound_transform(&mut self, instance: SoundInstanceHandle, transform: SoundTransform) {
        if let Some(&(m, h)) = self.instances.get(instance) {
            self.mixers[m].set_sound_transform(h, transform);
        }
    }

    fn get_sound_peak(&mut self, instance: SoundInstanceHandle) -> Option<[f32; 2]> {
        self.get(instance).and_then(|(m, h)| m.get_sound_peak(h))
    }

    /// Forgets the instances that finished on their own (Ruffle doesn't stop those).
    fn tick(&mut self) {
        let mixers = &self.mixers;
        self.instances.retain(|_, &mut (m, h)| mixers[m].get_sound_position(h).is_some());
    }

    fn volume(&self) -> f32 {
        self.mixers[MUSIC].volume()
    }

    fn set_volume(&mut self, volume: f32) {
        for m in &mut self.mixers {
            m.set_volume(volume);
        }
    }

    /// For SoundMixer.computeSpectrum: both mixers together.
    fn get_sample_history(&self) -> [[f32; 2]; 1024] {
        let mut out = self.mixers[MUSIC].get_sample_history();
        let sfx = self.mixers[SFX].get_sample_history();
        for (o, s) in out.iter_mut().zip(sfx) {
            o[0] += s[0];
            o[1] += s[1];
        }
        out
    }
}

/// Plays the 44.1 kHz gb2swf stream at the host rate (4-point Hermite interpolation),
/// all buses with one clock.
pub struct StreamResampler {
    rx: rtrb::Consumer<f32>,
    out_rate: f64,
    /// Position between hist[1] and hist[2], 0..1
    frac: f64,
    /// Last four input frames
    hist: [[f32; FRAME]; 4],
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
            hist: [[0.0; FRAME]; 4],
            priming: true,
        }
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.out_rate = rate as f64;
    }

    fn pop_frame(&mut self) -> Option<[f32; FRAME]> {
        let chunk = self.rx.read_chunk(FRAME).ok()?;
        let mut f = [0.0; FRAME];
        let (a, b) = chunk.as_slices();
        f[..a.len()].copy_from_slice(a);
        f[a.len()..].copy_from_slice(b);
        chunk.commit_all();
        Some(f)
    }

    /// Adds `n` samples of the stream to `out` (one slice per bus channel: bus 0 L, bus 0 R, …).
    pub fn render(&mut self, out: &mut [Vec<f32>; FRAME], n: usize) {
        let fill = (self.rx.slots() / FRAME) as f64;
        if self.priming {
            if fill < Self::TARGET {
                return;
            }
            self.priming = false;
        }
        // Way too much (e.g. after the editor or the DAW hung): skip ahead
        if fill > Self::TARGET * 4.0 {
            let skip = ((fill - Self::TARGET) as usize) * FRAME;
            if let Ok(chunk) = self.rx.read_chunk(skip) {
                chunk.commit_all();
            }
        }
        // Up to 0.5% faster or slower depending on how full the buffer is
        let fill = (self.rx.slots() / FRAME) as f64;
        let bend = ((fill - Self::TARGET) / Self::TARGET * 0.005).clamp(-0.005, 0.005);
        let step = STREAM_RATE / self.out_rate * (1.0 + bend);

        for i in 0..n {
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
                        self.hist = [[0.0; FRAME]; 4];
                        return;
                    }
                }
            }
            let t = self.frac as f32;
            let h = &self.hist;
            for (c, o) in out.iter_mut().enumerate() {
                o[i] += hermite(h[0][c], h[1][c], h[2][c], h[3][c], t);
            }
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
