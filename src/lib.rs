//! swf2DAW: a Flash player as an instrument plugin. The SWF's sound comes out of the
//! plugin, MIDI notes and CCs play the game.

mod audio;
mod dsp;
mod engine;
mod mapping;
mod params;
mod ui;

use audio::{BUSES, FRAME, MUSIC_BUS, SFX_BUS, StreamResampler};
use dsp::{Clock, MasterFx, Strip};
use engine::{Cmd, Engine, Midi, Saves, Shared};
use mapping::Mapping;
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use params::{BUS_NAMES, RackParams, StripParams};
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};

/// Samples kept for the spectrum analyzer.
pub const SCOPE_LEN: usize = 1024;

#[derive(Params)]
pub struct SwfParams {
    /// Renamed when the editor size changes, so projects saved with the old size open at the new one
    #[persist = "editor-state-1000x770"]
    editor_state: Arc<EguiState>,
    /// Path of the loaded SWF, so the project opens with the same game
    #[persist = "swf-path"]
    swf_path: Arc<RwLock<String>>,
    #[persist = "mapping"]
    mapping: Arc<RwLock<Mapping>>,
    /// Game saves (SharedObjects), base64
    #[persist = "saves"]
    saves: Saves,

    #[id = "volume"]
    pub volume: FloatParam,
    /// Pause the game while the DAW is stopped
    #[id = "follow"]
    pub follow: BoolParam,

    #[nested(id_prefix = "p1", group = "Pulse 1")]
    pub p1: StripParams,
    #[nested(id_prefix = "p2", group = "Pulse 2")]
    pub p2: StripParams,
    #[nested(id_prefix = "wave", group = "Wave")]
    pub wave: StripParams,
    #[nested(id_prefix = "noise", group = "Noise")]
    pub noise: StripParams,
    #[nested(id_prefix = "music", group = "Music")]
    pub music: StripParams,
    #[nested(id_prefix = "sfx", group = "SFX")]
    pub sfx: StripParams,
    #[nested(group = "Rack")]
    pub rack: RackParams,
}

impl SwfParams {
    /// The bus strips, in bus order.
    pub fn strips(&self) -> [&StripParams; BUSES] {
        [&self.p1, &self.p2, &self.wave, &self.noise, &self.music, &self.sfx]
    }
}

impl Default for SwfParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(ui::WIDTH, ui::HEIGHT),
            swf_path: Arc::new(RwLock::new(String::new())),
            mapping: Arc::new(RwLock::new(Mapping::default())),
            saves: Arc::new(RwLock::new(Default::default())),
            volume: FloatParam::new(
                "Volume",
                1.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-60.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(30.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            follow: BoolParam::new("Sync to DAW", false),
            p1: StripParams::new(BUS_NAMES[0]),
            p2: StripParams::new(BUS_NAMES[1]),
            wave: StripParams::new(BUS_NAMES[2]),
            noise: StripParams::new(BUS_NAMES[3]),
            music: StripParams::new(BUS_NAMES[4]),
            sfx: StripParams::new(BUS_NAMES[5]),
            rack: RackParams::default(),
        }
    }
}

pub struct Swf2Daw {
    params: Arc<SwfParams>,
    shared: Arc<Shared>,
    cmds: crossbeam_channel::Sender<Cmd>,
    midi: rtrb::Producer<Midi>,
    stream: StreamResampler,
    engine: Option<std::thread::JoinHandle<()>>,
    /// Ruffle mix, interleaved stereo
    scratch: Vec<f32>,
    /// One block of every bus, per channel (bus 0 L, bus 0 R, …)
    buses: [Vec<f32>; FRAME],
    strips: [Strip; BUSES],
    fx: MasterFx,
    rate: f32,
    scope: Vec<f32>,
    scope_pos: usize,
    /// SWF path the engine was last told to load
    requested: String,
}

impl Default for Swf2Daw {
    fn default() -> Self {
        let params = Arc::new(SwfParams::default());
        let shared = Arc::new(Shared::default());
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let (midi_tx, midi_rx) = rtrb::RingBuffer::new(1024);
        let (stream_tx, stream_rx) = rtrb::RingBuffer::new(audio::STREAM_CAPACITY);
        let engine = Engine::spawn(
            shared.clone(),
            params.mapping.clone(),
            params.saves.clone(),
            cmd_rx,
            midi_rx,
            stream_tx,
        );
        Self {
            params,
            shared,
            cmds: cmd_tx,
            midi: midi_tx,
            stream: StreamResampler::new(stream_rx),
            engine: Some(engine),
            scratch: Vec::new(),
            buses: Default::default(),
            strips: Default::default(),
            fx: MasterFx::new(48000.0),
            rate: 48000.0,
            scope: vec![0.0; SCOPE_LEN],
            scope_pos: 0,
            requested: String::new(),
        }
    }
}

impl Drop for Swf2Daw {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Relaxed);
        if let Some(engine) = self.engine.take() {
            let _ = engine.join();
        }
    }
}

impl Plugin for Swf2Daw {
    const NAME: &'static str = "swf2DAW";
    const VENDOR: &'static str = "Burg Media";
    const URL: &'static str = "https://burgmedia.eu";
    const EMAIL: &'static str = "info@burgmedia.eu";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    /// Main out: the whole mix through the rack. The extra outs: every bus on its own,
    /// after its strip and before the rack, to record the channels on separate tracks.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(2),
            aux_output_ports: &[new_nonzero_u32(2); BUSES],
            names: PortNames {
                layout: Some("Mix + channels"),
                main_output: Some("Mix"),
                aux_outputs: &BUS_NAMES,
                ..PortNames::const_default()
            },
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        ui::create(self.params.clone(), self.shared.clone(), self.cmds.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let max = buffer_config.max_buffer_size as usize;
        self.scratch = vec![0.0; max * 2];
        self.buses = std::array::from_fn(|_| vec![0.0; max]);
        self.rate = buffer_config.sample_rate;
        self.strips = Default::default();
        self.fx = MasterFx::new(buffer_config.sample_rate);
        self.stream.set_rate(buffer_config.sample_rate);
        self.shared
            .sample_rate
            .store(buffer_config.sample_rate as u32, Ordering::Relaxed);
        let _ = self.cmds.send(Cmd::SampleRate(buffer_config.sample_rate as u32));
        // State is restored by now: open the game the project was saved with
        let path = self.params.swf_path.read().unwrap().clone();
        if !path.is_empty() && path != self.requested {
            self.requested = path.clone();
            let _ = self.cmds.send(Cmd::Load(path.into()));
        }
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        while let Some(event) = context.next_event() {
            let m = match event {
                NoteEvent::NoteOn { note, velocity, .. } => Midi::Note(note, velocity > 0.0),
                NoteEvent::NoteOff { note, .. } => Midi::Note(note, false),
                NoteEvent::MidiCC { cc, value, .. } => Midi::Cc(cc, value),
                _ => continue,
            };
            let _ = self.midi.push(m);
        }
        self.shared
            .transport_playing
            .store(context.transport().playing, Ordering::Relaxed);
        self.shared
            .follow_transport
            .store(self.params.follow.value(), Ordering::Relaxed);

        let n = buffer.samples();
        let out = buffer.as_slice();
        let [left, right] = out else {
            return ProcessStatus::Normal;
        };
        left.fill(0.0);
        right.fill(0.0);
        if self.buses[0].len() < n {
            return ProcessStatus::Normal;
        }
        for b in &mut self.buses {
            b[..n].fill(0.0);
        }

        // Ruffle's own sounds go to the Music and SFX buses; skip a block rather than wait
        // if the engine is swapping mixers
        if let Ok(mixers) = self.shared.mixer.try_lock()
            && let Some(mixers) = mixers.as_ref()
            && self.scratch.len() >= n * 2
        {
            for (mixer, bus) in mixers.iter().zip([MUSIC_BUS, SFX_BUS]) {
                let mix = &mut self.scratch[..n * 2];
                mix.fill(0.0);
                mixer.mix::<f32>(mix);
                let [bl, br] = &mut self.buses[bus * 2..bus * 2 + 2] else { unreachable!() };
                for i in 0..n {
                    bl[i] = mix[i * 2];
                    br[i] = mix[i * 2 + 1];
                }
            }
        }

        // gb2swf sound, every bus
        self.stream.render(&mut self.buses, n);

        // Bus strips, into their own outputs and the mix
        let strips = self.params.strips();
        let solo = strips.iter().any(|s| s.solo.value());
        for (k, (sp, strip)) in strips.iter().zip(self.strips.iter_mut()).enumerate() {
            strip.update(self.rate, sp.eq());
            let on = !sp.mute.value() && (!solo || sp.solo.value());
            let [bl, br] = &mut self.buses[k * 2..k * 2 + 2] else { unreachable!() };
            let mut peak = 0.0f32;
            for i in 0..n {
                let gain = sp.level.smoothed.next();
                let (gl, gr) = dsp::pan_gains(sp.pan.smoothed.next());
                let (l, r) = if on {
                    (strip.run(0, bl[i]) * gain * gl, strip.run(1, br[i]) * gain * gr)
                } else {
                    (0.0, 0.0)
                };
                bl[i] = l;
                br[i] = r;
                left[i] += l;
                right[i] += r;
                peak = peak.max(l.abs()).max(r.abs());
            }
            self.shared.meters[k].fetch_max(peak.to_bits(), Ordering::Relaxed);
            if let Some(port) = aux.outputs.get_mut(k)
                && let [ol, or] = port.as_slice()
            {
                ol.copy_from_slice(&bl[..n]);
                or.copy_from_slice(&br[..n]);
            }
        }

        // The rack over the mix
        let transport = context.transport();
        let clock = Clock {
            bpm: transport.tempo.unwrap_or(120.0) as f32,
            beats: if transport.playing { transport.pos_beats() } else { None },
        };
        self.fx.process(&mut left[..n], &mut right[..n], self.params.rack.get(), clock);

        let mut vu = [0.0f32; 2];
        for i in 0..n {
            let gain = self.params.volume.smoothed.next();
            left[i] *= gain;
            right[i] *= gain;
            vu = [vu[0].max(left[i].abs()), vu[1].max(right[i].abs())];
            self.scope[self.scope_pos] = (left[i] + right[i]) * 0.5;
            self.scope_pos = (self.scope_pos + 1) % SCOPE_LEN;
        }
        for (shared, v) in self.shared.vu.iter().zip(vu) {
            shared.fetch_max(v.to_bits(), Ordering::Relaxed);
        }

        if self.params.editor_state.is_open()
            && let Ok(mut scope) = self.shared.scope.try_lock()
        {
            scope.clear();
            scope.extend_from_slice(&self.scope[self.scope_pos..]);
            scope.extend_from_slice(&self.scope[..self.scope_pos]);
            self.shared.scope_serial.fetch_add(1, Ordering::Relaxed);
        }

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Swf2Daw {
    const CLAP_ID: &'static str = "eu.burgmedia.swf2daw";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Play SWF files and gb2swf games inside your DAW");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for Swf2Daw {
    const VST3_CLASS_ID: [u8; 16] = *b"Swf2DawBurgMedia";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(Swf2Daw);
nih_export_vst3!(Swf2Daw);
