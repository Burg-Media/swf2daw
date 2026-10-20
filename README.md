# swf2DAW

Flash in your DAW. swf2DAW is a VST3/CLAP instrument that plays `.swf` files (and
gb2swf Game Boy games) inside the plugin window. The game's sound comes out
of the plugin as a normal instrument track, MIDI notes and CCs play the game.

Built on [Ruffle](https://ruffle.rs) (`ruffle_core`, pinned commit) and
[nih-plug](https://github.com/robbert-vdh/nih-plug).

## Features

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
- Hand-drawn 1990s rack unit skin: CRT screen, VFD ticker and spectrum, needle VU meters, hardware keys
- Every channel on its own output: Pulse 1, Pulse 2, Wave, Noise (gb2swf games built
  with stems support), Music and SFX
  - Flash games: every sound is sorted when it starts. Streams, looping sounds and
    sounds of 8 seconds or longer go to Music, all other sounds to SFX
  - older gb2swf games (no stems) play their whole mix on Music
  - mixer tab: per channel low / mid / high EQ, level, pan, mute, solo and a meter
  - EQ + FX tab, on the main output: 10-band EQ with presets, Mega Bass, a filter pad
    (cutoff / resonance) and Bitcrush, Tape Wobble, Ring Mod, Trance Gate and Echo
    (both synced to the DAW tempo) and Space
  - every knob is a plugin parameter, so the DAW can automate it

### Recording the channels on separate tracks

The plugin has a main output (the mix, through the EQ and effects) and six extra
stereo outputs, one per channel, after the channel strip and before the EQ and effects.

In FL Studio: open the plugin wrapper settings (the cog at the top left of the plugin
window) > Processing, and give every output in the Outputs list its own mixer track.
Record the channel tracks for separate stems, or the main output for the
finished mix; don't send both to the master or you hear everything twice.

Game Boy games give a sound effect a music channel for a moment, so an effect sounds
on the channel it borrows (often Noise or Pulse 1). The split is per channel, not
music vs. effects.

The four Game Boy channels only come out separately for games converted with a gb2swf
that supports stems; older SWFs play their whole mix on the Music channel.

Recording a playthrough: arm the instrument track and record. The MIDI you play is
recorded too, so the take can be played back into the game later.

## Build

Needs Rust (stable) and Java (Ruffle compiles its playerglobal at build time).

```
cargo xtask bundle swf2daw --release
```

Output: `target/bundled/swf2DAW.vst3` and `swf2DAW.clap`. GitHub Actions also builds the
Windows bundles on every push (artifact `swf2DAW-windows`).

### Windows, step by step

1. Install the **Visual Studio Build Tools** with the "Desktop development with C++"
   workload (Rust needs the MSVC linker).
2. Install **Rust** from https://rustup.rs (default options).
3. Install a **Java JDK** 17 or newer, e.g. `winget install EclipseAdoptium.Temurin.17.JDK`.
4. Open a new terminal in the repo and run `cargo xtask bundle swf2daw --release`.
   The first build downloads and compiles Ruffle and takes 10 to 20 minutes and several GB;
   later builds take a minute or two.
5. Copy `target\bundled\swf2DAW.vst3` to `C:\Program Files\Common Files\VST3\`
   (and/or `swf2DAW.clap` to `C:\Program Files\Common Files\CLAP\`), then rescan
   plugins in your DAW. In FL Studio: Options > Manage plugins > Find more plugins.

FL Studio keeps the plugin DLL locked while a project uses it: close the project (or FL)
before copying a new build over the old one.

## Test game

`demo/tobu-tobu-girl-deluxe.swf`: Tobu Tobu Girl Deluxe by Tangram Games converted with
gb2swf (code MIT, assets CC BY 4.0, see `demo/CREDITS.txt`). Load it with the eject
button. Steer with C4 / E4, G4 is Start, B4 is the A button.

## Architecture

| File | What |
|---|---|
| `src/lib.rs` | plugin, parameters, persisted state, MIDI in, audio out |
| `src/engine.rs` | Ruffle player on its own thread, offscreen wgpu render, ExternalInterface, input |
| `src/audio.rs` | Ruffle audio backend (music and SFX mixers), gb2swf stream resampler |
| `src/dsp.rs` | channel strip EQ, master EQ, filter and effects |
| `src/params.rs` | parameters of the channel strips and the master rack |
| `src/mapping.rs` | MIDI → game mapping, MIDI learn |
| `src/ui.rs` | the hand-drawn egui skin |

## Licence

GPL-3.0-or-later, see `LICENSE`. Copyright (c) 2026 Burg Media.

GPL because nih-plug's VST3 wrapper is GPLv3. Ruffle (MIT OR Apache-2.0), nih-plug (ISC)
and the other dependencies keep their own licences. The demo game in `demo/` has its own
licence and credits, see `demo/CREDITS.txt`.

## Roadmap

- Deterministic replay + faster-than-realtime offline bounce
- Embed the SWF in the project instead of a path
- Presets per game, MIDI out, BPM-synced frame rate
