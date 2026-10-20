Flash (SWF) and gb2swf Game Boy games as a VST3/CLAP instrument in your DAW, powered by Ruffle.

## New in 0.2.0

- Six extra stereo outputs, one per channel: Pulse 1, Pulse 2, Wave, Noise (gb2swf games built with stems), Music and SFX
- Flash games: every sound is sorted when it starts. Streams, loops and sounds of 8 seconds or longer go to Music, the rest to SFX
- Channel mixer: low / mid / high EQ, level, pan, mute, solo and a meter per channel
- EQ + FX rack on the main output: 10-band EQ with presets, Mega Bass, filter pad, Bitcrush, Tape Wobble, Ring Mod, Trance Gate and Echo (synced to the DAW tempo) and Space
- Every knob is an automatable plugin parameter
- MIDI mapper: click a button to play it, click its MIDI tag to learn, right-click to unbind; notes show their number (FL Studio calls note 60 "C5")
- New 1990s rack unit skin

## Install (Windows)

Unzip and copy `swf2DAW.vst3` to `C:\Program Files\Common Files\VST3\` or `swf2DAW.clap` to `C:\Program Files\Common Files\CLAP\`, then rescan plugins in your DAW. Tested in FL Studio 2024 on Windows 11 (CLAP). Check the download against `SHA256SUMS`.

Demo game: `demo/tobu-tobu-girl-deluxe.swf` in the repository (Tobu Tobu Girl Deluxe by Tangram Games, code MIT, assets CC BY 4.0).

GPL-3.0-or-later. Made by [Burg Media](https://burgmedia.eu/portfolio/swf2daw).
