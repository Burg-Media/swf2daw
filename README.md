# swf2DAW

Flash in your DAW. swf2DAW is a VST3/CLAP instrument that plays `.swf` files (and
gb2swf Game Boy games) inside the plugin window. The game's sound comes out
of the plugin as a normal instrument track, MIDI notes and CCs play the game.

Built on [Ruffle](https://ruffle.rs) (`ruffle_core`, pinned commit) and
[nih-plug](https://github.com/robbert-vdh/nih-plug).

## Features (v0.1)

- Loads any SWF (eject button), remembers it in the DAW project
- Game sound straight into the DAW at the host sample rate, 32-bit float, so recording,
  bouncing, freezing and effects just work
  - Ruffle sounds through Ruffle's mixer, created at the host rate
  - gb2swf games: their `ExternalInterface.call("gb2swfAudio", …)` samples are caught by the
    plugin (no browser, no WebAudio), resampled once with a drift-correcting Hermite resampler
- MIDI: notes and CCs to keys (arrows, X, Z, Enter, Shift, Space, C), mouse X/Y and click
  - default layout: C4 left, D4 down, D#4 up, E4 right, F4 select, G4 start, A4 B, B4 A,
    C5 space, D5 C; mod wheel = mouse X, CC 2 = mouse Y, sustain = click
  - MIDI learn: click a mapper button, then press a key / turn a knob
  - gb2swf games are driven through `gbPad(mask)` directly, no keyboard focus needed
- Mouse on the screen and keyboard in the editor work too
- Game saves (SharedObjects) are stored in the DAW project
- "Sync to DAW": the game only runs while the transport plays
- Retro 2010 media player skin: LCD ticker, LED spectrum analyzer, chrome buttons

Recording a playthrough: arm the instrument track and record. The MIDI you play is
recorded too, so the take can be played back into the game later.

## Build

Needs Rust (stable) and Java (Ruffle compiles its playerglobal at build time).

```
cargo xtask bundle swf2daw --release
```

Output: `target/bundled/swf2DAW.vst3` and `swf2DAW.clap`. GitHub Actions builds the
Windows bundles on every push (artifact `swf2DAW-windows`).

Install on Windows: copy `swf2DAW.vst3` to `C:\Program Files\Common Files\VST3\`
(and/or `swf2DAW.clap` to `C:\Program Files\Common Files\CLAP\`), then rescan plugins.

## Test game

`demo/tobu-tobu-girl-deluxe.swf`: Tobu Tobu Girl Deluxe by Tangram Games converted with
gb2swf (code MIT, assets CC BY 4.0, see `demo/CREDITS.txt`). Load it with the eject
button. Steer with C4 / E4, G4 is Start, B4 is the A button.

## Architecture

| File | What |
|---|---|
| `src/lib.rs` | plugin, parameters, persisted state, MIDI in, audio out |
| `src/engine.rs` | Ruffle player on its own thread, offscreen wgpu render, ExternalInterface, input |
| `src/audio.rs` | Ruffle mixer backend, gb2swf stream resampler |
| `src/mapping.rs` | MIDI → game mapping, MIDI learn |
| `src/ui.rs` | the hand-drawn egui skin |

## Licence

GPL-3.0-or-later, see `LICENSE`. Copyright (c) 2026 Burg Media.

GPL because nih-plug's VST3 wrapper is GPLv3. Ruffle (MIT OR Apache-2.0), nih-plug (ISC)
and the other dependencies keep their own licences. The demo game in `demo/` has its own
licence and credits, see `demo/CREDITS.txt`.

## Roadmap

- Multi-out stems (gb2swf: pulse 1, pulse 2, wave, noise on separate outputs)
- Deterministic replay + faster-than-realtime offline bounce
- Embed the SWF in the project instead of a path
- Presets per game, MIDI out, BPM-synced frame rate
