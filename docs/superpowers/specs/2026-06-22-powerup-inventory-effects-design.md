# Design: Powerup- & Inventar-Base-Engine + Effekt-Layer + HUD-Überarbeitung

- **Datum:** 2026-06-22
- **Status:** Entwurf (zur Freigabe)
- **Scope:** Die *Base-Engine* für Powerups + Inventar, ein wiederverwendbarer
  Effekt-/Animations-Layer und eine HUD-Überarbeitung. **Konkrete Powerups
  (Dash etc.) sind bewusst vertagt** — gebaut wird nur die Engine, die sie
  später trägt, plus ein Test-Powerup als Validierungs-Vehikel.

Dieses Dokument wurde mit einem Review-Subagent gegengeprüft; die kritischen
Befunde (fehlendes Welt-Modell, Trace-vs-Engine-Konflikt, Upgrade-Sequencing,
Namens-/Trigger-Kollision) sind unten aufgelöst.

---

## 1. Ziel

Das Spiel soll dynamischer werden: man sammelt Powerups als **Wörter auf einer
Map** ein (durch korrektes Nachschreiben), sie landen im **Inventar**, und man
**aktiviert** sie später durch Tippen ihres Namens. Dazu braucht es einen
zentralen **Effekt-Layer** (Pickup-Animation, Aktivierungs-„Welle", Highlights)
und eine aufgeräumte **HUD** mit einem Inventar-Overlay.

## 2. Look & Color-Scheme (eingefroren)

Validiert über einen wegwerfbaren Rust-Companion (ratatui+tachyonfx). Ergebnis:

- **Pickup/Hinzufügen:** Eintrag fliegt von rechts in seinen Slot (`slide_in ←`)
  und durchläuft dabei einen **Regenbogen** (volle 360°-Hue-Rotation über
  gesättigter Basis). Danach setzt er sich auf neutrales Body-Grau.
- **Aktivierung:** **Welle nach außen** — `expand` von der Mitte (`CircOut`) +
  Hue-Pop. *Fest übernommen* als Aktivierungs-Signatur.
- **Typing-Highlight:** schlichter **farbiger Background-Kasten** auf dem
  getippten Prefix; der Rest des Worts bleibt klar lesbar (keine wuselnde
  Ghost-Schicht — „passiert sonst zu viel").

**Palette** (`src/theme.rs`, Dark-Mode-Terminal vorausgesetzt, eine Quelle der
Wahrheit für alle Farben):

| Konstante      | Wert        | Verwendung                                         |
|----------------|-------------|----------------------------------------------------|
| `ACCENT`       | `#5AA9FF`   | Blau — HUD/Overlay-Text, Überschriften, Akzente    |
| `HIGHLIGHT_BG` | `#FF49A0`   | Pink — Highlighting (getippter Prefix)             |
| `HIGHLIGHT_FG` | `#141012`   | dunkler Text auf dem Pink-Kasten                   |
| `PANEL_BG`     | `#26262B`   | Panel-/Overlay-Füllung                             |
| `PICKUP_BASE`  | `#FF4080`   | gesättigte Basis für den Pickup-Regenbogen         |
| `TEXT`         | `#C8CCD4`   | lesbarer Body-Text                                 |
| `TEXT_DIM`     | `#6A6E78`   | gedämpfter Text, Borders                           |
| `DANGER`       | `#E54B4B`   | Warn-/Fehlerakzent                                 |

## 3. tachyonfx — Effekt-Library

- **Crate:** `tachyonfx` 0.25 (Effekt-/Animations-Lib der ratatui-Org; Repo
  `junkdog/tachyonfx`). Effekte bearbeiten den Frame-Buffer **nach** dem
  Widget-Rendering, getrieben von der pro-Frame-`elapsed`-Duration
  (`EffectManager::process_effects(elapsed.into(), buf, area)`).
- **⚠️ Versions-Zwang:** tachyonfx 0.25 verlangt **ratatui 0.30 / crossterm
  0.29** (`ratatui-core` 0.1). Das Repo ist auf **0.28** → ein breaking Upgrade
  ist Pflicht (→ Issue 0).
- **⚠️ Verifizierter Panic:** `expand`/`stretch` **paniken bei Overshoot-Easings**
  (`BackOut`, `ElasticOut`, `Back*` …) — Subtraktions-Overflow in
  `stretch.rs:56`. **Regel:** für `expand` nur Non-Overshoot-Kurven (`CircOut`,
  `QuadOut`, `SineOut`, `CubicOut`). Wird im `effects`-Modul gekapselt + per
  Smoke-Test abgesichert (Effekt bis zum Ende durchlaufen lassen, darf nicht
  paniken).

Verifizierte Bausteine: `slide_in(Motion, gradient_len, randomness, color, timer)`,
`hsl_shift(Some([h,s,l]), None, timer)`, `expand(ExpandDirection, Style, timer)`,
`coalesce`, `parallel(&[..])`, `sequence(&[..])`, `repeat(e, RepeatMode)`,
`ping_pong`, `delay`. `EvolveSymbolSet`/`RepeatMode` liegen unter `tachyonfx::fx`.

## 4. Architektur & Modul-Schnitt

| Modul                  | Zweck |
|------------------------|-------|
| `src/theme.rs`         | Dark-Mode-Palette (s. o.). Single source of truth. |
| `src/effects/`         | Dünner tachyonfx-Wrapper: benannte Effekt-Konstruktoren (`pickup()`, `activation()`, …) + Non-Overshoot-Regel + `process_effects`-Hook im Render. |
| `src/game/world.rs`    | **Neu.** Welt-/Map-Modell: vorplatzierte Entitäten mit Positionen (zunächst nur `PowerupWord`). |
| `src/game/powerup.rs`  | `Powerup { id, name, effect_tag }`, `PowerupWord` (Layout auf der Map), Spawn-Logik, Test-Powerup. |
| `src/game/inventory.rs`| `Inventory(Vec<Powerup>)`, Prefix-Matching, Aktivierungs-Dispatch. |
| `src/game/writing.rs`  | Erweiterung: **Trace-FSM** als *Beobachter* von `on_char` + Trigger-Suspendierung während Trace/Cast. `Direction` bleibt 4-Wege. |
| `src/app.rs`           | Verdrahtung: Trace, Inventar, Cast-Modus, Overlay-Auto-Pop. |
| `src/render/mod.rs`    | Schlanke HUD, Welt-/Powerup-Rendering, Inventar-Overlay, Effekt-Hook. |

> **Warum ein Welt-Modell?** Heute ist die „Welt" nur der eigene Trail,
> zentriert auf den Cursor gerendert — es gibt keine vorplatzierten Tiles. Die
> Pickup-Mechanik setzt aber Wörter voraus, die *vor* dem Erreichen auf der Map
> liegen. Das Welt-Modell ist damit das eigentliche Fundament (Teil von Issue B).

## 5. Welt-Modell & Powerup-Wort-Layout

```text
PowerupWord {
    id:       PowerupId,
    name:     String,          // logisches Wort, z.B. "dash"
    origin:   (i32, i32),      // Position des Tiles mit dem kleinsten Koordinatenwert auf der Achse
    axis:     Axis,            // Horizontal | Vertical
    reversed: bool,            // ob die Buchstaben rückwärts auf der Achse liegen
}
```

- Die belegten Tiles sind `p_0 .. p_{n-1}` entlang der Achse (aufsteigende
  Koordinate ab `origin`).
- **Reversed-Regel (eindeutig, vgl. Review S4):** Der Spieler tippt **immer das
  logische Wort** `name`. Die Engine bildet den *k*-ten Tastenanschlag auf das
  korrekte Tile ab — abhängig von der Traversier-Richtung und `reversed`.
  „Rückwärts gelegt" betrifft nur **Platzierung/Rendering**, nie das, was getippt
  wird. Beide Orientierungen werden per Unit-Test abgedeckt.

Rendering: nicht eingesammelte Powerup-Wörter werden als Tiles auf dem Grid
gezeichnet (dezent/ghost-styled), zusätzlich zum Trail.

## 6. Pickup: Räumliches Arming-Trace (FSM)

Der Trace ist eine **beobachtende** State-Machine — sie steuert nicht selbst,
sondern inspiziert jedes von `on_char` geschriebene Tile (Position, Zeichen,
aktuelle Richtung). Damit kollidiert sie nicht mit dem `on_char`-Modell (jeder
Buchstabe schreibt + bewegt in Laufrichtung).

**Zustände:** `Idle → Tracing { word_id, progress } → {Completed | Reset}`

- **Idle → Tracing (Arming, rein räumlich):** Beim Schreiben eines Tiles wird
  geprüft:
  1. Die geschriebene Position ist das **Eintritts-Tile** eines Powerup-Worts
     (das der Traversier-Richtung entsprechende Endtile), **und**
  2. die aktuelle **Laufrichtung liegt parallel zur Wort-Achse** und zeigt in das
     Wort hinein, **und**
  3. das geschriebene Zeichen == erster erwarteter **logischer** Buchstabe.

  Sind alle drei erfüllt → `Tracing { progress: 1 }`. Der auslösende Buchstabe
  wird **als Buchstabe 0 konsumiert** (und ganz normal als Tile geschrieben).
- **Tracing (pro weiterem Tile):**
  - Erwartetes Tile = Wort-Tile bei Index `progress` (per Traversier-Richtung
    gemappt); erwarteter Buchstabe = `name[progress]`.
  - Position **und** Zeichen stimmen → `progress += 1`. Bei `progress == n` →
    **Completed**: Powerup wandert ins Inventar, Wort despawnt, Pickup-Animation
    (Issue C) spielt. Zurück zu `Idle`.
  - Sonst (Spieler ist von der Achse abgebogen ⇒ falsche Position, oder falscher
    Buchstabe) → **Reset**: Trace bricht ab, zurück zu `Idle`. Das bereits
    geschriebene Tile bleibt; `current_word` wird geleert (kein stale Trigger).
    Trigger-Erkennung ist ab dem **nächsten** Zeichen wieder scharf.
- **Trigger-Suspendierung:** Während `Tracing` ist die Sofort-Trigger-Erkennung
  ausgesetzt — die eigenen Wort-Buchstaben (`up`, `stop`, …) dürfen nicht feuern.
  Da Trigger ausgesetzt sind, kann der Spieler **während** des Trace die Richtung
  nicht ändern: der Cursor läuft die Achse automatisch ab, man muss nur die
  richtigen Buchstaben tippen. Die *räumliche* Leistung liegt im **Positionieren
  + Ausrichten vor** dem Wort (Arming-Bedingung) — das ist „voll-räumlich".

## 7. Aktivierung: Eigener Cast-Modus

Aktivierung läuft über einen **dedizierten Modus** (statt Tippen im normalen
Fluss), damit beliebige Powerup-Namen erlaubt sind und nicht mit den
Sofort-Triggern kollidieren (vgl. Review S5; z.B. würde „update" sonst `up`
feuern).

- Eine **Umschalttaste** (Default `Tab`, finalisierbar) betritt/verlässt den
  Cast-Modus.
- Im Cast-Modus: Tastenanschläge **schreiben keine Tiles und bewegen den Cursor
  nicht**, sondern füllen einen `cast_buffer`. Trigger-Erkennung ausgesetzt.
- Das **Inventar-Overlay poppt automatisch auf**; der gematchte Prefix eines
  Eintrags wird mit dem Pink-Kasten gehighlightet (lesbarer Rest).
- `cast_buffer == Name eines Inventar-Powerups` → **Aktivierungs-Dispatch** +
  Wellen-Animation; Cast-Modus endet. `Esc`/Umschalttaste verlässt den Modus
  ohne Auslösen.

> Echte Aktivierungs-Effekte (Dash etc.) sind vertagt — jetzt nur der
> **Dispatch-Hook** (`match effect_tag { … }`, der vorerst nur loggt/Welle
> spielt) und das Test-Powerup als Dummy.

## 8. HUD-Überarbeitung

- **Schlanke Topbar:** nur `dir` + `combo`.
- **Raus:** `day`, `doubt` aus der HUD, großes Titel-Banner, verbose
  Trigger-Hilfe, `last_event`-Zeile → wandert ins Debug-Overlay (`PRFH_DEBUG`).
- **Inventar = Overlay-Panel** (Showcase-Stil: `PANEL_BG`, blauer Akzent-Titel,
  Padding; 1 BG-Zeile über dem Header, 1 darunter). Per Hotkey toggelbar und
  **automatisch aufpoppend**, sobald der Cast-Modus aktiv ist und ein Name matcht.

## 9. Dash & Diagonalen — nur „designed-for"

Vertagt. Keine Vorab-Verallgemeinerung der FSM oder des `Direction`-Enums
(bleibt konkret 4-Wege; ein 4-Varianten-Enum später zu erweitern ist billig).
Festgehalten ist nur die *Absicht*: spätere Powerups brauchen einen Armed-State
(`dash` schärft → nächster Richtungs-Input feuert den Vektor) und Diagonalen.
Der Cast-Modus (§7) ist bereits die natürliche Andock-Stelle dafür.

## 10. Test-Powerup

- Spawnt **neben dem Spieler-Start**, **nur unter `PRFH_DEBUG`**, mit Dummy-Effekt
  (Banner „collected/activated"). Validiert den ganzen Flow Pickup → Inventar →
  Cast → Dispatch.
- **Follow-up-Issue** „Test-Powerup entfernen/ersetzen" wird angelegt.

## 11. Testbarkeit (TDD wo sinnvoll)

Unit-testbar (Pflicht):
- Inventar-Prefix-Match („ist Buffer Prefix eines Powerup-Namens?").
- Powerup-Wort-Layout: aus `origin`/`axis`/`reversed` die Tile-Positionen **und**
  das Keystroke→Tile-Mapping (beide Orientierungen — testet §5).
- Trace-FSM-Übergänge: Arming / advance / falscher-Buchstabe-Reset /
  Abbiegen-Reset / Complete → Inventar. Reine Logik (FSM beobachtet
  Zeichen+Richtung, kein Rendering).
- Aktivierungs-Dispatch: voller Name feuert das richtige `effect_tag`
  (über einen aufgezeichneten Dispatch, ohne Rendering).
- Namens-Validierung im Cast-Kontext (beliebige Namen erlaubt).

Nicht unit-testbar (höchstens „konstruiert ohne Panic"-Smoke-Test): alles in
tachyonfx (visuell, zeitgetrieben), der Post-Render-Hook, Overlay-Rendering. Die
Non-Overshoot-Regel wird als Smoke-Test kodiert. `main` wird **nicht** auf
visuelle Korrektheit gegated.

## 12. Issue-Schnitt & Sequencing

Ein gemeinsames Design-Doc (dieses), daraus **6 Issues**. Reihenfolge minimiert
Merge-Kollisionen (zwei Claude-Instanzen, `app.rs`/`render/mod.rs` sind die
Hotspots):

```
0  →  0b  →  ( A ‖ B )  →  C  →  D
```

| Issue | Inhalt | Berührt v.a. | Abhängt von |
|-------|--------|--------------|-------------|
| **0** | **Upgrade** ratatui 0.28→0.30, crossterm 0.28→0.29. **Kein** Verhaltenswechsel, nur grün halten. Atomar, **zuerst**. | `Cargo.toml`, `main.rs`, `render/mod.rs`, `app.rs` (Imports) | — |
| **0b**| Schlanke `App`/HUD-Felder (Clutter raus, `last_event`→Debug) + `src/theme.rs`. | `app.rs`, `render/mod.rs`, neu `theme.rs` | 0 |
| **A** | Effekt-Layer: `tachyonfx`-Dep, `src/effects/`-Wrapper + Non-Overshoot-Regel + Smoke-Test, `process_effects`-Hook im Render. | `Cargo.toml`, `render/mod.rs`, neu `effects/` | 0b |
| **B** | Welt-Modell + Powerup + Inventar + Trace-FSM + Cast-Dispatch. | neu `world.rs`/`powerup.rs`/`inventory.rs`, `writing.rs`, `app.rs` | 0b |
| **C** | HUD/Overlay-UI: Inventar-Panel, Shadow-Highlight, Pickup-/Wellen-Animationen verdrahtet. | `render/mod.rs`, `app.rs` | A, B |
| **D** | Test-Powerup hinter `PRFH_DEBUG` + Follow-up-Remove-Issue. | `powerup.rs`/Spawn, `app.rs` | B |

**Kollisions-Vermeidung:** A bleibt in `render`/`effects`/`Cargo`, B im
Game-Logik-Layer + `app.rs`. B landet ein kleines **`App`-Skelett** (Felder +
Stub-Methoden), das C/D nur erweitern, statt dieselbe Methode mehrfach
umzuschreiben. Jede Instanz bekommt im Issue genannt, welche `App`-Felder/Methoden
ihr „gehören".

## 13. Offene Detail-Punkte (im Plan/Issue zu fixieren)

- Konkrete Cast-Modus-Umschalttaste (Default `Tab`).
- Hotkey für manuelles Inventar-Toggle.
- Genaues Ghost-Styling der nicht eingesammelten Map-Wörter.
