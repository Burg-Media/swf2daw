//! The editor: a 2010 desktop media player skin, all drawn by hand. Glossy gunmetal
//! body, a CRT-ish screen, a green LCD with a scrolling ticker, an LED spectrum
//! analyzer, round chrome buttons and a MIDI mapper panel.

use crate::engine::{Cmd, Shared};
use crate::mapping::Target;
use crate::{SCOPE_LEN, SwfParams};
use nih_plug::prelude::{Editor, Param, ParamSetter};
use nih_plug_egui::egui::{
    self, Align2, Color32, Context, FontId, Mesh, Pos2, Rect, Sense, Shape, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Vec2, pos2, vec2,
};
use nih_plug_egui::create_egui_editor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const WIDTH: u32 = 820;
pub const HEIGHT: u32 = 580;

const BARS: usize = 24;

// Palette
const BODY_TOP: Color32 = Color32::from_rgb(0x48, 0x4f, 0x5e);
const BODY_BOT: Color32 = Color32::from_rgb(0x15, 0x17, 0x1d);
const RIM: Color32 = Color32::from_rgb(0x7d, 0x87, 0x9c);
const CYAN: Color32 = Color32::from_rgb(0x39, 0xd5, 0xff);
const LCD_ON: Color32 = Color32::from_rgb(0xb8, 0xff, 0x6a);
const LCD_DIM: Color32 = Color32::from_rgb(0x2e, 0x55, 0x22);
const LCD_TOP: Color32 = Color32::from_rgb(0x10, 0x2a, 0x18);
const LCD_BOT: Color32 = Color32::from_rgb(0x05, 0x10, 0x08);
const ORANGE: Color32 = Color32::from_rgb(0xff, 0x9a, 0x1f);

struct UiState {
    tex: Option<TextureHandle>,
    frame_serial: u64,
    scope_serial: u64,
    bars: [f32; BARS],
    peaks: [f32; BARS],
    last_time: f64,
    screen_down: bool,
    shift: bool,
    picking: Arc<AtomicBool>,
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
        bars: [0.0; BARS],
        peaks: [0.0; BARS],
        last_time: 0.0,
        screen_down: false,
        shift: false,
        picking: Arc::new(AtomicBool::new(false)),
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

    // ---------- body ----------
    let body = r(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
    p.rect_filled(body, 0.0, Color32::from_rgb(4, 5, 7));
    let shell = body.shrink(3.0);
    grad_rrect(&p, shell, 16.0, BODY_TOP, BODY_BOT);
    // brushed metal
    let mut y = shell.top() + 2.0;
    while y < shell.bottom() - 2.0 {
        p.line_segment(
            [pos2(shell.left() + 10.0, y), pos2(shell.right() - 10.0, y)],
            Stroke::new(1.0, Color32::from_white_alpha(5)),
        );
        y += 3.0;
    }
    // glass sheen over the top
    grad_rrect(
        &p,
        Rect::from_min_max(shell.min, pos2(shell.right(), shell.top() + 46.0)),
        16.0,
        Color32::from_white_alpha(40),
        Color32::from_white_alpha(4),
    );
    p.rect_stroke(shell, 16.0, Stroke::new(1.0, RIM), StrokeKind::Inside);

    // ---------- title bar ----------
    for (i, c) in [
        Color32::from_rgb(0xff, 0x4d, 0x5e),
        Color32::from_rgb(0xff, 0xc8, 0x3d),
        Color32::from_rgb(0x5d, 0xe0, 0x6a),
    ]
    .into_iter()
    .enumerate()
    {
        orb(&p, pos2(26.0 + i as f32 * 20.0, 25.0) + o, 7.0, c);
    }
    glow_text(&p, pos2(92.0, 25.0) + o, Align2::LEFT_CENTER, "swf2DAW", FontId::proportional(24.0), Color32::WHITE, CYAN.gamma_multiply(0.35));
    p.text(pos2(196.0, 29.0) + o, Align2::LEFT_CENTER, "by BURG MEDIA", FontId::proportional(11.0), Color32::from_gray(150));
    p.text(
        pos2(WIDTH as f32 - 24.0, 25.0) + o,
        Align2::RIGHT_CENTER,
        concat!("v", env!("CARGO_PKG_VERSION"), "  •  RUFFLE INSIDE  •  FLASH IS NOT DEAD"),
        FontId::proportional(10.0),
        Color32::from_gray(120),
    );

    // ---------- screen ----------
    let bezel = r(16.0, 50.0, 496.0, 410.0);
    grad_rrect(&p, bezel, 12.0, Color32::from_gray(26), Color32::from_gray(60));
    p.rect_stroke(bezel, 12.0, Stroke::new(1.0, Color32::from_gray(90)), StrokeKind::Inside);
    let screen = bezel.shrink(12.0);
    p.rect_filled(screen, 4.0, Color32::BLACK);
    let mut game_rect = screen;

    {
        let frame = shared.frame.lock().unwrap();
        if frame.width > 0 && frame.serial != st.frame_serial {
            st.frame_serial = frame.serial;
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width as usize, frame.height as usize],
                &frame.rgba,
            );
            match &mut st.tex {
                Some(t) => t.set(image, TextureOptions::NEAREST),
                None => st.tex = Some(ctx.load_texture("swf", image, TextureOptions::NEAREST)),
            }
        }
        if frame.width == 0 {
            st.tex = None;
        } else {
            let aspect = frame.width as f32 / frame.height as f32;
            let size = if screen.width() / screen.height() > aspect {
                vec2(screen.height() * aspect, screen.height())
            } else {
                vec2(screen.width(), screen.width() / aspect)
            };
            game_rect = Rect::from_center_size(screen.center(), size);
        }
    }

    if let Some(tex) = &st.tex {
        p.image(tex.id(), game_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        attract(&p, screen, time, &status);
    }
    // scanlines + glass reflection
    let sp = p.with_clip_rect(screen);
    let mut y = screen.top();
    while y < screen.bottom() {
        sp.line_segment([pos2(screen.left(), y), pos2(screen.right(), y)], Stroke::new(1.0, Color32::from_black_alpha(40)));
        y += 3.0;
    }
    let mut glass = Mesh::default();
    glass.colored_vertex(screen.left_top(), Color32::from_white_alpha(22));
    glass.colored_vertex(pos2(screen.left() + screen.width() * 0.62, screen.top()), Color32::from_white_alpha(6));
    glass.colored_vertex(pos2(screen.left(), screen.top() + screen.height() * 0.55), Color32::from_white_alpha(0));
    glass.add_triangle(0, 1, 2);
    sp.add(Shape::mesh(glass));
    if loaded && !status.is_empty() {
        sp.text(screen.center_bottom() - vec2(0.0, 14.0), Align2::CENTER_CENTER, &status, FontId::monospace(12.0), ORANGE);
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

    // ---------- LCD ----------
    let lcd = r(508.0, 50.0, 804.0, 150.0);
    inset(&p, lcd, 8.0);
    let glass_lcd = lcd.shrink(6.0);
    grad_rrect(&p, glass_lcd, 5.0, LCD_TOP, LCD_BOT);
    // pixel grid
    let lp = p.with_clip_rect(glass_lcd);
    let mut x = glass_lcd.left();
    while x < glass_lcd.right() {
        lp.line_segment([pos2(x, glass_lcd.top()), pos2(x, glass_lcd.bottom())], Stroke::new(1.0, Color32::from_black_alpha(35)));
        x += 3.0;
    }
    let rate = shared.sample_rate.load(Ordering::Relaxed) as f32 / 1000.0;
    let fps = shared.fps.load(Ordering::Relaxed);
    let ticker = if loaded {
        format!(
            "♫ NOW PLAYING: {}  ★  {} FPS  ★  {:.1} kHz 32-BIT FLOAT  ★  {}  ★  ",
            title.to_uppercase(),
            fps,
            rate,
            if gb_mode { "GAME BOY MODE" } else { "FLASH MODE" }
        )
    } else {
        "★ NO DISC  ★  PRESS EJECT TO LOAD A .SWF  ★  WELCOME TO THE FUTURE OF 2010  ".to_string()
    };
    let font = FontId::monospace(13.0);
    let tw = p.layout_no_wrap(ticker.clone(), font.clone(), LCD_ON).size().x.max(1.0);
    let scroll = (time as f32 * 40.0) % tw;
    let mut tx = glass_lcd.left() + 8.0 - scroll;
    while tx < glass_lcd.right() {
        glow_text(&lp, pos2(tx, glass_lcd.top() + 16.0), Align2::LEFT_CENTER, &ticker, font.clone(), LCD_ON, LCD_ON.gamma_multiply(0.15));
        tx += tw;
    }
    let secs = shared.loaded_at.lock().unwrap().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    // ghost segments behind the time, like a real LCD
    lp.text(pos2(glass_lcd.left() + 10.0, glass_lcd.top() + 52.0), Align2::LEFT_CENTER, "88:88", FontId::monospace(34.0), LCD_DIM.gamma_multiply(0.5));
    glow_text(
        &lp,
        pos2(glass_lcd.left() + 10.0, glass_lcd.top() + 52.0),
        Align2::LEFT_CENTER,
        &format!("{:02}:{:02}", (secs / 60) % 100, secs % 60),
        FontId::monospace(34.0),
        LCD_ON,
        LCD_ON.gamma_multiply(0.2),
    );
    let mode = if !loaded { "STOP" } else if paused { "PAUSE" } else { "PLAY" };
    let last_midi = shared.last_midi.lock().unwrap().clone();
    let small = FontId::monospace(10.0);
    let right_x = glass_lcd.right() - 8.0;
    lcd_flag(&lp, pos2(right_x, glass_lcd.top() + 38.0), "STEREO", loaded);
    lcd_flag(&lp, pos2(right_x, glass_lcd.top() + 52.0), &format!("{rate:.1}kHz"), loaded);
    lcd_flag(&lp, pos2(right_x, glass_lcd.top() + 66.0), mode, loaded && (!paused || blink));
    lp.text(
        pos2(glass_lcd.left() + 128.0, glass_lcd.bottom() - 10.0),
        Align2::LEFT_CENTER,
        format!("MIDI {}", if last_midi.is_empty() { "--" } else { &last_midi }),
        small,
        LCD_ON.gamma_multiply(0.8),
    );

    // ---------- spectrum ----------
    let spec = r(508.0, 158.0, 804.0, 268.0);
    inset(&p, spec, 8.0);
    let area = spec.shrink(10.0);
    p.rect_filled(area, 3.0, Color32::from_rgb(4, 6, 8));
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
    let seg_h = 4.0;
    let segs = (area.height() / seg_h) as usize;
    let bw = area.width() / BARS as f32;
    for b in 0..BARS {
        st.bars[b] = levels[b].max(st.bars[b] - dt * 1.6);
        st.peaks[b] = st.bars[b].max(st.peaks[b] - dt * 0.35);
        let x0 = area.left() + b as f32 * bw + 1.5;
        let lit = (st.bars[b] * segs as f32) as usize;
        for s in 0..segs {
            let y1 = area.bottom() - s as f32 * seg_h - 1.0;
            let seg = Rect::from_min_max(pos2(x0, y1 - seg_h + 1.5), pos2(x0 + bw - 3.0, y1));
            let t = s as f32 / segs as f32;
            let c = if t > 0.85 {
                Color32::from_rgb(0xff, 0x3b, 0x3b)
            } else if t > 0.6 {
                Color32::from_rgb(0xff, 0xd8, 0x3b)
            } else {
                Color32::from_rgb(0x5d, 0xf0, 0x6a)
            };
            p.rect_filled(seg, 0.5, if s < lit { c } else { c.gamma_multiply(0.07) });
        }
        let py = area.bottom() - st.peaks[b] * segs as f32 * seg_h - 1.0;
        p.rect_filled(Rect::from_min_max(pos2(x0, py - 2.0), pos2(x0 + bw - 3.0, py)), 0.5, Color32::from_gray(230));
    }

    // ---------- transport ----------
    let names = ["EJECT", if paused { "PLAY" } else { "PAUSE" }, "STOP", "RESET"];
    for (i, name) in names.iter().enumerate() {
        let c = pos2(540.0 + i as f32 * 77.0, 302.0) + o;
        let rect = Rect::from_center_size(c, Vec2::splat(52.0));
        let resp = ui.interact(rect, ui.id().with(("transport", i)), Sense::click());
        chrome_button(&p, c, 25.0, resp.is_pointer_button_down_on(), resp.hovered());
        let ic = if resp.hovered() { CYAN } else { Color32::from_gray(225) };
        match i {
            0 => icon_eject(&p, c, ic),
            1 if paused || !loaded => icon_play(&p, c, ic),
            1 => icon_pause(&p, c, ic),
            2 => icon_stop(&p, c, ic),
            _ => icon_reset(&p, c, ic),
        }
        p.text(c + vec2(0.0, 34.0), Align2::CENTER_CENTER, *name, FontId::proportional(9.0), Color32::from_gray(140));
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

    // ---------- volume ----------
    let vol_row = r(508.0, 352.0, 804.0, 378.0);
    p.text(vol_row.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, "VOL", FontId::proportional(11.0), Color32::from_gray(170));
    let groove = Rect::from_min_max(vol_row.left_center() + vec2(40.0, -3.0), vol_row.right_center() + vec2(-60.0, 3.0));
    p.rect_filled(groove, 3.0, Color32::from_gray(8));
    p.rect_stroke(groove, 3.0, Stroke::new(1.0, Color32::from_gray(70)), StrokeKind::Outside);
    let v = params.volume.unmodulated_normalized_value();
    let fill = Rect::from_min_max(groove.min, pos2(groove.left() + groove.width() * v, groove.bottom()));
    grad_rrect(&p, fill, 3.0, CYAN, Color32::from_rgb(0x0b, 0x6c, 0xa8));
    let thumb = pos2(groove.left() + groove.width() * v, groove.center().y);
    let resp = ui.interact(groove.expand2(vec2(10.0, 10.0)), ui.id().with("vol"), Sense::click_and_drag());
    if resp.drag_started() || resp.clicked() {
        setter.begin_set_parameter(&params.volume);
    }
    if let Some(pos) = resp.interact_pointer_pos() {
        let nv = ((pos.x - groove.left()) / groove.width()).clamp(0.0, 1.0);
        setter.set_parameter_normalized(&params.volume, nv);
    }
    if resp.drag_stopped() || resp.clicked() {
        setter.end_set_parameter(&params.volume);
    }
    chrome_button(&p, thumb, 9.0, resp.dragged(), resp.hovered());
    p.text(
        vol_row.right_center() - vec2(4.0, 0.0),
        Align2::RIGHT_CENTER,
        params.volume.to_string(),
        FontId::monospace(11.0),
        Color32::from_gray(190),
    );

    // ---------- toggles ----------
    let follow = params.follow.value();
    let sync = r(508.0, 386.0, 650.0, 410.0);
    let resp = ui.interact(sync, ui.id().with("sync"), Sense::click());
    pill(&p, sync, "SYNC TO DAW", follow, resp.hovered());
    if resp.clicked() {
        setter.begin_set_parameter(&params.follow);
        setter.set_parameter(&params.follow, !follow);
        setter.end_set_parameter(&params.follow);
    }
    pill(&p, r(662.0, 386.0, 804.0, 410.0), "GAME BOY", gb_mode, false);

    // ---------- MIDI mapper ----------
    let panel = r(16.0, 420.0, 804.0, 566.0);
    inset(&p, panel, 10.0);
    p.text(panel.left_top() + vec2(14.0, 14.0), Align2::LEFT_CENTER, "MIDI MAPPER", FontId::proportional(12.0), CYAN);
    p.text(
        panel.left_top() + vec2(110.0, 14.0),
        Align2::LEFT_CENTER,
        if learning.is_some() { "now press a key or turn a knob on your controller…" } else { "click a button, then press a key or turn a knob to bind it" },
        FontId::proportional(10.0),
        if learning.is_some() { ORANGE } else { Color32::from_gray(140) },
    );
    let mapping = params.mapping.read().unwrap().clone();
    let cols = 7;
    let (bw, bh, gap) = (104.0, 50.0, 7.5);
    for (i, t) in Target::ALL.iter().enumerate() {
        let (col, row) = (i % cols, i / cols);
        let x0 = panel.left() + 12.0 + col as f32 * (bw + gap);
        let y0 = panel.top() + 28.0 + row as f32 * (bh + 8.0);
        let rect = Rect::from_min_size(pos2(x0, y0), vec2(bw, bh));
        let resp = ui.interact(rect, ui.id().with(("map", i)), Sense::click());
        let is_learning = learning == Some(*t);
        let down = held & (1 << t.index()) != 0;
        let (top, bot) = if down {
            (Color32::from_rgb(0x1f, 0x7a, 0xa8), Color32::from_rgb(0x0d, 0x3a, 0x55))
        } else if resp.hovered() {
            (Color32::from_gray(98), Color32::from_gray(44))
        } else {
            (Color32::from_gray(78), Color32::from_gray(32))
        };
        grad_rrect(&p, rect, 8.0, top, bot);
        grad_rrect(&p, Rect::from_min_max(rect.min, pos2(rect.right(), rect.center().y)), 8.0, Color32::from_white_alpha(50), Color32::from_white_alpha(6));
        p.rect_stroke(rect, 8.0, Stroke::new(1.0, if is_learning && blink { ORANGE } else { Color32::from_gray(20) }), StrokeKind::Inside);
        let led = if is_learning {
            if blink { ORANGE } else { ORANGE.gamma_multiply(0.2) }
        } else if down {
            Color32::from_rgb(0x6b, 0xff, 0x7a)
        } else {
            Color32::from_rgb(0x1c, 0x3a, 0x1f)
        };
        orb(&p, rect.right_top() + vec2(-10.0, 10.0), 4.0, led);
        p.text(rect.left_top() + vec2(10.0, 15.0), Align2::LEFT_CENTER, t.label(), FontId::proportional(12.0), Color32::WHITE);
        let src = mapping.source_of(*t).map(|s| s.name()).unwrap_or_else(|| "—".into());
        p.text(rect.left_bottom() + vec2(10.0, -13.0), Align2::LEFT_CENTER, src, FontId::monospace(11.0), LCD_ON);
        if resp.clicked() {
            let mut l = shared.learn.lock().unwrap();
            *l = if *l == Some(*t) { None } else { Some(*t) };
        }
    }
    p.text(
        panel.right_bottom() + vec2(-12.0, -12.0),
        Align2::RIGHT_CENTER,
        "tip: arm the track in your DAW to record your playthrough as MIDI",
        FontId::proportional(9.0),
        Color32::from_gray(110),
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

/// Band levels 0..1, log-spaced 50 Hz – 14 kHz (Goertzel per band, Hann window).
fn spectrum(samples: &[f32], rate: f32) -> [f32; BARS] {
    let n = samples.len();
    let mut out = [0.0; BARS];
    for (b, level) in out.iter_mut().enumerate() {
        let f = 50.0 * (14000.0f32 / 50.0).powf(b as f32 / (BARS - 1) as f32);
        let coeff = 2.0 * (std::f32::consts::TAU * f / rate).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for (i, x) in samples.iter().enumerate() {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos();
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

/// No disc: a starfield and a pulsing "INSERT SWF".
fn attract(p: &egui::Painter, screen: Rect, time: f64, status: &str) {
    let sp = p.with_clip_rect(screen);
    let t = time as f32;
    for i in 0..70u32 {
        let h = i.wrapping_mul(2654435761);
        let fx = (h % 1000) as f32 / 1000.0;
        let fy = ((h >> 10) % 1000) as f32 / 1000.0;
        let speed = 0.02 + ((h >> 20) % 100) as f32 / 1500.0;
        let x = screen.left() + ((fx + t * speed) % 1.0) * screen.width();
        let y = screen.top() + fy * screen.height();
        sp.circle_filled(pos2(x, y), 0.6 + speed * 12.0, Color32::from_white_alpha((60.0 + speed * 2000.0).min(220.0) as u8));
    }
    let pulse = 0.55 + 0.45 * (t * 3.0).sin();
    glow_text(&sp, screen.center() - vec2(0.0, 12.0), Align2::CENTER_CENTER, "INSERT  SWF", FontId::proportional(34.0), CYAN.gamma_multiply(pulse), CYAN.gamma_multiply(0.12 * pulse));
    let sub = if status.is_empty() { "press  ⏏  EJECT  to load a game" } else { status };
    sp.text(screen.center() + vec2(0.0, 26.0), Align2::CENTER_CENTER, sub, FontId::monospace(12.0), if status.starts_with("ERROR") { ORANGE } else { Color32::from_gray(170) });
}

// ---------- drawing helpers ----------

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
            let a = i as f32 / 40.0 * std::f32::consts::TAU;
            c + vec2(a.cos(), a.sin()) * rad
        })
        .collect();
    grad_fill(p, &pts, Rect::from_center_size(c, Vec2::splat(rad * 2.0)), top, bot);
}

/// Sunken panel.
fn inset(p: &egui::Painter, rect: Rect, r: f32) {
    grad_rrect(p, rect, r, Color32::from_gray(14), Color32::from_gray(40));
    p.rect_stroke(rect, r, Stroke::new(1.0, Color32::from_black_alpha(160)), StrokeKind::Inside);
    p.line_segment(
        [rect.left_bottom() + vec2(r, 0.0), rect.right_bottom() - vec2(r, 0.0)],
        Stroke::new(1.0, Color32::from_white_alpha(30)),
    );
}

/// Glossy round light.
fn orb(p: &egui::Painter, c: Pos2, rad: f32, color: Color32) {
    p.circle_filled(c, rad + 1.0, Color32::from_black_alpha(140));
    grad_circle(p, c, rad, mix(color, Color32::WHITE, 0.25), mix(color, Color32::BLACK, 0.45));
    p.circle_filled(c - vec2(rad * 0.3, rad * 0.4), rad * 0.4, Color32::from_white_alpha(120));
}

/// Round chrome button with a glass highlight.
fn chrome_button(p: &egui::Painter, c: Pos2, rad: f32, down: bool, hover: bool) {
    p.circle_filled(c + vec2(0.0, 2.0), rad + 2.0, Color32::from_black_alpha(120));
    grad_circle(p, c, rad + 1.5, Color32::from_gray(200), Color32::from_gray(40));
    let (top, bot) = if down {
        (Color32::from_gray(40), Color32::from_gray(95))
    } else if hover {
        (Color32::from_gray(125), Color32::from_gray(42))
    } else {
        (Color32::from_gray(105), Color32::from_gray(30))
    };
    grad_circle(p, c, rad, top, bot);
    let hl = Rect::from_center_size(c - vec2(0.0, rad * 0.42), vec2(rad * 1.45, rad * 0.9));
    grad_rrect(p, hl, rad * 0.45, Color32::from_white_alpha(if down { 25 } else { 85 }), Color32::from_white_alpha(4));
    if hover {
        p.circle_stroke(c, rad + 2.5, Stroke::new(1.0, CYAN.gamma_multiply(0.6)));
    }
}

fn pill(p: &egui::Painter, rect: Rect, label: &str, on: bool, hover: bool) {
    grad_rrect(p, rect, rect.height() / 2.0, Color32::from_gray(if hover { 80 } else { 62 }), Color32::from_gray(24));
    grad_rrect(p, Rect::from_min_max(rect.min, pos2(rect.right(), rect.center().y)), rect.height() / 2.0, Color32::from_white_alpha(40), Color32::from_white_alpha(4));
    orb(p, rect.left_center() + vec2(13.0, 0.0), 4.5, if on { Color32::from_rgb(0x6b, 0xff, 0x7a) } else { Color32::from_rgb(0x1c, 0x3a, 0x1f) });
    p.text(rect.left_center() + vec2(26.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(11.0), if on { Color32::WHITE } else { Color32::from_gray(150) });
}

fn glow_text(p: &egui::Painter, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32, glow: Color32) {
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0), vec2(0.0, 2.0)] {
        p.text(pos + d, align, text, font.clone(), glow);
    }
    p.text(pos, align, text, font, color);
}

fn lcd_flag(p: &egui::Painter, pos: Pos2, text: &str, on: bool) {
    p.text(pos, Align2::RIGHT_CENTER, text, FontId::monospace(10.0), if on { LCD_ON } else { LCD_DIM });
}

fn icon_play(p: &egui::Painter, c: Pos2, col: Color32) {
    p.add(Shape::convex_polygon(vec![c + vec2(-6.0, -9.0), c + vec2(10.0, 0.0), c + vec2(-6.0, 9.0)], col, Stroke::NONE));
}

fn icon_pause(p: &egui::Painter, c: Pos2, col: Color32) {
    p.rect_filled(Rect::from_center_size(c - vec2(5.0, 0.0), vec2(5.0, 17.0)), 1.0, col);
    p.rect_filled(Rect::from_center_size(c + vec2(5.0, 0.0), vec2(5.0, 17.0)), 1.0, col);
}

fn icon_stop(p: &egui::Painter, c: Pos2, col: Color32) {
    p.rect_filled(Rect::from_center_size(c, vec2(15.0, 15.0)), 2.0, col);
}

fn icon_eject(p: &egui::Painter, c: Pos2, col: Color32) {
    p.add(Shape::convex_polygon(vec![c + vec2(0.0, -10.0), c + vec2(10.0, 2.0), c + vec2(-10.0, 2.0)], col, Stroke::NONE));
    p.rect_filled(Rect::from_min_max(c + vec2(-10.0, 5.0), c + vec2(10.0, 9.0)), 1.0, col);
}

fn icon_reset(p: &egui::Painter, c: Pos2, col: Color32) {
    let pts: Vec<Pos2> = (0..=24)
        .map(|i| {
            let a = (40.0 + i as f32 * 12.0f32).to_radians();
            c + vec2(a.cos(), a.sin()) * 9.0
        })
        .collect();
    let end = *pts.last().unwrap();
    p.add(Shape::line(pts, Stroke::new(2.5, col)));
    p.add(Shape::convex_polygon(vec![end + vec2(-1.0, -6.0), end + vec2(6.0, 2.0), end + vec2(-4.0, 4.0)], col, Stroke::NONE));
}
