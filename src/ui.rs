//! The editor: a 1990s pro-audio rack unit, all drawn by hand. Walnut cheeks, a brushed
//! aluminium faceplate, a CRT for the game, a cyan VFD with a scrolling ticker, a VFD
//! spectrum, two needle VU meters, chunky hardware keys and a rack with tabs: the channel
//! mixer, the EQ and effects, and the MIDI mapper.
//!
//! Every key, knob and switch does something, every lamp shows real state.

use crate::audio::BUSES;
use crate::engine::{Cmd, Shared};
use crate::mapping::{Mapping, Source, Target};
use crate::params::{BUS_NAMES, EQ_LABELS, FxParams};
use crate::{SCOPE_LEN, SwfParams};
use nih_plug::prelude::{BoolParam, Editor, FloatParam, Param, ParamSetter};
use nih_plug_egui::egui::{
    self, Align2, Color32, Context, FontId, Mesh, Pos2, Rect, Sense, Shape, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Vec2, pos2, vec2,
};
use nih_plug_egui::create_egui_editor;
use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const WIDTH: u32 = 1000;
pub const HEIGHT: u32 = 770;

const BARS: usize = 32;

// Palette
const ALU_TOP: Color32 = Color32::from_rgb(0xe2, 0xe5, 0xe9);
const ALU_BOT: Color32 = Color32::from_rgb(0x9a, 0x9f, 0xa7);
const INK: Color32 = Color32::from_rgb(0x1c, 0x1e, 0x22);
const SILK: Color32 = Color32::from_rgb(0xdd, 0xe2, 0xea);
const SILK_DIM: Color32 = Color32::from_rgb(0x84, 0x8b, 0x96);
const VFD: Color32 = Color32::from_rgb(0x5c, 0xff, 0xe6);
const VFD_DIM: Color32 = Color32::from_rgb(0x0c, 0x34, 0x30);
const VFD_BG: Color32 = Color32::from_rgb(0x02, 0x0b, 0x0c);
const AMBER: Color32 = Color32::from_rgb(0xff, 0xb0, 0x2e);
const RED: Color32 = Color32::from_rgb(0xff, 0x3b, 0x30);
const GREEN: Color32 = Color32::from_rgb(0x6b, 0xff, 0x7a);
const YELLOW: Color32 = Color32::from_rgb(0xff, 0xd8, 0x3b);
const CYAN: Color32 = Color32::from_rgb(0x39, 0xd5, 0xff);
/// One colour per bus, used on its strip.
const BUS_COLORS: [Color32; BUSES] = [
    CYAN,
    GREEN,
    AMBER,
    Color32::from_rgb(0xff, 0x5f, 0xa2),
    Color32::from_rgb(0xb0, 0x8c, 0xff),
    YELLOW,
];

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Mixer,
    Rack,
    Midi,
}

struct UiState {
    tex: Option<TextureHandle>,
    frame_serial: u64,
    scope_serial: u64,
    /// Time the DSP last delivered a block, for the DSP lamp
    dsp_at: f64,
    bars: [f32; BARS],
    peaks: [f32; BARS],
    last_time: f64,
    screen_down: bool,
    shift: bool,
    picking: Arc<AtomicBool>,
    tab: Tab,
    /// Strip meters, 0..1, falling slowly
    meters: [f32; BUSES],
    /// VU needles, 0..1, and the time each peak lamp last lit
    vu: [f32; 2],
    clip_at: [f64; 2],
    /// Mapper buttons held with the mouse
    mouse_held: [bool; Target::ALL.len()],
}

pub fn create(
    params: Arc<SwfParams>,
    shared: Arc<Shared>,
    cmds: crossbeam_channel::Sender<Cmd>,
) -> Option<Box<dyn Editor>> {
    let state = UiState {
        tex: None,
        frame_serial: 0,
        scope_serial: 0,
        dsp_at: -10.0,
        bars: [0.0; BARS],
        peaks: [0.0; BARS],
        last_time: 0.0,
        screen_down: false,
        shift: false,
        picking: Arc::new(AtomicBool::new(false)),
        tab: Tab::Mixer,
        meters: [0.0; BUSES],
        vu: [0.0; 2],
        clip_at: [-10.0; 2],
        mouse_held: [false; Target::ALL.len()],
    };
    create_egui_editor(
        params.editor_state.clone(),
        state,
        |_, _| {},
        move |ctx, setter, st| {
            egui::CentralPanel::default()
                .frame(egui::Frame::default())
                .show(ctx, |ui| draw(ui, ctx, setter, st, &params, &shared, &cmds));
            ctx.request_repaint();
        },
    )
}

fn draw(
    ui: &mut egui::Ui,
    ctx: &Context,
    setter: &ParamSetter,
    st: &mut UiState,
    params: &Arc<SwfParams>,
    shared: &Arc<Shared>,
    cmds: &crossbeam_channel::Sender<Cmd>,
) {
    let o = ui.max_rect().min.to_vec2();
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(pos2(x0, y0) + o, pos2(x1, y1) + o);
    let p = ui.painter().clone();
    let time = ctx.input(|i| i.time);
    let dt = ((time - st.last_time) as f32).clamp(0.0, 0.1);
    st.last_time = time;

    let title = shared.title.lock().unwrap().clone();
    let status = shared.status.lock().unwrap().clone();
    let loaded = !title.is_empty();
    let paused = shared.paused.load(Ordering::Relaxed);
    let gb_mode = shared.gb_mode.load(Ordering::Relaxed);
    let held = shared.held_bits.load(Ordering::Relaxed);
    let learning = *shared.learn.lock().unwrap();
    let blink = (time * 4.0) as i64 % 2 == 0;

    keyboard(ctx, st, cmds);

    // ---------- cabinet ----------
    let w = WIDTH as f32;
    let h = HEIGHT as f32;
    p.rect_filled(r(0.0, 0.0, w, h), 0.0, Color32::from_rgb(10, 6, 4));
    wood(&p, r(0.0, 0.0, 16.0, h));
    wood(&p, r(w - 16.0, 0.0, w, h));
    let face = r(16.0, 0.0, w - 16.0, h);
    brushed(&p, face);
    // bevel where the faceplate meets the wood
    p.line_segment([face.left_top(), face.left_bottom()], Stroke::new(2.0, Color32::from_white_alpha(120)));
    p.line_segment([face.right_top(), face.right_bottom()], Stroke::new(2.0, Color32::from_black_alpha(140)));
    for c in [pos2(30.0, 14.0), pos2(w - 30.0, 14.0), pos2(30.0, h - 14.0), pos2(w - 30.0, h - 14.0)] {
        screw(&p, c + o, 6.0, (c.x * 7.0 + c.y) % 3.0);
    }

    // ---------- header: logo and model plate ----------
    chrome_logo(&p, pos2(48.0, 33.0) + o, "swf2DAW");
    engrave(&p, pos2(206.0, 25.0) + o, Align2::LEFT_CENTER, "DIGITAL ENTERTAINMENT PROCESSOR", FontId::proportional(11.0));
    engrave(&p, pos2(206.0, 41.0) + o, Align2::LEFT_CENTER, "FLASH  •  GAME BOY  •  32-BIT FLOAT  •  RUFFLE ENGINE", FontId::proportional(9.0));
    let plate = r(w - 300.0, 14.0, w - 48.0, 52.0);
    p.rect_filled(plate, 3.0, Color32::from_rgb(0x1a, 0x1b, 0x1f));
    p.rect_stroke(plate, 3.0, Stroke::new(1.0, Color32::from_white_alpha(90)), StrokeKind::Outside);
    p.text(plate.left_center() + vec2(10.0, -7.0), Align2::LEFT_CENTER, "MODEL SWF-2000", FontId::proportional(11.0), SILK);
    p.text(
        plate.left_center() + vec2(10.0, 8.0),
        Align2::LEFT_CENTER,
        concat!("BURG MEDIA  •  FIRMWARE ", env!("CARGO_PKG_VERSION")),
        FontId::proportional(8.0),
        SILK_DIM,
    );
    // Real lamps: the audio thread is delivering blocks, a game is loaded
    let dsp_serial = shared.scope_serial.load(Ordering::Relaxed);
    if dsp_serial != st.scope_serial {
        st.dsp_at = time;
    }
    let dsp_on = time - st.dsp_at < 0.5;
    lamp(&p, plate.right_center() + vec2(-92.0, 0.0), "DSP", dsp_on, GREEN);
    lamp(&p, plate.right_center() + vec2(-46.0, 0.0), "DISC", loaded, AMBER);

    // ---------- CRT ----------
    let bezel = r(30.0, 66.0, 512.0, 450.0);
    grad_rrect(&p, bezel, 14.0, Color32::from_gray(48), Color32::from_gray(14));
    p.rect_stroke(bezel, 14.0, Stroke::new(1.5, Color32::from_black_alpha(220)), StrokeKind::Outside);
    p.text(bezel.center_bottom() + vec2(0.0, -9.0), Align2::CENTER_CENTER, "TRINITUBE  •  VIDEO MONITOR", FontId::proportional(8.0), SILK_DIM);
    let screen = Rect::from_min_max(bezel.min + vec2(16.0, 16.0), bezel.max - vec2(16.0, 24.0));
    p.rect_filled(screen, 10.0, Color32::BLACK);
    let mut game_rect = screen.shrink(4.0);

    {
        let frame = shared.frame.lock().unwrap();
        if frame.width > 0 && frame.serial != st.frame_serial {
            st.frame_serial = frame.serial;
            let image = egui::ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &frame.rgba);
            match &mut st.tex {
                Some(t) => t.set(image, TextureOptions::NEAREST),
                None => st.tex = Some(ctx.load_texture("swf", image, TextureOptions::NEAREST)),
            }
        }
        if frame.width == 0 {
            st.tex = None;
        } else {
            let area = screen.shrink(4.0);
            let aspect = frame.width as f32 / frame.height as f32;
            let size = if area.width() / area.height() > aspect {
                vec2(area.height() * aspect, area.height())
            } else {
                vec2(area.width(), area.width() / aspect)
            };
            game_rect = Rect::from_center_size(area.center(), size);
        }
    }
    if let Some(tex) = &st.tex {
        p.image(tex.id(), game_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        attract(&p, screen, time, &status);
    }
    // scanlines, vignette, glass
    let sp = p.with_clip_rect(screen);
    let mut y = screen.top();
    while y < screen.bottom() {
        sp.line_segment([pos2(screen.left(), y), pos2(screen.right(), y)], Stroke::new(1.0, Color32::from_black_alpha(46)));
        y += 3.0;
    }
    for i in 0..10 {
        let k = i as f32;
        sp.rect_stroke(screen.shrink(k * 2.0), 10.0, Stroke::new(2.0, Color32::from_black_alpha((60.0 - k * 6.0) as u8)), StrokeKind::Inside);
    }
    let mut glass = Mesh::default();
    glass.colored_vertex(screen.left_top(), Color32::from_white_alpha(26));
    glass.colored_vertex(pos2(screen.left() + screen.width() * 0.6, screen.top()), Color32::from_white_alpha(6));
    glass.colored_vertex(pos2(screen.left(), screen.top() + screen.height() * 0.5), Color32::from_white_alpha(0));
    glass.add_triangle(0, 1, 2);
    sp.add(Shape::mesh(glass));
    if loaded && !status.is_empty() {
        sp.text(screen.center_bottom() - vec2(0.0, 14.0), Align2::CENTER_CENTER, &status, FontId::monospace(12.0), AMBER);
    }

    // mouse on the game
    let screen_resp = ui.interact(game_rect, ui.id().with("screen"), Sense::click_and_drag());
    if loaded {
        if let Some(pos) = screen_resp.hover_pos() {
            let rel = (pos - game_rect.min) / game_rect.size();
            let _ = cmds.send(Cmd::Mouse(rel.x, rel.y));
        }
        let down = screen_resp.is_pointer_button_down_on();
        if down != st.screen_down {
            st.screen_down = down;
            let _ = cmds.send(Cmd::MouseButton(down));
        }
    }

    // ---------- VFD ----------
    let col_l = 526.0;
    let col_r = w - 30.0;
    let vfd = r(col_l, 66.0, col_r, 160.0);
    section(&p, vfd, None);
    let glass_vfd = vfd.shrink(7.0);
    vfd_glass(&p, glass_vfd);
    let lp = p.with_clip_rect(glass_vfd);
    let rate = shared.sample_rate.load(Ordering::Relaxed) as f32 / 1000.0;
    let fps = shared.fps.load(Ordering::Relaxed);
    let ticker = if loaded {
        format!(
            "NOW PLAYING: {}  ***  {} FPS  ***  {:.1} kHz  ***  {}  ***  ",
            title.to_uppercase(),
            fps,
            rate,
            if gb_mode { "GAME BOY MODE" } else { "FLASH MODE" }
        )
    } else {
        "NO DISC  ***  PRESS EJECT TO LOAD A .SWF  ***  SWF-2000 READY  ***  ".to_string()
    };
    let font = FontId::monospace(13.0);
    let tw = p.layout_no_wrap(ticker.clone(), font.clone(), VFD).size().x.max(1.0);
    let scroll = (time as f32 * 45.0) % tw;
    let mut tx = glass_vfd.left() + 8.0 - scroll;
    while tx < glass_vfd.right() {
        glow_text(&lp, pos2(tx, glass_vfd.top() + 13.0), Align2::LEFT_CENTER, &ticker, font.clone(), VFD, VFD.gamma_multiply(0.18));
        tx += tw;
    }
    let secs = shared.loaded_at.lock().unwrap().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let big = FontId::monospace(38.0);
    let time_pos = pos2(glass_vfd.left() + 10.0, glass_vfd.top() + 50.0);
    lp.text(time_pos, Align2::LEFT_CENTER, "88:88", big.clone(), VFD_DIM);
    glow_text(&lp, time_pos, Align2::LEFT_CENTER, &format!("{:02}:{:02}", (secs / 60) % 100, secs % 60), big, VFD, VFD.gamma_multiply(0.22));
    let mode = if !loaded { "STOP" } else if paused { "PAUSE" } else { "PLAY" };
    let last_midi = shared.last_midi.lock().unwrap().clone();
    let fx_x = glass_vfd.left() + 150.0;
    for (i, (label, on)) in [
        ("PLAY", loaded && !paused),
        ("PAUSE", loaded && paused && blink),
        ("GB", gb_mode),
        ("SWF", loaded && !gb_mode),
        ("SYNC", params.follow.value()),
    ]
    .into_iter()
    .enumerate()
    {
        vfd_flag(&lp, pos2(fx_x + i as f32 * 46.0, glass_vfd.top() + 38.0), label, on);
    }
    lp.text(pos2(fx_x, glass_vfd.top() + 60.0), Align2::LEFT_CENTER, format!("MIDI IN {}", if last_midi.is_empty() { "--" } else { &last_midi }), FontId::monospace(11.0), VFD.gamma_multiply(0.85));
    lp.text(pos2(glass_vfd.right() - 8.0, glass_vfd.top() + 60.0), Align2::RIGHT_CENTER, format!("{rate:.1}kHz {mode}"), FontId::monospace(11.0), VFD.gamma_multiply(0.85));

    // ---------- VFD spectrum ----------
    let spec = r(col_l, 166.0, col_r, 254.0);
    section(&p, spec, None);
    let area = spec.shrink(8.0);
    vfd_glass(&p, area);
    let serial = shared.scope_serial.load(Ordering::Relaxed);
    let mut levels = [0.0f32; BARS];
    if serial != st.scope_serial {
        st.scope_serial = serial;
        if let Ok(scope) = shared.scope.lock()
            && scope.len() == SCOPE_LEN
        {
            levels = spectrum(&scope, shared.sample_rate.load(Ordering::Relaxed).max(1) as f32);
        }
    }
    let seg_h = 3.0;
    let inner = area.shrink2(vec2(6.0, 5.0));
    let segs = (inner.height() / seg_h) as usize;
    let bw = inner.width() / BARS as f32;
    for b in 0..BARS {
        st.bars[b] = levels[b].max(st.bars[b] - dt * 1.8);
        st.peaks[b] = st.bars[b].max(st.peaks[b] - dt * 0.4);
        let x0 = inner.left() + b as f32 * bw + 1.0;
        let lit = (st.bars[b] * segs as f32) as usize;
        for s in 0..segs {
            let y1 = inner.bottom() - s as f32 * seg_h;
            let seg = Rect::from_min_max(pos2(x0, y1 - seg_h + 1.0), pos2(x0 + bw - 2.0, y1));
            p.rect_filled(seg, 0.0, if s < lit { VFD } else { VFD_DIM.gamma_multiply(0.6) });
        }
        let py = inner.bottom() - (st.peaks[b] * segs as f32).floor() * seg_h;
        p.rect_filled(Rect::from_min_max(pos2(x0, py - seg_h + 1.0), pos2(x0 + bw - 2.0, py)), 0.0, AMBER);
    }

    // ---------- VU meters + volume ----------
    let vu_row = r(col_l, 260.0, col_r, 360.0);
    section(&p, vu_row, None);
    let vu_w = (vu_row.width() - 140.0) / 2.0;
    for ch in 0..2 {
        let peak = f32::from_bits(shared.vu[ch].swap(0, Ordering::Relaxed));
        if peak > 0.98 {
            st.clip_at[ch] = time;
        }
        // 0 VU = -10 dBFS, scale -20..+3 VU
        let vu_db = 20.0 * (peak + 1e-9).log10() + 10.0;
        let target = ((vu_db + 20.0) / 23.0).clamp(0.0, 1.05);
        let k = if target > st.vu[ch] { 14.0 } else { 3.5 };
        st.vu[ch] += (target - st.vu[ch]) * (dt * k).min(1.0);
        let rect = Rect::from_min_size(vu_row.left_top() + vec2(8.0 + ch as f32 * (vu_w + 6.0), 8.0), vec2(vu_w, vu_row.height() - 16.0));
        vu_meter(&p, rect, st.vu[ch], if ch == 0 { "L" } else { "R" }, time - st.clip_at[ch] < 0.6);
    }
    let vol_c = pos2(vu_row.right() - 66.0, vu_row.center().y - 6.0);
    knob(ui, &p, setter, &params.volume, vol_c, 28.0, "MASTER", AMBER, false);

    // ---------- transport ----------
    let tr = r(col_l, 366.0, col_r, 450.0);
    section(&p, tr, Some("TRANSPORT"));
    let names = ["EJECT", if paused { "PLAY" } else { "PAUSE" }, "STOP", "RESET"];
    let kw = (tr.width() - 16.0 - 3.0 * 8.0) / 4.0;
    for (i, name) in names.iter().enumerate() {
        let rect = Rect::from_min_size(tr.left_top() + vec2(8.0 + i as f32 * (kw + 8.0), 30.0), vec2(kw, 36.0));
        let resp = ui.interact(rect, ui.id().with(("transport", i)), Sense::click());
        let led = match i {
            1 => Some(loaded && !paused),
            2 => Some(!loaded),
            _ => None,
        };
        hw_key(&p, rect, resp.is_pointer_button_down_on(), resp.hovered(), led, GREEN);
        let c = rect.center() + if resp.is_pointer_button_down_on() { vec2(0.0, 1.5) } else { Vec2::ZERO };
        let ic = Color32::from_gray(if resp.hovered() { 250 } else { 215 });
        match i {
            0 => icon_eject(&p, c, ic),
            1 if paused || !loaded => icon_play(&p, c, ic),
            1 => icon_pause(&p, c, ic),
            2 => icon_stop(&p, c, ic),
            _ => icon_reset(&p, c, ic),
        }
        p.text(rect.center_bottom() + vec2(0.0, 9.0), Align2::CENTER_CENTER, *name, FontId::proportional(8.0), SILK_DIM);
        if resp.clicked() {
            match i {
                0 => pick_swf(st, params, cmds),
                1 => drop(cmds.send(Cmd::TogglePause)),
                2 => {
                    *params.swf_path.write().unwrap() = String::new();
                    let _ = cmds.send(Cmd::Eject);
                }
                _ => drop(cmds.send(Cmd::Reset)),
            }
        }
    }
    let sync = Rect::from_min_size(tr.right_top() + vec2(-138.0, 5.0), vec2(130.0, 18.0));
    switch(ui, &p, setter, &params.follow, sync, "SYNC TO DAW", GREEN);

    // ---------- rack ----------
    let panel = r(30.0, 460.0, w - 30.0, h - 26.0);
    section(&p, panel, None);
    let tabs = [(Tab::Mixer, "CHANNEL MIXER"), (Tab::Rack, "EQ + EFFECTS"), (Tab::Midi, "MIDI MAPPER")];
    for (i, (tab, label)) in tabs.into_iter().enumerate() {
        let rect = Rect::from_min_size(panel.left_top() + vec2(10.0 + i as f32 * 138.0, 8.0), vec2(130.0, 24.0));
        let resp = ui.interact(rect, ui.id().with(("tab", i)), Sense::click());
        let down = resp.is_pointer_button_down_on();
        hw_key(&p, rect, down || st.tab == tab, resp.hovered(), Some(st.tab == tab), AMBER);
        p.text(rect.center() + vec2(6.0, if down { 1.0 } else { 0.0 }), Align2::CENTER_CENTER, label, FontId::proportional(10.0), if st.tab == tab { Color32::WHITE } else { SILK_DIM });
        if resp.clicked() {
            st.tab = tab;
        }
    }
    let body = Rect::from_min_max(panel.left_top() + vec2(10.0, 42.0), panel.right_bottom() - vec2(10.0, 10.0));
    match st.tab {
        Tab::Mixer => mixer_panel(ui, &p, setter, st, params, shared, panel, body, dt, gb_mode),
        Tab::Rack => rack_panel(ui, &p, setter, params, panel, body),
        Tab::Midi => midi_panel(ui, &p, st, params, shared, cmds, panel, body, held, learning, blink),
    }
    engrave(&p, pos2(w / 2.0, h - 12.0) + o, Align2::CENTER_CENTER, "BURG MEDIA  •  FLASH IS NOT DEAD  •  MADE WITH RUFFLE AND NIH-PLUG", FontId::proportional(8.0));
}

/// A strip per bus: 3-band EQ, level, pan, mute, solo and a meter.
#[allow(clippy::too_many_arguments)]
fn mixer_panel(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    setter: &ParamSetter,
    st: &mut UiState,
    params: &SwfParams,
    shared: &Shared,
    panel: Rect,
    body: Rect,
    dt: f32,
    gb_mode: bool,
) {
    let strips = params.strips();
    let reset = Rect::from_min_size(panel.right_top() + vec2(-120.0, 8.0), vec2(110.0, 24.0));
    if text_key(ui, p, reset, "RESET MIXER", "reset-mixer") {
        for sp in strips {
            reset_param(setter, &sp.level);
            reset_param(setter, &sp.pan);
            reset_param(setter, &sp.low);
            reset_param(setter, &sp.mid);
            reset_param(setter, &sp.high);
            reset_param(setter, &sp.mute);
            reset_param(setter, &sp.solo);
        }
    }
    p.text(reset.left_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, "each channel has its own plugin output", FontId::proportional(9.0), SILK_DIM);

    let gap = 6.0;
    let w = (body.width() - (BUSES - 1) as f32 * gap) / BUSES as f32;
    for (k, sp) in strips.into_iter().enumerate() {
        let color = BUS_COLORS[k];
        let strip = Rect::from_min_size(body.left_top() + vec2(k as f32 * (w + gap), 0.0), vec2(w, body.height()));
        grad_rrect(p, strip, 6.0, Color32::from_gray(38), Color32::from_gray(18));
        p.rect_stroke(strip, 6.0, Stroke::new(1.0, Color32::from_black_alpha(200)), StrokeKind::Inside);
        p.line_segment([strip.left_top() + vec2(6.0, 1.0), strip.right_top() + vec2(-6.0, 1.0)], Stroke::new(1.0, Color32::from_white_alpha(30)));

        // channel name on a little VFD
        let name = Rect::from_min_size(strip.left_top() + vec2(6.0, 6.0), vec2(w - 30.0, 20.0));
        vfd_glass(p, name);
        glow_text(p, name.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, &BUS_NAMES[k].to_uppercase(), FontId::monospace(11.0), color, color.gamma_multiply(0.18));
        let src = if k >= 4 { "SWF" } else if gb_mode { "GB" } else { "--" };
        p.text(name.right_center() - vec2(5.0, 0.0), Align2::RIGHT_CENTER, src, FontId::monospace(8.0), color.gamma_multiply(0.55));

        // meter on the right edge
        let peak = f32::from_bits(shared.meters[k].swap(0, Ordering::Relaxed));
        let level = ((20.0 * (peak + 1e-9).log10() + 48.0) / 48.0).clamp(0.0, 1.0);
        st.meters[k] = level.max(st.meters[k] - dt * 1.2);
        meter(p, Rect::from_min_max(strip.right_top() + vec2(-18.0, 6.0), strip.right_bottom() - vec2(6.0, 6.0)), st.meters[k]);

        let inner = w - 22.0;
        let col = inner / 3.0;
        for (j, (param, label)) in [(&sp.low, "LOW"), (&sp.mid, "MID"), (&sp.high, "HIGH")].into_iter().enumerate() {
            knob(ui, p, setter, param, pos2(strip.left() + 2.0 + col * (j as f32 + 0.5), strip.top() + 62.0), 13.0, label, color, true);
        }
        knob(ui, p, setter, &sp.level, pos2(strip.left() + inner * 0.36, strip.top() + 138.0), 22.0, "LEVEL", color, false);
        knob(ui, p, setter, &sp.pan, pos2(strip.left() + inner * 0.84, strip.top() + 146.0), 12.0, "PAN", color, true);
        let bw = (inner - 16.0) / 2.0;
        let by = strip.bottom() - 30.0;
        switch(ui, p, setter, &sp.mute, Rect::from_min_size(pos2(strip.left() + 6.0, by), vec2(bw, 22.0)), "MUTE", RED);
        switch(ui, p, setter, &sp.solo, Rect::from_min_size(pos2(strip.left() + 10.0 + bw, by), vec2(bw, 22.0)), "SOLO", YELLOW);
    }
}

/// 10-band EQ with presets and Mega Bass, the filter pad and six effects.
fn rack_panel(ui: &mut egui::Ui, p: &egui::Painter, setter: &ParamSetter, params: &SwfParams, panel: Rect, body: Rect) {
    let rack = &params.rack;
    let effects: [(&FxParams, &str); 6] = [
        (&rack.crush, "BITCRUSH"),
        (&rack.wobble, "TAPE WOBBLE"),
        (&rack.ring, "RING MOD"),
        (&rack.gate, "TRANCE GATE"),
        (&rack.echo, "ECHO 3/16"),
        (&rack.space, "SPACE"),
    ];
    let reset = Rect::from_min_size(panel.right_top() + vec2(-120.0, 8.0), vec2(110.0, 24.0));
    if text_key(ui, p, reset, "RESET ALL", "reset-rack") {
        for band in &rack.eq {
            reset_param(setter, &band.gain);
        }
        for param in [&rack.mega, &rack.filter] {
            reset_param(setter, param);
        }
        reset_param(setter, &rack.cutoff);
        reset_param(setter, &rack.reso);
        for (fx, _) in effects {
            reset_param(setter, &fx.on);
            reset_param(setter, &fx.amount);
        }
    }
    p.text(reset.left_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, "EQ and effects work on the main output (the mix)", FontId::proportional(9.0), SILK_DIM);

    // EQ
    let eq = Rect::from_min_size(body.left_top(), vec2(390.0, body.height()));
    sub_panel(p, eq, "GRAPHIC EQUALIZER");
    let presets: [(&str, [f32; 10]); 3] = [
        ("FLAT", [0.0; 10]),
        ("SMILE", [9.0, 7.0, 4.0, 0.0, -3.0, -3.0, 0.0, 4.0, 7.0, 9.0]),
        ("PHONE", [-12.0, -12.0, -10.0, -2.0, 6.0, 9.0, 6.0, -6.0, -12.0, -12.0]),
    ];
    for (i, (name, gains)) in presets.iter().enumerate() {
        let rect = Rect::from_min_size(eq.right_top() + vec2(-56.0 - i as f32 * 52.0, 5.0), vec2(48.0, 16.0));
        if text_key(ui, p, rect, name, ("preset", i)) {
            for (band, g) in rack.eq.iter().zip(gains) {
                set_once(setter, &band.gain, band.gain.preview_normalized(*g));
            }
        }
    }
    // dB scale on the left
    let fw = (eq.width() - 46.0) / 10.0;
    let groove_top = eq.top() + 38.0;
    let groove_h = 110.0;
    for (db, t) in [("+12", 0.0), ("+6", 0.25), ("0", 0.5), ("-6", 0.75), ("-12", 1.0)] {
        let y = groove_top + groove_h * t;
        p.text(pos2(eq.left() + 30.0, y), Align2::RIGHT_CENTER, db, FontId::monospace(8.0), SILK_DIM);
        p.line_segment([pos2(eq.left() + 34.0, y), pos2(eq.right() - 8.0, y)], Stroke::new(1.0, Color32::from_white_alpha(if t == 0.5 { 28 } else { 10 })));
    }
    for (i, band) in rack.eq.iter().enumerate() {
        let x = eq.left() + 38.0 + (i as f32 + 0.5) * fw;
        fader(ui, p, setter, &band.gain, pos2(x, groove_top), groove_h, EQ_LABELS[i]);
    }
    let mega = Rect::from_min_size(eq.left_bottom() + vec2(10.0, -30.0), vec2(120.0, 22.0));
    switch(ui, p, setter, &rack.mega, mega, "MEGA BASS", AMBER);
    p.text(mega.right_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, "+12 dB low shelf @ 110 Hz", FontId::monospace(9.0), SILK_DIM);

    // Filter pad
    let fbox = Rect::from_min_size(eq.right_top() + vec2(8.0, 0.0), vec2(176.0, body.height()));
    sub_panel(p, fbox, "FILTER PAD");
    let pad = Rect::from_min_size(fbox.left_top() + vec2(10.0, 26.0), vec2(156.0, 130.0));
    filter_pad(ui, p, setter, params, pad);
    switch(ui, p, setter, &rack.filter, Rect::from_min_size(fbox.left_bottom() + vec2(10.0, -30.0), vec2(156.0, 22.0)), "FILTER", CYAN);

    // Effects
    let fx = Rect::from_min_max(fbox.right_top() + vec2(8.0, 0.0), body.right_bottom());
    sub_panel(p, fx, "EFFECTS");
    let a = |f: &FxParams| f.amount.value();
    let crush = a(&rack.crush);
    let readouts = [
        format!("{}-BIT/{}k", (12.0 - crush * 10.0).round().max(2.0), (48.0 / (1.0 + (crush * 24.0).floor())).round()),
        format!("{:.1} Hz", 0.6 + a(&rack.wobble) * 7.0),
        format!("{:.0} Hz", 30.0 + a(&rack.ring).powi(2) * 1400.0),
        format!("1/16 {:.0}%", a(&rack.gate) * 100.0),
        format!("FB {:.0}%", 20.0 + a(&rack.echo) * 62.0),
        format!("{:.0}% WET", a(&rack.space) * 100.0),
    ];
    let mw = (fx.width() - 16.0 - 2.0 * 6.0) / 3.0;
    let mh = (fx.height() - 30.0 - 6.0 - 6.0) / 2.0;
    for (i, ((f, name), readout)) in effects.into_iter().zip(readouts).enumerate() {
        let at = vec2(8.0 + (i % 3) as f32 * (mw + 6.0), 26.0 + (i / 3) as f32 * (mh + 6.0));
        fx_module(ui, p, setter, f, Rect::from_min_size(fx.left_top() + at, vec2(mw, mh)), name, &readout);
    }
}

/// The game's buttons. Press one with the mouse to play, the MIDI tag under it learns a
/// note or CC (right-click the tag to unbind).
#[allow(clippy::too_many_arguments)]
fn midi_panel(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    st: &mut UiState,
    params: &SwfParams,
    shared: &Shared,
    cmds: &crossbeam_channel::Sender<Cmd>,
    panel: Rect,
    body: Rect,
    held: u32,
    learning: Option<Target>,
    blink: bool,
) {
    let reset = Rect::from_min_size(panel.right_top() + vec2(-120.0, 8.0), vec2(110.0, 24.0));
    if text_key(ui, p, reset, "DEFAULT MAP", "reset-map") {
        *params.mapping.write().unwrap() = Mapping::default();
        *shared.learn.lock().unwrap() = None;
    }
    let hint = if learning.is_some() {
        "LEARNING: press a key or turn a knob on your controller (click the tag again to cancel)"
    } else {
        "press a button to play  •  click its MIDI tag to learn  •  right-click the tag to unbind"
    };
    p.text(reset.left_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, hint, FontId::proportional(9.0), if learning.is_some() { AMBER } else { SILK_DIM });

    let mapping = params.mapping.read().unwrap().clone();
    let cols = 7;
    let gap = 8.0;
    let bw = (body.width() - (cols - 1) as f32 * gap) / cols as f32;
    let bh = 84.0;
    for (i, t) in Target::ALL.iter().enumerate() {
        let (col, row) = (i % cols, i / cols);
        let rect = Rect::from_min_size(body.left_top() + vec2(col as f32 * (bw + gap), 4.0 + row as f32 * (bh + 12.0)), vec2(bw, bh));
        let key = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - 24.0));
        let tag = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 20.0), rect.max);

        // the button itself plays the game
        let resp = ui.interact(key, ui.id().with(("map-key", i)), Sense::click_and_drag());
        let down = resp.is_pointer_button_down_on();
        if t.is_axis() {
            if down && let Some(pos) = resp.interact_pointer_pos() {
                let v = ((pos.x - key.left()) / key.width()).clamp(0.0, 1.0);
                let _ = cmds.send(Cmd::MouseAxis(*t, v));
            }
        } else if down != st.mouse_held[i] {
            st.mouse_held[i] = down;
            let _ = cmds.send(Cmd::Button(*t, down));
        }
        let lit = held & (1 << t.index()) != 0 || (t.is_axis() && down);
        hw_key(p, key, down || lit, resp.hovered(), Some(lit), GREEN);
        let off = if down || lit { vec2(0.0, 1.5) } else { Vec2::ZERO };
        p.text(key.center() + vec2(6.0, -6.0) + off, Align2::CENTER_CENTER, t.label(), FontId::proportional(13.0), Color32::WHITE);
        let what = if t.is_axis() { "drag ←→" } else if *t == Target::Click { "game click" } else { "hold" };
        p.text(key.center() + vec2(6.0, 11.0) + off, Align2::CENTER_CENTER, what, FontId::proportional(8.0), SILK_DIM);

        // the MIDI tag learns
        let tresp = ui.interact(tag, ui.id().with(("map-tag", i)), Sense::click());
        let is_learning = learning == Some(*t);
        vfd_glass(p, tag);
        if tresp.hovered() {
            p.rect_stroke(tag, 3.0, Stroke::new(1.0, VFD.gamma_multiply(0.6)), StrokeKind::Inside);
        }
        let text = if is_learning {
            if blink { "LEARN…".to_string() } else { String::new() }
        } else {
            match mapping.source_of(*t) {
                Some(Source::Note(n)) => format!("MIDI {} #{n}", Source::Note(n).name()),
                Some(s) => format!("MIDI {}", s.name()),
                None => "MIDI --".to_string(),
            }
        };
        glow_text(p, tag.center(), Align2::CENTER_CENTER, &text, FontId::monospace(10.0), if is_learning { AMBER } else { VFD }, VFD.gamma_multiply(0.15));
        if tresp.clicked() {
            let mut l = shared.learn.lock().unwrap();
            *l = if *l == Some(*t) { None } else { Some(*t) };
        }
        if tresp.secondary_clicked() {
            params.mapping.write().unwrap().clear(*t);
        }
    }
    p.text(
        body.right_bottom() + vec2(-2.0, -6.0),
        Align2::RIGHT_CENTER,
        "note #60 is middle C (C4 here, C5 in FL Studio)  •  arm the track to record your playthrough as MIDI",
        FontId::proportional(9.0),
        SILK_DIM,
    );
}

/// Arrow keys, X, Z, Enter, Space, C and Shift in the editor window play too.
fn keyboard(ctx: &Context, st: &mut UiState, cmds: &crossbeam_channel::Sender<Cmd>) {
    ctx.input(|i| {
        for e in &i.events {
            if let egui::Event::Key { key, pressed, repeat: false, .. } = e {
                let t = match key {
                    egui::Key::ArrowUp => Target::Up,
                    egui::Key::ArrowDown => Target::Down,
                    egui::Key::ArrowLeft => Target::Left,
                    egui::Key::ArrowRight => Target::Right,
                    egui::Key::X => Target::A,
                    egui::Key::Z => Target::B,
                    egui::Key::Enter => Target::Start,
                    egui::Key::Space => Target::Space,
                    egui::Key::C => Target::KeyC,
                    _ => continue,
                };
                let _ = cmds.send(Cmd::Button(t, *pressed));
            }
        }
        if i.modifiers.shift != st.shift {
            st.shift = i.modifiers.shift;
            let _ = cmds.send(Cmd::Button(Target::Select, st.shift));
        }
    });
}

fn pick_swf(st: &UiState, params: &Arc<SwfParams>, cmds: &crossbeam_channel::Sender<Cmd>) {
    if st.picking.swap(true, Ordering::Relaxed) {
        return;
    }
    let (picking, params, cmds) = (st.picking.clone(), params.clone(), cmds.clone());
    // The dialog blocks, keep it off the GUI thread
    std::thread::spawn(move || {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Insert disc")
            .add_filter("Flash movie", &["swf"])
            .pick_file()
        {
            *params.swf_path.write().unwrap() = path.to_string_lossy().into_owned();
            let _ = cmds.send(Cmd::Load(path));
        }
        picking.store(false, Ordering::Relaxed);
    });
}

/// Band levels 0..1, log-spaced 40 Hz – 16 kHz (Goertzel per band, Hann window).
fn spectrum(samples: &[f32], rate: f32) -> [f32; BARS] {
    let n = samples.len();
    let mut out = [0.0; BARS];
    for (b, level) in out.iter_mut().enumerate() {
        let f = 40.0 * (16000.0f32 / 40.0).powf(b as f32 / (BARS - 1) as f32);
        let coeff = 2.0 * (TAU * f / rate).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for (i, x) in samples.iter().enumerate() {
            let w = 0.5 - 0.5 * (TAU * i as f32 / n as f32).cos();
            let s = x * w + coeff * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        let power = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0);
        let mag = power.sqrt() / (n as f32 * 0.25);
        let db = 20.0 * (mag + 1e-9).log10();
        *level = ((db + 54.0) / 54.0).clamp(0.0, 1.0);
    }
    out
}

/// No disc: a starfield and a pulsing "INSERT DISC".
fn attract(p: &egui::Painter, screen: Rect, time: f64, status: &str) {
    let sp = p.with_clip_rect(screen);
    let t = time as f32;
    for i in 0..80u32 {
        let h = i.wrapping_mul(2654435761);
        let fx = (h % 1000) as f32 / 1000.0;
        let fy = ((h >> 10) % 1000) as f32 / 1000.0;
        let speed = 0.02 + ((h >> 20) % 100) as f32 / 1500.0;
        let x = screen.left() + ((fx + t * speed) % 1.0) * screen.width();
        let y = screen.top() + fy * screen.height();
        sp.circle_filled(pos2(x, y), 0.6 + speed * 12.0, Color32::from_white_alpha((60.0 + speed * 2000.0).min(220.0) as u8));
    }
    let pulse = 0.55 + 0.45 * (t * 3.0).sin();
    glow_text(&sp, screen.center() - vec2(0.0, 12.0), Align2::CENTER_CENTER, "INSERT  DISC", FontId::proportional(36.0), VFD.gamma_multiply(pulse), VFD.gamma_multiply(0.14 * pulse));
    let sub = if status.is_empty() { "press  EJECT  to load a .swf" } else { status };
    sp.text(screen.center() + vec2(0.0, 28.0), Align2::CENTER_CENTER, sub, FontId::monospace(12.0), if status.starts_with("ERROR") { AMBER } else { Color32::from_gray(170) });
}

// ---------- controls ----------

/// Begins, sets and ends a parameter change in one go (clicks, presets, resets).
fn set_once<P: Param>(setter: &ParamSetter, param: &P, normalized: f32) {
    setter.begin_set_parameter(param);
    setter.set_parameter_normalized(param, normalized);
    setter.end_set_parameter(param);
}

fn reset_param<P: Param>(setter: &ParamSetter, param: &P) {
    if param.unmodulated_normalized_value() != param.default_normalized_value() {
        set_once(setter, param, param.default_normalized_value());
    }
}

/// Shared by knobs and faders: the wheel steps, a double-click resets.
fn wheel_and_reset<P: Param>(ui: &egui::Ui, setter: &ParamSetter, param: &P, resp: &egui::Response) {
    if resp.double_clicked() {
        set_once(setter, param, param.default_normalized_value());
    } else if resp.hovered() {
        let (scroll, fine) = ui.input(|i| (i.raw_scroll_delta.y, i.modifiers.shift));
        if scroll != 0.0 {
            let step = if fine { 0.005 } else { 0.03 } * scroll.signum();
            set_once(setter, param, (param.unmodulated_normalized_value() + step).clamp(0.0, 1.0));
        }
    }
}

/// Machined aluminium knob with a knurled skirt, a printed scale and an LED ring.
/// Drag up/down (Shift: fine), wheel, double-click resets. Bipolar knobs light the ring
/// from the middle.
#[allow(clippy::too_many_arguments)]
fn knob<P: Param>(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    setter: &ParamSetter,
    param: &P,
    c: Pos2,
    rad: f32,
    label: &str,
    color: Color32,
    bipolar: bool,
) {
    let rect = Rect::from_center_size(c, Vec2::splat(rad * 2.0 + 14.0));
    let resp = ui.interact(rect, ui.id().with(param.name()), Sense::click_and_drag());
    let v = param.unmodulated_normalized_value();
    if resp.drag_started() {
        setter.begin_set_parameter(param);
    }
    if resp.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        let d = -resp.drag_delta().y * if fine { 0.0015 } else { 0.006 };
        setter.set_parameter_normalized(param, (v + d).clamp(0.0, 1.0));
    }
    if resp.drag_stopped() {
        setter.end_set_parameter(param);
    }
    wheel_and_reset(ui, setter, param, &resp);
    let v = param.unmodulated_normalized_value();
    let hot = resp.hovered() || resp.dragged();

    // 0 = lower left, 1 = lower right, clockwise over the top
    let ang = |t: f32| (-225.0 + 270.0 * t).to_radians();
    let dir = |t: f32| vec2(ang(t).cos(), ang(t).sin());
    let ring = rad + 5.0;
    // printed scale
    let ticks = if rad >= 20.0 { 21 } else { 11 };
    for i in 0..ticks {
        let t = i as f32 / (ticks - 1) as f32;
        let major = i % 5 == 0 || ticks == 11;
        let len = if major { 4.0 } else { 2.0 };
        p.line_segment([c + dir(t) * (ring + 3.0), c + dir(t) * (ring + 3.0 + len)], Stroke::new(1.0, Color32::from_gray(if major { 150 } else { 95 })));
    }
    // LED ring: a dot per step, lit up to the value
    let dots = if rad >= 20.0 { 31 } else { 17 };
    let from = if bipolar { 0.5 } else { 0.0 };
    for i in 0..dots {
        let t = i as f32 / (dots - 1) as f32;
        let on = if v >= from { t >= from - 1e-4 && t <= v + 1e-4 } else { t <= from + 1e-4 && t >= v - 1e-4 };
        let on = on && (v - from).abs() > 0.004;
        let q = c + dir(t) * ring;
        if on {
            p.circle_filled(q, 3.2, color.gamma_multiply(0.25));
            p.circle_filled(q, 1.7, color);
        } else {
            p.circle_filled(q, 1.4, Color32::from_gray(28));
        }
    }

    // body: shadow, knurled skirt, machined cap with rings, gloss, pointer
    p.circle_filled(c + vec2(1.0, 4.0), rad + 2.0, Color32::from_black_alpha(160));
    grad_circle(p, c, rad, Color32::from_gray(if hot { 205 } else { 182 }), Color32::from_gray(38));
    let knurls = (rad * 2.2) as usize;
    for i in 0..knurls {
        let a = ang(v) + i as f32 / knurls as f32 * TAU;
        let d = vec2(a.cos(), a.sin());
        p.line_segment([c + d * rad * 0.78, c + d * rad * 0.98], Stroke::new(1.0, Color32::from_black_alpha(110)));
    }
    grad_circle(p, c, rad * 0.72, Color32::from_gray(if hot { 225 } else { 200 }), Color32::from_gray(92));
    for k in 1..4 {
        p.circle_stroke(c, rad * 0.72 * k as f32 / 4.0, Stroke::new(1.0, Color32::from_black_alpha(18)));
    }
    p.circle_stroke(c, rad * 0.72, Stroke::new(1.0, Color32::from_black_alpha(140)));
    let hl = Rect::from_center_size(c - vec2(rad * 0.15, rad * 0.32), vec2(rad * 0.9, rad * 0.45));
    grad_rrect(p, hl, rad * 0.22, Color32::from_white_alpha(120), Color32::from_white_alpha(0));
    let d = dir(v);
    p.line_segment([c + d * rad * 0.18, c + d * rad * 0.7], Stroke::new(3.5, Color32::from_black_alpha(190)));
    p.circle_filled(c + d * rad * 0.82, (rad * 0.11).max(1.8), color);

    let text = if hot { param.normalized_value_to_string(v, true) } else { label.to_string() };
    p.text(c + vec2(0.0, ring + 12.0), Align2::CENTER_CENTER, text, FontId::monospace(9.0), if hot { color } else { SILK });
}

/// Long-throw EQ fader with a chrome cap; `top` is the top of the groove.
fn fader(ui: &mut egui::Ui, p: &egui::Painter, setter: &ParamSetter, param: &FloatParam, top: Pos2, len: f32, label: &str) {
    let groove = Rect::from_min_size(top - vec2(2.5, 0.0), vec2(5.0, len));
    let resp = ui.interact(groove.expand2(vec2(12.0, 8.0)), ui.id().with(param.name()), Sense::click_and_drag());
    if resp.drag_started() {
        setter.begin_set_parameter(param);
    }
    if resp.dragged()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        setter.set_parameter_normalized(param, (1.0 - (pos.y - groove.top()) / groove.height()).clamp(0.0, 1.0));
    }
    if resp.drag_stopped() {
        setter.end_set_parameter(param);
    }
    wheel_and_reset(ui, setter, param, &resp);
    let v = param.unmodulated_normalized_value();
    let hot = resp.hovered() || resp.dragged();

    p.rect_filled(groove, 2.5, Color32::BLACK);
    p.rect_stroke(groove, 2.5, Stroke::new(1.0, Color32::from_white_alpha(30)), StrokeKind::Outside);
    let mid = groove.center().y;
    let y = groove.bottom() - v * groove.height();
    let lit = Rect::from_min_max(pos2(groove.left() + 1.0, y.min(mid)), pos2(groove.right() - 1.0, y.max(mid)));
    p.rect_filled(lit.expand(2.0), 2.0, AMBER.gamma_multiply(0.15));
    p.rect_filled(lit, 1.0, AMBER);
    let cap = Rect::from_center_size(pos2(groove.center().x, y), vec2(24.0, 14.0));
    p.rect_filled(cap.translate(vec2(1.0, 3.0)), 2.0, Color32::from_black_alpha(170));
    grad_rrect(p, cap, 2.0, Color32::from_gray(if hot { 245 } else { 225 }), Color32::from_gray(70));
    for k in [-3.0, 0.0, 3.0] {
        p.line_segment([cap.left_center() + vec2(3.0, k), cap.right_center() + vec2(-3.0, k)], Stroke::new(1.0, Color32::from_black_alpha(if k == 0.0 { 200 } else { 60 })));
    }
    let text = if hot { format!("{:+.1}", param.value()) } else { label.to_string() };
    p.text(pos2(groove.center().x, groove.bottom() + 14.0), Align2::CENTER_CENTER, text, FontId::monospace(9.0), if hot { AMBER } else { SILK });
}

/// Cutoff left to right, resonance bottom to top. Touching the pad turns the filter on.
fn filter_pad(ui: &mut egui::Ui, p: &egui::Painter, setter: &ParamSetter, params: &SwfParams, pad: Rect) {
    let rack = &params.rack;
    let on = rack.filter.value();
    vfd_glass(p, pad);
    let grid = Stroke::new(1.0, VFD.gamma_multiply(0.07));
    for i in 1..8 {
        let t = i as f32 / 8.0;
        p.line_segment([pos2(pad.left() + pad.width() * t, pad.top()), pos2(pad.left() + pad.width() * t, pad.bottom())], grid);
        p.line_segment([pos2(pad.left(), pad.top() + pad.height() * t), pos2(pad.right(), pad.top() + pad.height() * t)], grid);
    }

    let resp = ui.interact(pad, ui.id().with("filter-pad"), Sense::click_and_drag());
    let pressed = resp.drag_started() || resp.clicked();
    if pressed {
        if !on {
            set_once(setter, &rack.filter, 1.0);
        }
        setter.begin_set_parameter(&rack.cutoff);
        setter.begin_set_parameter(&rack.reso);
    }
    if (resp.dragged() || pressed)
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let rel = ((pos - pad.min) / pad.size()).clamp(Vec2::ZERO, Vec2::splat(1.0));
        setter.set_parameter_normalized(&rack.cutoff, rel.x);
        setter.set_parameter_normalized(&rack.reso, 1.0 - rel.y);
    }
    if resp.drag_stopped() || resp.clicked() {
        setter.end_set_parameter(&rack.cutoff);
        setter.end_set_parameter(&rack.reso);
    }

    let dot = pos2(
        pad.left() + rack.cutoff.unmodulated_normalized_value() * pad.width(),
        pad.bottom() - rack.reso.unmodulated_normalized_value() * pad.height(),
    );
    let c = if on { VFD } else { VFD_DIM.gamma_multiply(2.5) };
    let cp = p.with_clip_rect(pad);
    // the filter's response curve, drawn like an old analyzer
    let curve: Vec<Pos2> = (0..=48)
        .map(|i| {
            let x = i as f32 / 48.0;
            let d = x - (dot.x - pad.left()) / pad.width();
            let reso = rack.reso.unmodulated_normalized_value();
            let y = if d < 0.0 { 0.55 } else { 0.55 + d * 3.0 } - reso * 0.4 * (-(d * 14.0).powi(2)).exp();
            pos2(pad.left() + x * pad.width(), pad.top() + y.clamp(0.02, 1.2) * pad.height())
        })
        .collect();
    cp.add(Shape::line(curve, Stroke::new(1.5, c.gamma_multiply(0.7))));
    cp.line_segment([pos2(dot.x, pad.top()), pos2(dot.x, pad.bottom())], Stroke::new(1.0, c.gamma_multiply(0.25)));
    cp.line_segment([pos2(pad.left(), dot.y), pos2(pad.right(), dot.y)], Stroke::new(1.0, c.gamma_multiply(0.25)));
    cp.circle_filled(dot, 10.0, c.gamma_multiply(0.15));
    cp.circle_filled(dot, 5.0, c.gamma_multiply(0.7));
    cp.circle_filled(dot, 2.5, Color32::WHITE);
    p.text(pad.left_bottom() + vec2(5.0, -8.0), Align2::LEFT_CENTER, "CUTOFF →", FontId::monospace(8.0), VFD.gamma_multiply(0.5));
    p.text(pad.right_top() + vec2(-5.0, 9.0), Align2::RIGHT_CENTER, "↑ RESO", FontId::monospace(8.0), VFD.gamma_multiply(0.5));
    let readout = if on { format!("{}  Q {}", rack.cutoff, rack.reso) } else { "BYPASS".to_string() };
    p.text(pad.center_bottom() + vec2(0.0, 10.0), Align2::CENTER_CENTER, readout, FontId::monospace(9.0), if on { VFD } else { SILK_DIM });
}

/// One effect: a lit on switch with its name, a knob, a reset key when it was touched.
fn fx_module(ui: &mut egui::Ui, p: &egui::Painter, setter: &ParamSetter, fx: &FxParams, rect: Rect, name: &str, readout: &str) {
    let on = fx.on.value();
    grad_rrect(p, rect, 5.0, Color32::from_gray(if on { 48 } else { 36 }), Color32::from_gray(16));
    p.rect_stroke(rect, 5.0, Stroke::new(1.0, if on { AMBER.gamma_multiply(0.5) } else { Color32::BLACK }), StrokeKind::Inside);

    let head = Rect::from_min_size(rect.left_top() + vec2(5.0, 5.0), vec2(rect.width() - 10.0, 18.0));
    let touched = on || fx.amount.unmodulated_normalized_value() != fx.amount.default_normalized_value();
    let switch_rect = if touched { Rect::from_min_max(head.min, head.max - vec2(24.0, 0.0)) } else { head };
    switch(ui, p, setter, &fx.on, switch_rect, name, AMBER);
    if touched {
        let rst = Rect::from_min_max(pos2(head.right() - 20.0, head.top()), head.max);
        if text_key(ui, p, rst, "↺", ("fx-reset", fx.on.name())) {
            reset_param(setter, &fx.on);
            reset_param(setter, &fx.amount);
        }
    }
    let c = pos2(rect.center().x, rect.top() + 26.0 + (rect.height() - 44.0) / 2.0);
    let before = fx.amount.unmodulated_normalized_value();
    knob(ui, p, setter, &fx.amount, c, 15.0, readout, if on { AMBER } else { SILK_DIM }, false);
    // Turning the knob of an effect that is off switches it on
    if !on && fx.amount.unmodulated_normalized_value() != before {
        set_once(setter, &fx.on, 1.0);
    }
}

/// A latching push button with a lamp in it, bound to a switch parameter.
fn switch(ui: &mut egui::Ui, p: &egui::Painter, setter: &ParamSetter, param: &BoolParam, rect: Rect, label: &str, lamp_color: Color32) {
    let resp = ui.interact(rect, ui.id().with(param.name()), Sense::click());
    let on = param.value();
    let down = resp.is_pointer_button_down_on();
    hw_key(p, rect, down || on, resp.hovered(), Some(on), lamp_color);
    let off = if down || on { vec2(0.0, 1.0) } else { Vec2::ZERO };
    p.text(rect.center() + vec2(6.0, 0.0) + off, Align2::CENTER_CENTER, label, FontId::proportional(9.0), if on { Color32::WHITE } else { SILK_DIM });
    if resp.clicked() {
        set_once(setter, param, if on { 0.0 } else { 1.0 });
    }
}

/// A momentary key with a text label. Returns whether it was clicked.
fn text_key(ui: &mut egui::Ui, p: &egui::Painter, rect: Rect, label: &str, id: impl std::hash::Hash) -> bool {
    let resp = ui.interact(rect, ui.id().with(id), Sense::click());
    let down = resp.is_pointer_button_down_on();
    hw_key(p, rect, down, resp.hovered(), None, GREEN);
    p.text(rect.center() + if down { vec2(0.0, 1.0) } else { Vec2::ZERO }, Align2::CENTER_CENTER, label, FontId::proportional(9.0), SILK);
    resp.clicked()
}

// ---------- hardware drawing ----------

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

fn round_points(rect: Rect, r: f32) -> Vec<Pos2> {
    let r = r.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (pos2(rect.left() + r, rect.top() + r), 180.0f32),
        (pos2(rect.right() - r, rect.top() + r), 270.0),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), 90.0),
    ];
    let mut pts = Vec::with_capacity(32);
    for (c, start) in corners {
        for s in 0..=6 {
            let a = (start + s as f32 * 15.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * r);
        }
    }
    pts
}

/// Convex shape filled with a vertical gradient.
fn grad_fill(p: &egui::Painter, pts: &[Pos2], rect: Rect, top: Color32, bot: Color32) {
    let at = |y: f32| mix(top, bot, (y - rect.top()) / rect.height().max(1.0));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), at(rect.center().y));
    for q in pts {
        mesh.colored_vertex(*q, at(q.y));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(mesh));
}

fn grad_rrect(p: &egui::Painter, rect: Rect, r: f32, top: Color32, bot: Color32) {
    grad_fill(p, &round_points(rect, r), rect, top, bot);
}

fn grad_circle(p: &egui::Painter, c: Pos2, rad: f32, top: Color32, bot: Color32) {
    let pts: Vec<Pos2> = (0..40)
        .map(|i| {
            let a = i as f32 / 40.0 * TAU;
            c + vec2(a.cos(), a.sin()) * rad
        })
        .collect();
    grad_fill(p, &pts, Rect::from_center_size(c, Vec2::splat(rad * 2.0)), top, bot);
}

/// Walnut cheek with grain.
fn wood(p: &egui::Painter, rect: Rect) {
    p.rect_filled(rect, 0.0, Color32::from_rgb(0x5b, 0x2e, 0x16));
    let mut x = rect.left();
    let mut i = 0u32;
    while x < rect.right() {
        let h = i.wrapping_mul(2654435761);
        let shade = 20 + (h % 40) as u8;
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x2a, 0x12, 0x06, shade + 40)));
        // knots and waves
        let mut y = rect.top() + (h % 90) as f32;
        while y < rect.bottom() {
            p.line_segment([pos2(x, y), pos2(x + 1.0, y + 18.0)], Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xa0, 0x5c, 0x2c, 40)));
            y += 70.0 + (h % 50) as f32;
        }
        x += 2.0 + (h % 3) as f32;
        i += 1;
    }
    p.rect_filled(Rect::from_min_max(rect.min, pos2(rect.left() + 2.0, rect.bottom())), 0.0, Color32::from_white_alpha(20));
}

/// Brushed aluminium: a vertical gradient with fine horizontal hairlines.
fn brushed(p: &egui::Painter, rect: Rect) {
    grad_fill(p, &[rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()], rect, ALU_TOP, ALU_BOT);
    let mut y = rect.top();
    let mut i = 0u32;
    while y < rect.bottom() {
        let h = i.wrapping_mul(2246822519);
        let a = (h % 22) as u8;
        let c = if h & 1 == 0 { Color32::from_white_alpha(a) } else { Color32::from_black_alpha(a / 2) };
        p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.0, c));
        y += 1.0 + (h % 2) as f32;
        i += 1;
    }
}

/// Phillips screw head.
fn screw(p: &egui::Painter, c: Pos2, rad: f32, rot: f32) {
    p.circle_filled(c + vec2(0.0, 1.0), rad + 1.0, Color32::from_black_alpha(120));
    grad_circle(p, c, rad, Color32::from_gray(235), Color32::from_gray(90));
    let a = rot;
    for k in [0.0, std::f32::consts::FRAC_PI_2] {
        let d = vec2((a + k).cos(), (a + k).sin()) * rad * 0.7;
        p.line_segment([c - d, c + d], Stroke::new(1.6, Color32::from_gray(55)));
    }
}

/// Text stamped into the aluminium.
fn engrave(p: &egui::Painter, pos: Pos2, align: Align2, text: &str, font: FontId) {
    p.text(pos + vec2(0.0, 1.0), align, text, font.clone(), Color32::from_white_alpha(150));
    p.text(pos, align, text, font, INK);
}

/// The logo: chrome letters with a bevel.
fn chrome_logo(p: &egui::Painter, pos: Pos2, text: &str) {
    let font = FontId::proportional(34.0);
    p.text(pos + vec2(2.0, 3.0), Align2::LEFT_CENTER, text, font.clone(), Color32::from_black_alpha(110));
    p.text(pos + vec2(0.0, 1.5), Align2::LEFT_CENTER, text, font.clone(), Color32::from_gray(40));
    p.text(pos - vec2(0.0, 1.0), Align2::LEFT_CENTER, text, font.clone(), Color32::WHITE);
    let galley = p.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE);
    let rect = Align2::LEFT_CENTER.anchor_size(pos, galley.size());
    // a stripe of reflection through the letters
    let cp = p.with_clip_rect(Rect::from_min_max(pos2(rect.left(), rect.center().y - 1.0), pos2(rect.right(), rect.bottom())));
    cp.text(pos, Align2::LEFT_CENTER, text, font.clone(), Color32::from_gray(120));
    let cp = p.with_clip_rect(Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(rect.right(), rect.center().y - 1.0)));
    cp.text(pos, Align2::LEFT_CENTER, text, font, Color32::from_gray(232));
}

/// Black anodized section, sunk into the faceplate, with an optional silkscreen title.
fn section(p: &egui::Painter, rect: Rect, title: Option<&str>) {
    p.rect_filled(rect.expand(2.0), 7.0, Color32::from_white_alpha(110));
    p.rect_filled(rect.expand(1.0).translate(vec2(0.0, -1.0)), 7.0, Color32::from_black_alpha(120));
    grad_rrect(p, rect, 6.0, Color32::from_rgb(0x1c, 0x1d, 0x21), Color32::from_rgb(0x0b, 0x0c, 0x0e));
    if let Some(t) = title {
        silk_title(p, rect.left_top() + vec2(10.0, 13.0), t, rect.width() - 20.0);
    }
}

/// A darker group inside a section, with a silkscreen title.
fn sub_panel(p: &egui::Painter, rect: Rect, title: &str) {
    grad_rrect(p, rect, 5.0, Color32::from_rgb(0x15, 0x16, 0x19), Color32::from_rgb(0x0a, 0x0a, 0x0c));
    p.rect_stroke(rect, 5.0, Stroke::new(1.0, Color32::from_white_alpha(14)), StrokeKind::Inside);
    silk_title(p, rect.left_top() + vec2(10.0, 13.0), title, rect.width() * 0.45);
}

/// "── TITLE ──────" in white silkscreen.
fn silk_title(p: &egui::Painter, pos: Pos2, title: &str, width: f32) {
    let r = p.text(pos, Align2::LEFT_CENTER, title, FontId::proportional(9.0), SILK);
    if width > r.width() + 20.0 {
        p.line_segment([pos2(r.right() + 6.0, pos.y), pos2(pos.x + width, pos.y)], Stroke::new(1.0, Color32::from_white_alpha(40)));
    }
}

/// Smoked VFD glass.
fn vfd_glass(p: &egui::Painter, rect: Rect) {
    p.rect_filled(rect, 3.0, VFD_BG);
    grad_rrect(p, Rect::from_min_max(rect.min, pos2(rect.right(), rect.center().y)), 3.0, Color32::from_white_alpha(12), Color32::from_white_alpha(0));
    p.rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_black_alpha(220)), StrokeKind::Inside);
}

fn vfd_flag(p: &egui::Painter, pos: Pos2, text: &str, on: bool) {
    if on {
        glow_text(p, pos, Align2::LEFT_CENTER, text, FontId::monospace(10.0), VFD, VFD.gamma_multiply(0.2));
    } else {
        p.text(pos, Align2::LEFT_CENTER, text, FontId::monospace(10.0), VFD_DIM);
    }
}

/// Chunky plastic key, with an optional lamp on its left.
fn hw_key(p: &egui::Painter, rect: Rect, down: bool, hover: bool, lamp: Option<bool>, lamp_color: Color32) {
    p.rect_filled(rect.expand(1.5).translate(vec2(0.0, 1.0)), 4.0, Color32::from_black_alpha(220));
    let face = if down { rect.translate(vec2(0.0, 1.5)) } else { rect };
    let (top, bot) = if down {
        (Color32::from_gray(40), Color32::from_gray(58))
    } else if hover {
        (Color32::from_gray(96), Color32::from_gray(40))
    } else {
        (Color32::from_gray(80), Color32::from_gray(32))
    };
    grad_rrect(p, face, 3.0, top, bot);
    if !down {
        p.line_segment([face.left_top() + vec2(3.0, 1.0), face.right_top() + vec2(-3.0, 1.0)], Stroke::new(1.0, Color32::from_white_alpha(70)));
    }
    p.rect_stroke(face, 3.0, Stroke::new(1.0, Color32::from_black_alpha(200)), StrokeKind::Inside);
    if let Some(on) = lamp {
        let c = pos2(face.left() + 9.0, face.center().y);
        let lens = Rect::from_center_size(c, vec2(6.0, (face.height() - 10.0).clamp(4.0, 10.0)));
        if on {
            p.rect_filled(lens.expand(3.0), 3.0, lamp_color.gamma_multiply(0.22));
            p.rect_filled(lens, 1.5, lamp_color);
            p.rect_filled(Rect::from_min_size(lens.min + vec2(1.0, 1.0), vec2(2.0, lens.height() * 0.4)), 1.0, Color32::from_white_alpha(160));
        } else {
            p.rect_filled(lens, 1.5, mix(lamp_color, Color32::BLACK, 0.82));
        }
    }
}

/// A panel lamp with a label under it.
fn lamp(p: &egui::Painter, c: Pos2, label: &str, on: bool, color: Color32) {
    p.circle_filled(c - vec2(0.0, 5.0), 5.0, Color32::from_gray(10));
    if on {
        p.circle_filled(c - vec2(0.0, 5.0), 8.0, color.gamma_multiply(0.18));
        grad_circle(p, c - vec2(0.0, 5.0), 3.8, mix(color, Color32::WHITE, 0.4), color);
    } else {
        p.circle_filled(c - vec2(0.0, 5.0), 3.8, mix(color, Color32::BLACK, 0.8));
    }
    p.text(c + vec2(0.0, 8.0), Align2::CENTER_CENTER, label, FontId::proportional(7.5), SILK_DIM);
}

/// Needle VU meter on a backlit cream face; `value` 0..1 over -20..+3 VU.
fn vu_meter(p: &egui::Painter, rect: Rect, value: f32, label: &str, clip: bool) {
    p.rect_filled(rect, 4.0, Color32::from_gray(8));
    let face = rect.shrink(3.0);
    grad_rrect(p, face, 3.0, Color32::from_rgb(0xff, 0xf2, 0xc8), Color32::from_rgb(0xe0, 0xc8, 0x8a));
    let pivot = pos2(face.center().x, face.bottom() + face.height() * 0.45);
    let rad = face.height() * 1.15;
    let ang = |t: f32| (-90.0f32 - 42.0 + 84.0 * t).to_radians();
    let at = |t: f32, r: f32| pivot + vec2(ang(t).cos(), ang(t).sin()) * r;
    let fp = p.with_clip_rect(face);
    let t0 = 20.0 / 23.0; // 0 VU
    let arc = |a: f32, b: f32| -> Vec<Pos2> { (0..=24).map(|i| at(a + (b - a) * i as f32 / 24.0, rad * 0.86)).collect() };
    fp.add(Shape::line(arc(0.0, t0), Stroke::new(1.5, Color32::from_gray(30))));
    fp.add(Shape::line(arc(t0, 1.0), Stroke::new(4.0, Color32::from_rgb(0xd0, 0x1e, 0x1e))));
    for (db, txt) in [(-20.0, "20"), (-10.0, "10"), (-7.0, "7"), (-5.0, "5"), (-3.0, "3"), (-1.0, "1"), (0.0, "0"), (1.0, "1"), (3.0, "3")] {
        let t = (db + 20.0) / 23.0;
        let red = db > 0.0;
        let col = if red { Color32::from_rgb(0xc0, 0x10, 0x10) } else { Color32::from_gray(30) };
        fp.line_segment([at(t, rad * 0.86), at(t, rad * 0.96)], Stroke::new(1.2, col));
        fp.text(at(t, rad * 1.04), Align2::CENTER_CENTER, txt, FontId::proportional(8.0), col);
    }
    fp.text(pos2(face.center().x, face.bottom() - 16.0), Align2::CENTER_CENTER, "VU", FontId::proportional(12.0), Color32::from_gray(40));
    fp.text(face.left_bottom() + vec2(8.0, -8.0), Align2::LEFT_CENTER, label, FontId::proportional(9.0), Color32::from_gray(60));
    // needle
    let t = value.clamp(-0.02, 1.04);
    fp.line_segment([pivot + vec2(1.5, 1.5), at(t, rad * 0.97) + vec2(1.5, 1.5)], Stroke::new(1.5, Color32::from_black_alpha(50)));
    fp.line_segment([pivot, at(t, rad * 0.97)], Stroke::new(1.4, Color32::from_gray(15)));
    // glass glare
    grad_rrect(p, Rect::from_min_max(face.min, pos2(face.right(), face.top() + face.height() * 0.4)), 3.0, Color32::from_white_alpha(70), Color32::from_white_alpha(0));
    // peak lamp
    let c = face.right_bottom() + vec2(-10.0, -9.0);
    if clip {
        p.circle_filled(c, 7.0, RED.gamma_multiply(0.3));
        p.circle_filled(c, 3.5, RED);
    } else {
        p.circle_filled(c, 3.5, Color32::from_rgb(0x50, 0x10, 0x10));
    }
    p.text(c - vec2(8.0, 0.0), Align2::RIGHT_CENTER, "PEAK", FontId::proportional(7.0), Color32::from_gray(60));
}

/// Vertical LED ladder, `level` 0..1 (-48..0 dB).
fn meter(p: &egui::Painter, rect: Rect, level: f32) {
    p.rect_filled(rect, 2.0, Color32::from_rgb(3, 3, 4));
    let segs = 24;
    let h = (rect.height() - 4.0) / segs as f32;
    let lit = (level * segs as f32).round() as usize;
    for s in 0..segs {
        let y1 = rect.bottom() - 2.0 - s as f32 * h;
        let seg = Rect::from_min_max(pos2(rect.left() + 2.0, y1 - h + 1.5), pos2(rect.right() - 2.0, y1));
        let t = s as f32 / segs as f32;
        let c = if t > 0.85 { RED } else if t > 0.65 { YELLOW } else { GREEN };
        if s < lit {
            p.rect_filled(seg.expand(1.0), 1.0, c.gamma_multiply(0.2));
            p.rect_filled(seg, 0.5, c);
        } else {
            p.rect_filled(seg, 0.5, c.gamma_multiply(0.07));
        }
    }
}

fn glow_text(p: &egui::Painter, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32, glow: Color32) {
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0), vec2(0.0, 2.0)] {
        p.text(pos + d, align, text, font.clone(), glow);
    }
    p.text(pos, align, text, font, color);
}

fn icon_play(p: &egui::Painter, c: Pos2, col: Color32) {
    p.add(Shape::convex_polygon(vec![c + vec2(-5.0, -8.0), c + vec2(9.0, 0.0), c + vec2(-5.0, 8.0)], col, Stroke::NONE));
}

fn icon_pause(p: &egui::Painter, c: Pos2, col: Color32) {
    p.rect_filled(Rect::from_center_size(c - vec2(4.5, 0.0), vec2(4.5, 15.0)), 1.0, col);
    p.rect_filled(Rect::from_center_size(c + vec2(4.5, 0.0), vec2(4.5, 15.0)), 1.0, col);
}

fn icon_stop(p: &egui::Painter, c: Pos2, col: Color32) {
    p.rect_filled(Rect::from_center_size(c, vec2(13.0, 13.0)), 1.5, col);
}

fn icon_eject(p: &egui::Painter, c: Pos2, col: Color32) {
    p.add(Shape::convex_polygon(vec![c + vec2(0.0, -9.0), c + vec2(9.0, 1.0), c + vec2(-9.0, 1.0)], col, Stroke::NONE));
    p.rect_filled(Rect::from_min_max(c + vec2(-9.0, 4.0), c + vec2(9.0, 8.0)), 1.0, col);
}

fn icon_reset(p: &egui::Painter, c: Pos2, col: Color32) {
    let pts: Vec<Pos2> = (0..=24)
        .map(|i| {
            let a = (40.0 + i as f32 * 12.0f32).to_radians();
            c + vec2(a.cos(), a.sin()) * 8.0
        })
        .collect();
    let end = *pts.last().unwrap();
    p.add(Shape::line(pts, Stroke::new(2.5, col)));
    p.add(Shape::convex_polygon(vec![end + vec2(-1.0, -6.0), end + vec2(6.0, 2.0), end + vec2(-4.0, 4.0)], col, Stroke::NONE));
}
