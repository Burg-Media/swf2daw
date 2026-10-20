# swf2daw: feiten

Bronpakket voor de case, shoppagina en blog op burgmedia.eu. Alles hieronder komt uit de code,
de README, de git-geschiedenis of de test van 20 oktober 2026 in FL Studio. Wat niet zeker is,
staat onder "Open vragen".

## Belangrijk vooraf: het is geen converter

De opdracht gaat uit van een conversietool (SWF erin, audio/MIDI/samples eruit, importeren in
FL Studio). Zo werkt swf2daw niet. swf2daw is een **instrument-plugin** (VST3 en CLAP): je zet
hem in je DAW op een kanaal, laadt er een .swf-bestand in en de Flash-game of -animatie draait
live in het pluginvenster. Het geluid komt als gewone audio uit de plugin, en MIDI-noten
bedienen de game. Er wordt niets geconverteerd of geëxporteerd; opnemen doe je met de DAW zelf.

De conversie in de Burg Media-keten zit een stap eerder, in **gb2swf** (een aparte tool die
een Game Boy-ROM omzet naar een .swf). swf2daw speelt zulke gb2swf-bestanden en gewone
Flash-bestanden af.

## Wat swf2daw doet, in 3 zinnen

1. swf2daw zet een Flash-game of Flash-animatie (.swf) als instrument in je muziekprogramma,
   zodat je hem daar gewoon kunt spelen en zien.
2. Het geluid van de game komt rechtstreeks in je project terecht, dus je kunt het opnemen,
   bewerken en er effecten op zetten zoals bij elk ander instrument.
3. Je bestuurt de game met je MIDI-keyboard of met de muis, en wat je speelt neemt de DAW mee
   op, zodat een speelsessie ook muziek wordt.

## Voor wie en welk probleem

- **Voor wie:** muzikanten en producers die met chiptune, gamegeluid of Flash-nostalgie werken;
  makers die een gb2swf-conversie van hun eigen of vrije Game Boy-game in hun muziek willen
  gebruiken; liefhebbers die Flash-werk willen bewaren en hergebruiken.
- **Probleem dat het oplost:** gamegeluid uit een Flash-game of Game Boy-game in een DAW krijgen
  ging tot nu toe via een omweg: de game in een browser of emulator draaien en het geluid via
  een loopback of schermopname opvangen, met vertraging, ruis en geen scheiding tussen muziek en
  effecten. swf2daw speelt de game in de DAW zelf, op de samplerate van het project, in 32-bit
  float, en geeft elk geluidskanaal een eigen uitgang.

## Werkwijze stap voor stap

1. **Installeren:** de plugin-bestanden `swf2DAW.vst3` en/of `swf2DAW.clap` in de standaard
   pluginmap zetten (Windows: `C:\Program Files\Common Files\VST3\` of `...\CLAP\`) en in de DAW
   de plugins opnieuw laten zoeken. In FL Studio: Options > Manage plugins > Find more plugins.
2. **Plugin op een kanaal zetten:** in FL Studio via het plusje in de Channel rack > More plugins
   > swf2DAW. FL Studio toont hem als "swf2DAW, CLAP, 64 bit, Burg Media" (screenshot 06).
3. **SWF laden:** op EJECT klikken en een .swf kiezen. Het scherm in de plugin toont de game,
   de lichtkrant toont "NOW PLAYING: (bestandsnaam)" en "GAME BOY MODE" of "FLASH MODE".
4. **Spelen:** met MIDI-noten (standaard C4 links, E4 rechts, G4 Start, B4 de A-knop, zie
   hieronder), met de muis op het scherm, of door op de knoppen in het tabblad MIDI MAPPER te
   klikken.
5. **Geluid:** de hoofduitgang geeft de mix (door de EQ en effecten). Daarnaast zijn er zes extra
   stereo-uitgangen: Pulse 1, Pulse 2, Wave, Noise, Music en SFX. In FL Studio koppel je die via
   het tandwiel van de plugin > Processing > "Auto map outputs" aan aparte mixertracks
   (screenshot 07).
6. **Opnemen:** met de normale opnamefunctie van de DAW. Als het instrumentkanaal opneemt,
   worden ook de MIDI-noten opgenomen, zodat de speelsessie later opnieuw de game in kan.
7. **Opslaan:** het pad naar de SWF, de MIDI-indeling en de savegames van de game worden in het
   DAW-project opgeslagen. Bij heropenen start dezelfde game weer.

**Wat erin gaat:** een .swf-bestand, MIDI-noten en -CC's, muis- en toetsenbordinvoer.
**Wat eruit komt:** alleen audio (1 stereo mix plus 6 stereo kanalen) en het beeld in het
pluginvenster. Geen MIDI-uitvoer, geen samples, geen bestanden.

**Andere DAW's:** het zijn standaard VST3- en CLAP-plugins, dus elke DAW die een van die formaten
laadt zou moeten werken. Getest is alleen FL Studio 2024 op Windows 11 (zie Open vragen).

### Standaard MIDI-indeling

| Noot (middelste C = C4) | Functie |
|---|---|
| C4 | links |
| D4 | omlaag |
| D#4 | omhoog |
| E4 | rechts |
| F4 | Select |
| G4 | Start |
| A4 | B-knop (Z) |
| B4 | A-knop (X) |
| C5 | spatie |
| D5 | C-toets |
| CC 1 (modwiel) | muis X |
| CC 2 | muis Y |
| CC 64 (sustain) | klik |

Let op: FL Studio noemt de middelste C "C5", dus in FL Studio liggen deze noten een octaaf hoger
dan de namen hierboven. De plugin vermeldt dat zelf onderaan het tabblad MIDI MAPPER.
Elke knop is opnieuw toe te wijzen (MIDI learn: op het MIDI-label klikken, dan een toets of knop
indrukken) en met de muis te bedienen.

## Ondersteunde invoer en uitvoer

- **Invoer:** .swf-bestanden. Afspelen gebeurt met Ruffle (de open source Flash Player-vervanger),
  vastgezet op een vaste Ruffle-versie. Welke SWF-versies en ActionScript-versies werken, hangt
  af van Ruffle; swf2daw voegt daar zelf niets aan toe of af.
- **Getest bestand:** `tobu-tobu-girl-deluxe.swf` (gb2swf-conversie, SWF-versie 17, gecomprimeerd,
  ActionScript 3, 60 fps in de header). Zie "Bekende beperkingen" voor wat wel en niet getest is.
- **Geluid in SWF's:** de build zet Ruffle's decoders voor MP3, AAC en Nellymoser aan.
- **gb2swf-games:** worden herkend zodra de game zijn `gbPad`-functie aanmeldt ("GAME BOY MODE").
  Hun geluid komt via een eigen route binnen (zie techniek.md). Games die gemaakt zijn met een
  gb2swf-versie met "stems" geven de vier Game Boy-kanalen apart; oudere geven alles op Music.
- **Uitvoer:** audio, stereo, 32-bit float, op de samplerate van de DAW. 1 hoofduitgang (mix) en
  6 extra uitgangen (Pulse 1, Pulse 2, Wave, Noise, Music, SFX).
- **Bij Flash-games:** elk geluid wordt bij het starten ingedeeld: streams, herhalende geluiden
  en geluiden van 8 seconden of langer gaan naar Music, de rest naar SFX.
- **Geluidsopties in de plugin:** per kanaal een 3-bands EQ, volume, pan, mute, solo en meter;
  op de mix een 10-bands EQ met presets (PHONE, SMILE, FLAT), Mega Bass, een filterpad
  (cutoff/resonantie) en zes effecten: Bitcrush, Tape Wobble, Ring Mod, Trance Gate, Echo en
  Space. Trance Gate en Echo lopen mee op het tempo van de DAW.
- **Automatisering:** 70 parameters (42 voor de zes kanaalstrips, 26 voor EQ en effecten,
  plus volume en "Sync to DAW") zijn gewone pluginparameters die de DAW kan automatiseren.
- **Sync to DAW:** als dat aan staat, loopt de game alleen als het transport van de DAW speelt.

## Platforms en installatie

- **Vorm:** plugin met grafische interface (geen los programma, geen CLI). Formaten: VST3 en CLAP.
- **Windows:** gebouwd en getest (Windows 11, FL Studio 2024, CLAP-versie geladen).
- **macOS:** het project is volgens de maker op een Mac gebouwd; of het daar in een DAW getest is,
  is niet bekend (Open vragen).
- **Linux:** niet gebouwd en niet getest.
- **Download:** er is nog geen release. GitHub Actions bouwt bij elke push de Windows-versie
  (artifact `swf2DAW-windows`). Zelf bouwen kan met Rust en Java:
  `cargo xtask bundle swf2daw --release` (stappen in de README).
- **Systeemeis:** de plugin rendert het beeld via de grafische kaart (wgpu).

## Versie, licentie, GitHub

- **Versie:** in `Cargo.toml` staat 0.1.0 (de plugin meldt zich ook zo, "FIRMWARE 0.1.0" in de
  interface). De functies uit dit pakket (kanaaluitgangen, mixer, EQ en effecten, nieuwe skin)
  zijn op 20 oktober 2026 als "v0.2" op `main` gezet; het versienummer is nog niet opgehoogd.
- **Licentie:** GPL-3.0-or-later (omdat de VST3-laag van nih-plug GPLv3 is). Ruffle (MIT of
  Apache-2.0) en nih-plug (ISC) houden hun eigen licentie.
- **GitHub:** https://github.com/Burg-Media/swf2daw, **publiek**.
- **Maker:** Burg Media (burgmedia.eu). Gebouwd op Ruffle en nih-plug.

## Bekende beperkingen

- **Alleen in realtime.** Er is geen snelle offline-bounce: een game speelt in echte tijd, dus
  exporteren of bouncen sneller dan realtime levert geen bruikbaar resultaat. Staat op de roadmap.
- **Niet deterministisch.** Dezelfde opgenomen MIDI geeft niet gegarandeerd precies dezelfde
  speelsessie opnieuw. Staat op de roadmap.
- **SWF wordt via een pad bewaard,** niet in het project zelf. Verplaats je het project naar een
  andere computer, dan moet de .swf op hetzelfde pad staan. Staat op de roadmap.
- **Andere samplerate = herstart.** Verandert de DAW de samplerate, dan start de game opnieuw.
- **Kanaalscheiding bij Game Boy-games is per kanaal, niet muziek tegen effecten:** een Game
  Boy-game leent voor een geluidseffect even een muziekkanaal, dus effecten klinken op het kanaal
  dat ze lenen (vaak Noise of Pulse 1).
- **Oudere gb2swf-games zonder stems** geven hun hele mix op Music. Dat is ook bij de testgame zo:
  in screenshot 08 heeft alleen het Music-kanaal signaal.
- **Dubbel geluid in FL Studio:** stuur je zowel de hoofduitgang als de losse kanalen naar de
  master, dan hoor je alles twee keer. Kies per project: de mix, of de losse kanalen.
- **FL Studio houdt de plugin vast:** een nieuwe build kan pas over de oude heen als het project
  (of FL Studio) dicht is.
- **Beeld maximaal 960 pixels** aan de langste kant; grotere SWF's worden kleiner weergegeven.
  Het pluginvenster is 1000 bij 770 pixels.
- **Alleen getest met één vrij te tonen game.** Zie Open vragen.

## Open vragen

1. **Andere DAW's:** werkt swf2daw in Ableton, Reaper, Bitwig, Studio One, Cubase? Niet getest.
2. **macOS:** is de Mac-build ook in een DAW getest, en welke? Is de Mac-versie gesigneerd?
3. **Release:** komt er een downloadbare release (en zo ja, gratis of betaald via de shop)?
   Nu is er alleen zelf bouwen of het CI-artifact (dat een GitHub-account vraagt).
4. **Versienummer:** wordt dit 0.2.0? In de code staat nog 0.1.0.
5. **Compatibiliteit met gewone Flash-games:** tijdens de ontwikkeling is ook een bestaande,
   auteursrechtelijk beschermde Flash-game geladen. Tijdens die test meldde de gebruiker dat de
   effecten niet leken te werken; daarna is het Music/SFX-systeem gebouwd. Of het daarna in die
   game goed werkte, is niet vastgelegd. Hoe breed Flash-games werken, is niet gemeten.
6. **Netwerk en externe bestanden:** swf2daw stelt in Ruffle geen netwerk- of bestandstoegang in.
   SWF's die bij het starten andere bestanden of internet nodig hebben, zullen naar verwachting
   niet volledig werken. Niet getest.
7. **Prijs en positionering op de shoppagina:** niet in de repo vastgelegd.
