# Skin mock

`skin-mock.dc.html` is the design reference for the editor UI (1120×720): a 2010 Windows
media player look, every visual driven by the audio, plus the EQ / FX rack the plugin will
get.

The mock plays its own demo chiptune with WebAudio; the plugin will run the same effects
as Rust DSP on the game audio. `src/ui.rs` still has the first, simpler skin.
