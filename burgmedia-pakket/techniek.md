# swf2daw: techniek

## Taal, libraries, architectuur

swf2daw is geschreven in **Rust** (edition 2024), 3296 regels in `src/` (geteld met `wc -l` op
de v0.2-commit). De belangrijkste bouwstenen:

- **Ruffle** (`ruffle_core` en `ruffle_render_wgpu`): de open source Flash Player in Rust. Vast
  op één commit, omdat `ruffle_core` geen stabiele API heeft.
- **nih-plug** en **nih_plug_egui**: het pluginframework dat er één codebase als VST3 en CLAP
  uit bouwt, met egui voor de interface.
- **rtrb** (lock-free ringbuffers), **crossbeam-channel** (commando's), **wgpu** (via Ruffle),
  **rfd** (bestandskiezer), **base64** en **serde** (projectstatus).

De plugin bestaat uit drie draden die elkaar nooit laten wachten:

1. **De audiodraad van de DAW** (`src/lib.rs`): leest MIDI, mixt Ruffle's geluid en de
   gb2swf-stream, draait de kanaalstrips en de effecten (`src/dsp.rs`) en schrijft de zeven
   uitgangen. Hier wordt niets gealloceerd en op niets gewacht.
2. **De engine-draad** (`src/engine.rs`): draait de Ruffle-speler, rendert elk beeld offscreen
   naar een wgpu-texture, vangt de ExternalInterface-aanroepen van de game op en zet MIDI om in
   toetsen, muis of Game Boy-knoppen.
3. **De editor** (`src/ui.rs`): een met de hand getekende egui-interface (rackmodule uit de jaren
   negentig met CRT-scherm, lichtkrant, spectrum, VU-meters en drie tabbladen).

Ze praten via een gedeelde `Shared`-struct met atomics, een commandokanaal en twee ringbuffers
(MIDI erin, gb2swf-audio eruit).

## De lastigste problemen en hoe ze zijn opgelost

### 1. Een Flash-speler die niet in een plugin wil passen

Ruffle's `Player` gebruikt `Rc` en is daardoor niet thread-safe: hij kan niet van draad wisselen.
Een DAW roept een plugin juist aan vanaf verschillende draden, en op de audiodraad mag niets
blokkeren. **Oplossing:** de speler leeft zijn hele leven op een eigen draad
(`swf2daw-engine`). De audiodraad haalt Ruffle's mixers op via proxies en gebruikt `try_lock`:
is de engine net een nieuwe game aan het laden, dan slaat de audiodraad dat ene blok over in
plaats van te wachten. MIDI gaat via een lock-free ringbuffer naar de engine, die in korte
slaapjes van 1 tot 4 ms draait zodat invoer snel aankomt.

### 2. Geluid van gb2swf-games zonder browser

gb2swf-games maken hun geluid zelf (een Game Boy-geluidschip in ActionScript) en geven de samples
in de browser via `ExternalInterface.call("gb2swfAudio", samples)` door aan WebAudio, omdat Ruffle
geen `SampleDataEvent` heeft. In een plugin is er geen browser. **Oplossing:** swf2daw doet zich
voor als de webpagina rond de SWF (`PluginExternalInterface`), antwoordt "ja" op
`gb2swfAudioReady`, en vangt de samples op in een ringbuffer van ongeveer 1 seconde.

Die samples komen op 44,1 kHz binnen, in brokken per gameframe, terwijl de DAW een eigen
samplerate en een eigen klok heeft. Een `StreamResampler` speelt de stream af op de rate van de
DAW met 4-punts Hermite-interpolatie en buigt de afspeelsnelheid maximaal 0,5% bij om de buffer
rond 60 ms vulling te houden. Loopt de buffer leeg, dan valt hij stil en wacht tot er weer 60 ms
klaarstaat; loopt hij meer dan vier keer te vol (na een hapering van DAW of editor), dan springt
hij vooruit. Zo ontstaat geen hoorbare toonhoogteverschuiving en geen groeiende vertraging.

### 3. Eén mixer, toch muziek en effecten apart

Ruffle heeft één audiomixer, dus alles komt als één mix naar buiten. **Oplossing:** een eigen
`AudioBackend` met twee Ruffle-mixers, één voor muziek en één voor effecten. Elk geluid wordt in
beide geregistreerd; pas bij het starten wordt gekozen waar het speelt: streams, herhalende
geluiden en geluiden van 8 seconden of langer naar Music, de rest naar SFX. De backend houdt zelf
bij welke instantie in welke mixer zit, en ruimt instanties op die vanzelf gestopt zijn.

Voor gb2swf-games met stems (`gb2swfAudioStems`, 8 waarden per frame: vier stereokanalen) gaan
alle zes de bussen door één ringbuffer en één resampler, zodat Pulse 1, Pulse 2, Wave en Noise
sample-exact gelijk lopen. Elke bus krijgt een eigen kanaalstrip en een eigen pluginuitgang, vóór
de EQ en effecten op de mix.

### 4. Een game besturen zonder toetsenbordfocus

Pluginvensters krijgen in DAW's niet betrouwbaar toetsaanslagen, en een game moet spelen terwijl
de muzikant op een MIDI-keyboard speelt. **Oplossing:** MIDI-noten en -CC's worden omgezet naar
doelen (pijlen, A, B, Start, Select, spatie, C, muis X/Y, klik; 13 in totaal). Bij een gb2swf-game
roept de plugin direct de interne functie `gbPad(mask)` van de game aan met alle ingedrukte
knoppen tegelijk, dus zonder toetsenbordfocus. Bij een gewone Flash-game maakt hij nep-KeyDown en
-KeyUp-events. Per doel telt een teller hoeveel bronnen het ingedrukt houden, zodat een MIDI-noot
en een muisklik elkaar niet loslaten. MIDI learn: knop aanklikken, toets of knop indrukken.

### 5. Alles bewaren in het DAW-project

Een plugin hoort na het heropenen van een project precies terug te komen. **Oplossing:** het pad
van de SWF, de MIDI-indeling en de savegames van de game (Flash SharedObjects, base64) zitten in
de pluginstatus en worden dus in het project opgeslagen. Na het laden van de status start de
engine dezelfde game opnieuw. De mixer van Ruffle wordt voor één samplerate gebouwd; verandert de
DAW die, dan wordt de game opnieuw gestart op de nieuwe rate.

## De live-test in FL Studio

**Opzet:** Windows 11, FL Studio 2024 (plugin wrapper versie 2.7.0.4597), de CLAP-versie van
swf2daw, FL Studio op 44,1 kHz (zo getoond in de lichtkrant van de plugin). Testgame:
`demo/tobu-tobu-girl-deluxe.swf` (vrije licentie, zie shotlijst.md).

**Verloop van de ontwikkeling op 20 oktober 2026** (uit de git-geschiedenis en de verzoeken van
de maker tijdens de test):

- 03:57: v0.1 op de Mac gebouwd en gepubliceerd (Ruffle als instrument, MIDI-besturing,
  DAW-audio, eerste skin).
- Daarna live op Windows getest in FL Studio. Wensen en problemen uit die test:
  - de plugin op een eigen track in plaats van op de master zetten;
  - opgenomen audio achteraf per track kunnen verdelen, effecten en muziek gescheiden;
  - EQ en eenvoudige geluidsopties in de plugin, met retro knoppen;
  - bij een gewone Flash-game leken de effecten niet te werken;
  - de interface overdreven retro, maar zonder nepknoppen (de nep-Mac-vensterknoppen moesten weg);
  - een resetknop voor aangepaste effecten;
  - de MIDI-mapping werkte niet goed en moest ook met de muis klikbaar zijn.
- Aanpassingen die daaruit volgden (commit "swf2DAW v0.2"): zes extra uitgangen per kanaal,
  het Music/SFX-systeem voor Flash-games, stems voor gb2swf, kanaalstrips en de EQ- en
  effectenrack, de nieuwe skin, klikbare MIDI-mapper, resetknoppen.

**Wat in de test voor dit pakket werkte** (zelf uitgevoerd op 20 oktober 2026, rond 18:35):

- FL Studio vond de plugin in de lijst ("swf2DAW, CLAP, 64, Burg Media") en laadde hem zonder
  fout. De wrapper meldt latency "none".
- De demo-SWF laadde via EJECT; de plugin herkende hem als Game Boy-game ("GAME BOY MODE").
- Muziek kwam direct uit de plugin: spectrum, VU-meters en de meter van het Music-kanaal bewogen
  mee. In de FL-mixer kwam alleen op het Music-kanaal signaal binnen, wat klopt voor deze game
  (gemaakt zonder stems).
- Met de knoppen in de MIDI MAPPER kwam de game van het titelscherm via het menu in het spel.
- De EQ-preset SMILE en de effecten Bitcrush, Echo en Space gingen aan zonder haperingen.
- "Auto map outputs" in FL Studio koppelde Pulse 1 tot en met SFX aan mixer-inserts 1 tot en met 6.

**Wat niet werkte of opviel:**

- Een muisklik van 60 ms op een knop in de MIDI MAPPER werd door de game niet opgemerkt; een
  klik van 400 ms wel. Vermoedelijk leest de game de knoppen één keer per gameframe, zodat een
  heel korte druk ertussendoor valt. Niet verder onderzocht.
- Toetsaanslagen die met Windows SendKeys naar het venster gestuurd werden, kwamen niet in de
  game aan. Met een echt toetsenbord is dat in deze test niet geprobeerd.
- De lichtkrant toonde tijdens het spelen tussen 17 en 35 FPS, terwijl de SWF 60 fps vraagt.
  Wat dat getal precies telt (gerenderde beelden, niet gamelogica) en of het spel daardoor
  trager liep, is niet gemeten. Zie Open vragen.
- Een opname rechtstreeks van een FL-mixertrack lukte niet zomaar: FL Studio vraagt dan om een
  audio-ingang ("Audio input needed"). Er is daarom geen screenshot van een opname in de playlist.

## Cijfers (alleen gemeten of geteld)

| Wat | Waarde | Hoe gemeten |
|---|---|---|
| Regels Rust-code in `src/` | 3296 | `wc -l src/*.rs` op de v0.2-commit |
| Grootte plugin (CLAP, Windows, release) | 22.431.232 bytes (ca. 21 MB) | bestandsgrootte van `target/bundled/swf2DAW.clap` |
| Automatiseerbare parameters | 70 | geteld in `src/params.rs` en `src/lib.rs` |
| Uitgangen | 7 stereo (1 mix + 6 kanalen) | `AUDIO_IO_LAYOUTS` in `src/lib.rs`, bevestigd in FL Studio |
| MIDI-doelen | 13 | `Target::ALL` in `src/mapping.rs` |
| Buffer gb2swf-audio | ca. 1 s, doel 60 ms, bijsturen max. 0,5% | constanten in `src/audio.rs` |
| Getoonde FPS tijdens het spelen | 17 tot 35 | afgelezen in de lichtkrant, meerdere screenshots |
| Latency volgens FL Studio | none | Settings-tab van de FL-wrapper |
| Geteste SWF-bestanden voor dit pakket | 1 | `tobu-tobu-girl-deluxe.swf` |
| Eerste build | 10 tot 20 minuten, enkele GB | volgens de README, niet opnieuw gemeten |

Er zijn geen metingen van CPU-gebruik, latency in milliseconden of laadtijd. Die getallen staan
hier dus bewust niet.

## Open vragen

1. Waarom toont de lichtkrant 17 tot 35 FPS bij een SWF van 60 fps? Telt hij alleen frames
   waarin iets nieuws getekend is, of loopt de game echt langzamer?
2. Hoeveel CPU gebruikt de plugin in FL Studio? Niet gemeten.
3. Is het korte-klikprobleem (60 ms) ook merkbaar met echte MIDI-noten, bijvoorbeeld bij snel
   staccato spelen?
4. Welke andere SWF's (gewone Flash-games, andere gb2swf-games) zijn met welk resultaat getest?
   Alleen de demo-game is voor dit pakket getest; eerdere tests gebruikten beschermde games.
5. Is in de eerdere live-test opgenomen via Edison of via de audio-ingang? De opnames in dat
   project heten "Insert 5" en "Insert 6", maar de precieze methode is niet vastgelegd.
