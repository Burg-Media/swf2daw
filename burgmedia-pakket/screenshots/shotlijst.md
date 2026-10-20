# Shotlijst

Alle beelden zijn op 20 oktober 2026 gemaakt op Windows 11 in FL Studio 2024, in een nieuw,
leeg project (template "Empty"), op een scherm van 1920 bij 1080.

**Bewerking:** alleen de Windows-taakbalk is eraf geknipt (onderste 48 pixels), zodat elk beeld
1920 bij 1032 pixels is. Verder niets aangepast: geen retouche, geen schaling.
**Privacy:** de werkbalk van FL Studio is verborgen (Ctrl+F11), omdat die de naam van de
geregistreerde gebruiker toont. Er staan geen mappaden, gebruikersnamen of andere vensters in beeld.
Het venster voor het kiezen van een bestand is bewust niet gefotografeerd.

## Bron van het testmateriaal

- **Game in beeld en geluid:** Tobu Tobu Girl Deluxe, (c) 2017 Tangram Games (geluid door
  potato-tan). Code onder de MIT-licentie, beeld, tekst, geluid en muziek onder **CC BY 4.0**.
  Origineel: tangramgames.itch.io, broncode: github.com/SimonLarsen/tobutobugirl-dx. Ongewijzigde
  ROM, met gb2swf omgezet naar SWF. Het bestand staat in de repo als
  `demo/tobu-tobu-girl-deluxe.swf`, met credits in `demo/CREDITS.txt`.
- **CC BY 4.0 vraagt naamsvermelding.** Zet bij publicatie van deze beelden bijvoorbeeld:
  "Game: Tobu Tobu Girl Deluxe door Tangram Games, CC BY 4.0".
- **Software in beeld:** FL Studio 2024 (Image-Line) en swf2daw (Burg Media). Er staan geen
  samples of andere muziek in beeld: het project is leeg op swf2daw en een lege Sampler na.

## Bestanden

| Bestand | Wat erop staat |
|---|---|
| `01-fl-plugin-titelscherm.png` | swf2daw in FL Studio met het titelscherm "tobu tobu girl deluxe, PRESS START". Lichtkrant "NOW PLAYING", spectrum en VU-meters bewegen, tabblad CHANNEL MIXER open met signaal op Music. |
| `02-fl-gameplay-midi-mapper.png` | Midden in het spel (level Plains). Tabblad MIDI MAPPER met de 13 knoppen en hun standaardnoten (C4 links, E4 rechts, G4 Start enzovoort). |
| `03-fl-kanaalmixer-in-plugin.png` | Zelfde spelmoment, tabblad CHANNEL MIXER: zes kanaalstrips (Pulse 1, Pulse 2, Wave, Noise, Music, SFX) met low/mid/high, level, pan, mute, solo. Alleen Music heeft signaal (game zonder stems). |
| `04-fl-eq-en-effecten.png` | Tijdens het spel, tabblad EQ + EFFECTS: 10-bands EQ op preset SMILE, filterpad, en de effecten Bitcrush, Echo en Space aan. |
| `05-fl-plugin-geladen-leeg.png` | De plugin net geladen, nog zonder SWF: scherm "INSERT DISC, press EJECT to load a .swf", lichtkrant "NO DISC". Geen foutscherm; dit is de beginstand. |
| `06-fl-plugin-kiezen.png` | FL Studio, venster "Select generator plugin": swf2DAW bovenaan, formaat CLAP, 64 bit, maker Burg Media. Laat zien dat het een gewone plugin in de lijst is. |
| `07-fl-uitgangen-naar-mixer.png` | FL-wrapper van swf2daw, tab Processing: de zeven uitgangen (Output, Pulse 1, Pulse 2, Wave, Noise, Music, SFX), na "Auto map outputs" gekoppeld aan mixer-inserts 1 tot en met 6. |
| `08-fl-mixer-music-kanaal.png` | FL-mixer: alleen Insert 5 (Music) en de Master krijgen signaal, wat klopt voor deze game. De inserts hebben nog hun standaardnaam, niet de kanaalnamen. |

## Hoofdbeeld

**Mijn keuze: `01-fl-plugin-titelscherm.png`.** Het titelscherm is meteen herkenbaar als game,
de retro-interface komt goed uit, en de bewegende meters laten zien dat er geluid uit de plugin
komt, met FL Studio eromheen.
Tweede keus voor een technischere invalshoek: `04-fl-eq-en-effecten.png` (spel plus effectenrack).

Tip voor uitsnijden: de plugin zelf staat in alle pluginbeelden op ongeveer x 140 tot 1145,
y 225 tot 1020. Voor een strakke uitsnede van alleen de plugin is dat het gebied.

## Wat ontbreekt

- **Voor/na (bron-SWF naast resultaat):** niet gemaakt. swf2daw zet niets om; de SWF speelt in de
  plugin zelf. Een eerlijke vergelijking zou de game in een browser naast de game in FL Studio
  zijn; die is niet gemaakt.
- **Opname in de playlist / piano roll:** niet gelukt in deze sessie (FL Studio vroeg om een
  audio-ingang bij het opnemen van een mixertrack, zie techniek.md). Er is dus geen beeld van
  opgenomen audio of MIDI in de playlist.
- **CLI-uitvoer of "conversie bezig":** bestaat niet; swf2daw heeft geen CLI en geen conversiestap.
- **Demo-filmpje (demo/):** niet gemaakt.
- **2x-resolutie:** het scherm is 1920 bij 1080 op 100%, dus de beelden zijn 1920 breed, niet 2x.
