//! The Ruffle player lives on its own thread: it holds `Rc`s, so it can't move between
//! threads, and it must never run on the DAW's audio thread. The plugin, the editor and
//! this thread talk through `Shared`, a command channel and two lock-free ring buffers.

use crate::audio::{FRAME, MUSIC_BUS, PluginAudioBackend};
use crate::mapping::{Mapping, Source, Target};
use base64::Engine as _;
use ruffle_core::backend::audio::AudioMixerProxy;
use ruffle_core::backend::storage::StorageBackend;
use ruffle_core::context::UpdateContext;
use ruffle_core::events::{
    KeyDescriptor, KeyLocation, LogicalKey, MouseButton, NamedKey, PhysicalKey,
};
use ruffle_core::external::{ExternalInterfaceProvider, Value};
use ruffle_core::{FloatDuration, Player, PlayerBuilder, PlayerEvent};
use ruffle_render_wgpu::backend::{
    WgpuRenderBackend, create_wgpu_instance, request_adapter_and_device,
};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::target::TextureTarget;
use ruffle_render_wgpu::wgpu;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Biggest picture we render; bigger stages are scaled down.
const MAX_SIDE: f64 = 960.0;

pub enum Cmd {
    Load(PathBuf),
    Eject,
    TogglePause,
    Reset,
    SampleRate(u32),
    /// A mapper button clicked or a key pressed in the editor
    Button(Target, bool),
    /// Mouse on the screen, in 0..1 of the stage
    Mouse(f32, f32),
    /// Mouse X or Y alone (the mapper's axis buttons), 0..1
    MouseAxis(Target, f32),
    MouseButton(bool),
}

/// MIDI from the audio thread.
#[derive(Clone, Copy)]
pub enum Midi {
    Note(u8, bool),
    Cc(u8, f32),
}

#[derive(Default)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub serial: u64,
}

/// Everything the editor shows comes from here.
#[derive(Default)]
pub struct Shared {
    pub frame: Mutex<Frame>,
    /// File name of the loaded SWF ("" = no disc)
    pub title: Mutex<String>,
    /// Error or status line for the LCD
    pub status: Mutex<String>,
    pub loaded_at: Mutex<Option<Instant>>,
    pub paused: AtomicBool,
    pub fps: AtomicU32,
    /// Is this a gb2swf game (it registered gbPad)?
    pub gb_mode: AtomicBool,
    /// Bit per `Target::ALL` index that is held down, for the LEDs
    pub held_bits: AtomicU32,
    /// Mapper button waiting for a MIDI note or CC
    pub learn: Mutex<Option<Target>>,
    pub last_midi: Mutex<String>,
    /// Set by the audio thread
    pub transport_playing: AtomicBool,
    pub sample_rate: AtomicU32,
    pub follow_transport: AtomicBool,
    /// Ruffle's mixers (music, sfx), picked up by the audio thread
    pub mixer: Mutex<Option<[AudioMixerProxy; 2]>>,
    /// Last mono output samples for the spectrum analyzer
    pub scope: Mutex<Vec<f32>>,
    pub scope_serial: AtomicU64,
    /// Peak per bus since the editor last looked (f32 bits), for the strip meters
    pub meters: [AtomicU32; crate::audio::BUSES],
    /// Output peak, left and right, since the editor last looked (f32 bits), for the VU meters
    pub vu: [AtomicU32; 2],
    pub shutdown: AtomicBool,
}

/// SharedObjects (game saves) go into the plugin state, so they are stored in the DAW project.
pub type Saves = Arc<RwLock<HashMap<String, String>>>;

struct ProjectStorage(Saves);

impl StorageBackend for ProjectStorage {
    fn get(&self, name: &str) -> Option<Vec<u8>> {
        let saves = self.0.read().ok()?;
        base64::engine::general_purpose::STANDARD.decode(saves.get(name)?).ok()
    }

    fn put(&mut self, name: &str, value: &[u8]) -> bool {
        let value = base64::engine::general_purpose::STANDARD.encode(value);
        self.0.write().map(|mut s| s.insert(name.to_string(), value)).is_ok()
    }

    fn remove_key(&mut self, name: &str) {
        if let Ok(mut s) = self.0.write() {
            s.remove(name);
        }
    }
}

/// Pretends to be the web page around the SWF, for gb2swf's sound route.
struct PluginExternalInterface {
    stream: Rc<RefCell<rtrb::Producer<f32>>>,
    gb_pad: Rc<Cell<bool>>,
}

impl PluginExternalInterface {
    /// Writes `samples` (`width` values per frame) into the stream at bus channel `at`,
    /// the other buses silent. Whole frames only; drops what doesn't fit.
    fn push(&self, samples: &[Value], width: usize, at: usize) {
        let mut tx = self.stream.borrow_mut();
        let frames = (samples.len() / width).min(tx.slots() / FRAME);
        if let Ok(chunk) = tx.write_chunk_uninit(frames * FRAME) {
            chunk.fill_from_iter((0..frames * FRAME).map(|i| {
                let (f, c) = (i / FRAME, i % FRAME);
                match c.checked_sub(at).filter(|&k| k < width).map(|k| &samples[f * width + k]) {
                    Some(Value::Number(x)) => *x as f32,
                    _ => 0.0,
                }
            }));
        }
    }
}

impl ExternalInterfaceProvider for PluginExternalInterface {
    fn call_method(&self, _context: &mut UpdateContext<'_>, name: &str, args: &[Value]) -> Value {
        match name {
            "gb2swfAudioReady" | "gb2swfStemsReady" => Value::Bool(true),
            // The plain mix goes to the Music bus
            "gb2swfAudio" => {
                if let [Value::List(samples)] = args {
                    self.push(samples, 2, MUSIC_BUS * 2);
                }
                Value::Undefined
            }
            // Pulse 1, Pulse 2, Wave, Noise to buses 0..3
            "gb2swfAudioStems" => {
                if let [Value::List(samples)] = args {
                    self.push(samples, 8, 0);
                }
                Value::Undefined
            }
            _ => Value::Undefined,
        }
    }

    fn on_callback_available(&self, name: &str) {
        if name == "gbPad" {
            self.gb_pad.set(true);
        }
    }

    fn get_id(&self) -> Option<String> {
        None
    }
}

struct Loaded {
    player: Arc<Mutex<Player>>,
    path: PathBuf,
    width: u32,
    height: u32,
    gb_pad: Rc<Cell<bool>>,
}

pub struct Engine {
    shared: Arc<Shared>,
    mapping: Arc<RwLock<Mapping>>,
    saves: Saves,
    cmds: crossbeam_channel::Receiver<Cmd>,
    midi: rtrb::Consumer<Midi>,
    stream: Rc<RefCell<rtrb::Producer<f32>>>,
    gpu: Option<Arc<Descriptors>>,
    loaded: Option<Loaded>,
    sample_rate: u32,
    /// How many sources hold each target down
    held: [u8; Target::ALL.len()],
    mouse: (f32, f32),
    mouse_down: bool,
}

impl Engine {
    pub fn spawn(
        shared: Arc<Shared>,
        mapping: Arc<RwLock<Mapping>>,
        saves: Saves,
        cmds: crossbeam_channel::Receiver<Cmd>,
        midi: rtrb::Consumer<Midi>,
        stream: rtrb::Producer<f32>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::Builder::new()
            .name("swf2daw-engine".into())
            .spawn(move || {
                Engine {
                    shared,
                    mapping,
                    saves,
                    cmds,
                    midi,
                    stream: Rc::new(RefCell::new(stream)),
                    gpu: None,
                    loaded: None,
                    sample_rate: 44100,
                    held: [0; Target::ALL.len()],
                    mouse: (0.5, 0.5),
                    mouse_down: false,
                }
                .run()
            })
            .expect("engine thread")
    }

    fn run(mut self) {
        let mut last = Instant::now();
        let mut frames = 0u32;
        let mut fps_at = Instant::now();
        while !self.shared.shutdown.load(Ordering::Relaxed) {
            while let Ok(cmd) = self.cmds.try_recv() {
                self.command(cmd);
            }
            while let Ok(m) = self.midi.pop() {
                self.midi_in(m);
            }

            let now = Instant::now();
            let dt = now - last;
            last = now;
            let Some(loaded) = &self.loaded else {
                std::thread::sleep(Duration::from_millis(15));
                continue;
            };

            let held_by_daw = self.shared.follow_transport.load(Ordering::Relaxed)
                && !self.shared.transport_playing.load(Ordering::Relaxed);
            let mut player = loaded.player.lock().unwrap();
            if !held_by_daw {
                player.tick(FloatDuration::from_secs(dt.as_secs_f64()));
            }
            if player.needs_render() {
                player.render();
                let renderer = <dyn Any>::downcast_mut::<WgpuRenderBackend<TextureTarget>>(
                    player.renderer_mut(),
                )
                .unwrap();
                if let Some(image) = renderer.capture_frame() {
                    let mut frame = self.shared.frame.lock().unwrap();
                    frame.width = image.width();
                    frame.height = image.height();
                    frame.rgba = image.into_raw();
                    frame.serial += 1;
                    frames += 1;
                }
            }
            let wait = player.time_til_next_frame();
            drop(player);
            self.shared.gb_mode.store(loaded.gb_pad.get(), Ordering::Relaxed);

            if fps_at.elapsed() >= Duration::from_secs(1) {
                self.shared.fps.store(frames, Ordering::Relaxed);
                frames = 0;
                fps_at = Instant::now();
            }
            // Short naps so MIDI input stays snappy
            std::thread::sleep(wait.clamp(Duration::from_millis(1), Duration::from_millis(4)));
        }
        self.unload();
    }

    fn set_status(&self, s: impl Into<String>) {
        *self.shared.status.lock().unwrap() = s.into();
    }

    fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Load(path) => {
                // The plugin asks again after every re-activation; keep the running game
                if self.loaded.as_ref().is_none_or(|l| l.path != path) {
                    self.load(path);
                }
            }
            Cmd::Eject => {
                self.unload();
                self.set_status("");
            }
            Cmd::TogglePause => {
                if let Some(l) = &self.loaded {
                    let mut p = l.player.lock().unwrap();
                    let playing = !p.is_playing();
                    p.set_is_playing(playing);
                    self.shared.paused.store(!playing, Ordering::Relaxed);
                }
            }
            Cmd::Reset => {
                if let Some(path) = self.loaded.as_ref().map(|l| l.path.clone()) {
                    self.load(path);
                }
            }
            Cmd::SampleRate(rate) => {
                if rate != self.sample_rate {
                    self.sample_rate = rate;
                    // The mixer is built for one rate: start the movie again
                    if let Some(path) = self.loaded.as_ref().map(|l| l.path.clone()) {
                        self.load(path);
                    }
                }
            }
            Cmd::Button(t, down) => self.press(t, down),
            Cmd::Mouse(x, y) => self.mouse_to(x, y),
            Cmd::MouseAxis(Target::MouseX, x) => self.mouse_to(x, self.mouse.1),
            Cmd::MouseAxis(_, y) => self.mouse_to(self.mouse.0, y),
            Cmd::MouseButton(down) => self.click(down),
        }
    }

    fn gpu(&mut self) -> Result<Arc<Descriptors>, String> {
        if let Some(d) = &self.gpu {
            return Ok(d.clone());
        }
        let backends = wgpu::Backends::all();
        let instance = create_wgpu_instance(backends, wgpu::BackendOptions::default(), None);
        let (adapter, device, queue) = futures::executor::block_on(request_adapter_and_device(
            backends,
            &instance,
            None,
            wgpu::PowerPreference::HighPerformance,
        ))
        .map_err(|e| e.to_string())?;
        let d = Arc::new(Descriptors::new(instance, adapter, device, queue));
        self.gpu = Some(d.clone());
        Ok(d)
    }

    fn load(&mut self, path: PathBuf) {
        self.unload();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.set_status("LOADING...");
        match self.build_player(&path) {
            Ok(loaded) => {
                *self.shared.title.lock().unwrap() = name;
                *self.shared.loaded_at.lock().unwrap() = Some(Instant::now());
                self.shared.paused.store(false, Ordering::Relaxed);
                self.loaded = Some(loaded);
                self.set_status("");
            }
            Err(e) => self.set_status(format!("ERROR: {e}")),
        }
    }

    fn build_player(&mut self, path: &std::path::Path) -> Result<Loaded, String> {
        let movie =
            ruffle_core::tag_utils::movie_from_path(path, None).map_err(|e| e.to_string())?;
        let scale = (MAX_SIDE / movie.width().to_pixels().max(movie.height().to_pixels())).min(1.0);
        let width = ((movie.width().to_pixels() * scale).round() as u32).max(1);
        let height = ((movie.height().to_pixels() * scale).round() as u32).max(1);

        let gpu = self.gpu()?;
        let target = TextureTarget::new(&gpu.device, (width, height)).map_err(|e| e.to_string())?;
        let renderer = WgpuRenderBackend::new(gpu, target).map_err(|e| e.to_string())?;
        let audio = PluginAudioBackend::new(self.sample_rate);
        *self.shared.mixer.lock().unwrap() = Some(audio.proxies());

        let gb_pad = Rc::new(Cell::new(false));
        let player = PlayerBuilder::new()
            .with_renderer(renderer)
            .with_audio(audio)
            .with_storage(Box::new(ProjectStorage(self.saves.clone())))
            .with_external_interface(Box::new(PluginExternalInterface {
                stream: self.stream.clone(),
                gb_pad: gb_pad.clone(),
            }))
            .with_movie(movie)
            .with_viewport_dimensions(width, height, 1.0)
            .with_autoplay(true)
            .build();

        Ok(Loaded {
            player,
            path: path.to_path_buf(),
            width,
            height,
            gb_pad,
        })
    }

    fn unload(&mut self) {
        *self.shared.mixer.lock().unwrap() = None;
        if let Some(l) = self.loaded.take() {
            // Write pending saves before the player goes away
            l.player.lock().unwrap().flush_shared_objects();
        }
        *self.shared.title.lock().unwrap() = String::new();
        *self.shared.loaded_at.lock().unwrap() = None;
        *self.shared.frame.lock().unwrap() = Frame::default();
        self.held = [0; Target::ALL.len()];
        self.shared.held_bits.store(0, Ordering::Relaxed);
        self.shared.gb_mode.store(false, Ordering::Relaxed);
    }

    fn midi_in(&mut self, m: Midi) {
        let (src, down, value) = match m {
            Midi::Note(n, on) => (Source::Note(n), on, if on { 1.0 } else { 0.0 }),
            Midi::Cc(c, v) => (Source::Cc(c), v >= 0.5, v),
        };
        *self.shared.last_midi.lock().unwrap() = src.name();

        let learning = self.shared.learn.lock().unwrap().take();
        if let Some(target) = learning {
            // Learn on a press, so the release of the same key doesn't trigger anything
            if down || matches!(src, Source::Cc(_)) {
                self.mapping.write().unwrap().learn(src, target);
                return;
            }
            *self.shared.learn.lock().unwrap() = Some(target);
        }

        let Some(target) = self.mapping.read().unwrap().get(src) else {
            return;
        };
        match target {
            Target::MouseX => self.mouse_to(value, self.mouse.1),
            Target::MouseY => self.mouse_to(self.mouse.0, value),
            Target::Click => self.click(down),
            t => self.press(t, down),
        }
    }

    fn press(&mut self, t: Target, down: bool) {
        let i = t.index();
        let was = self.held[i] > 0;
        self.held[i] = if down { self.held[i].saturating_add(1) } else { self.held[i].saturating_sub(1) };
        let is = self.held[i] > 0;
        if was == is {
            return;
        }
        let bits = self.shared.held_bits.load(Ordering::Relaxed);
        self.shared
            .held_bits
            .store(if is { bits | 1 << i } else { bits & !(1 << i) }, Ordering::Relaxed);

        if t == Target::Click {
            self.click(is);
            return;
        }
        let Some(l) = &self.loaded else { return };
        let mut player = l.player.lock().unwrap();
        if l.gb_pad.get() && t.gb_bit().is_some() {
            // gb2swf: set the whole pad at once, no keyboard focus needed
            let mask = Target::ALL
                .iter()
                .filter_map(|t| t.gb_bit().filter(|_| self.held[t.index()] > 0))
                .fold(0u32, |m, b| m | 1 << b);
            player.call_internal_interface("gbPad", [Value::Number(mask as f64)]);
        } else if let Some(key) = key_for(t) {
            player.handle_event(if is {
                PlayerEvent::KeyDown { key }
            } else {
                PlayerEvent::KeyUp { key }
            });
        }
    }

    fn mouse_to(&mut self, x: f32, y: f32) {
        self.mouse = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        if let Some(l) = &self.loaded {
            let (x, y) = (self.mouse.0 as f64 * l.width as f64, self.mouse.1 as f64 * l.height as f64);
            l.player.lock().unwrap().handle_event(PlayerEvent::MouseMove { x, y });
        }
    }

    fn click(&mut self, down: bool) {
        if down == self.mouse_down {
            return;
        }
        self.mouse_down = down;
        if let Some(l) = &self.loaded {
            let (x, y) = (self.mouse.0 as f64 * l.width as f64, self.mouse.1 as f64 * l.height as f64);
            let button = MouseButton::Left;
            l.player.lock().unwrap().handle_event(if down {
                PlayerEvent::MouseDown { x, y, button, index: None }
            } else {
                PlayerEvent::MouseUp { x, y, button }
            });
        }
    }
}

fn key_for(t: Target) -> Option<KeyDescriptor> {
    let (physical_key, logical_key, key_location) = match t {
        Target::Up => (PhysicalKey::ArrowUp, LogicalKey::Named(NamedKey::ArrowUp), KeyLocation::Standard),
        Target::Down => (PhysicalKey::ArrowDown, LogicalKey::Named(NamedKey::ArrowDown), KeyLocation::Standard),
        Target::Left => (PhysicalKey::ArrowLeft, LogicalKey::Named(NamedKey::ArrowLeft), KeyLocation::Standard),
        Target::Right => (PhysicalKey::ArrowRight, LogicalKey::Named(NamedKey::ArrowRight), KeyLocation::Standard),
        Target::A => (PhysicalKey::KeyX, LogicalKey::Character('x'), KeyLocation::Standard),
        Target::B => (PhysicalKey::KeyZ, LogicalKey::Character('z'), KeyLocation::Standard),
        Target::Start => (PhysicalKey::Enter, LogicalKey::Named(NamedKey::Enter), KeyLocation::Standard),
        Target::Select => (PhysicalKey::ShiftLeft, LogicalKey::Named(NamedKey::Shift), KeyLocation::Left),
        Target::Space => (PhysicalKey::Space, LogicalKey::Character(' '), KeyLocation::Standard),
        Target::KeyC => (PhysicalKey::KeyC, LogicalKey::Character('c'), KeyLocation::Standard),
        Target::MouseX | Target::MouseY | Target::Click => return None,
    };
    Some(KeyDescriptor { physical_key, logical_key, key_location })
}
