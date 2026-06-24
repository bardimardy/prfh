# Design: HUD-Inventar + Pickup-Animation + echtes Powerup-Spawn (W3 / #44)

- **Datum:** 2026-06-24
- **Status:** Entwurf (zur Freigabe)
- **Issue:** #44 — `feat(hud): Inventar-Overlay + Shadow-Highlight + Animationen + echtes Powerup-Spawn (W3)`
- **Baut auf:** Powerup-Spec `2026-06-22-powerup-inventory-effects-design.md` (§2 Look, §7 Cast,
  §8 HUD/Inventar), Welt-Base-Engine `2026-06-23-world-base-engine-design.md` (§4/§7 Arena +
  Render-Transform). #44 ist **Issue C + D** aus §12 jenes Docs, in einem PR.
- **Visuell validiert:** alle Looks in `examples/hud_lab.rs` **Szene 6** am lebenden Bild
  durchgespielt und vom User gewählt (3 Iterationen + unabhängiger Konsistenz-Review).

---

## 1. Ziel

Die UI-Schicht des Powerup-Systems live verdrahten und Wörter echt spawnen:

1. **Inventar-Overlay** als Panel (Showcase-Stil, §8) — oben rechts, wächst nach unten.
2. **Pickup-Animation** auf der neu eingesammelten Inventar-Zeile.
3. **Shadow-Autocomplete-Highlight** des getippten Cast-Prefixes.
4. **Echtes Spawnen** von Powerup-Wörtern in die `Arena` (Andockpunkt für spätere
   prozedurale Gen) + Ghost-Styling der nicht eingesammelten Map-Wörter.

Querschnitt-Anforderung (User): **Multiplayer-tauglich von Anfang an** — Animationen
spielen bei *allen* Spielern, Aktivierungen können später *andere* Spieler beeinflussen
(Schaden etc.), und das System muss **dynamisch erweiterbar** sein. Das prägt den
Architektur-Schnitt (§3), nicht den Funktionsumfang dieses PRs.

## 2. Gewählte Looks (eingefroren in Szene 6)

| Achse | Gewählt | Notiz |
|-------|---------|-------|
| Panel-Position | **oben rechts** (`Anchor::TopRight`) | Center würde über Cursor **und** zentriertem Cast-Ring (§7) liegen — bewusst vermieden. |
| Panel-Höhe | **dynamisch**, wächst nach unten mit der Item-Zahl | Header + N Zeilen + §8-Breathing-Rows. |
| Panel-Skin | **`rounded`** (`BorderType::Rounded`) | PANEL_BG-Füllung, blauer ACCENT-Titel ` POWERUPS `, je 1 PANEL_BG-Leerzeile über Header / unter Zeilen (§8). „erstmal" — andere Skins bleiben im Companion als Referenz. |
| Pickup-Animation | **`pop-pulse`** (Kombination Flash + Doppel-Puls) | Kurzer heller Pop-Flash beim Landen der neuen Zeile → zwei Hue-Pulse über `PICKUP_BASE` → setzt **präzise** auf Body-Grau (`TEXT`). ~0,60 s One-Shot. |
| Shadow-Highlight | **`box+dim`** | Getippter Prefix im Pink-Kasten (`HIGHLIGHT_BG/FG`), Rest lesbar; nicht-gematchte Zeilen gedimmt (`TEXT_DIM`, BG bleibt `PANEL_BG` → über scrollender Welt lesbar). |
| Aktivierungs-Welle | **`ManualRing`** (bereits im Spiel) | render-time-Math, scroll-immun — schon via `cast_wave`/`draw_cast_ring` verdrahtet (#43). |

### 2.1 Palette-Ergänzung (theme.rs)

Der Pop-Flash ist ein bewusster, vom User freigegebener **Look-Zusatz über §2 hinaus**
(§2 spezifizierte nur „gesättigte Basis → Body-Grau"). Das reinweiße Flash-Pixel war im
Companion ein roher `Color::Rgb(255,255,255)` — der Konsistenz-Review hat das als
Verstoß gegen die Single-Source-of-Truth-Palette markiert. **Fix:** neue Konstante
`PICKUP_FLASH` in `src/theme.rs` (warmes Off-White statt Reinweiß, z. B. `#FFF4E6`), die
sowohl der Game-Pickup als auch der Companion nutzen. Die Pop-Flash-Erweiterung wird
hier dokumentiert, damit sie nachvollziehbar bleibt.

## 3. Architektur: render-time-Animation + host-autoritativer Effekt-Seam

**Kern-Entscheidung:** Pickup- und Aktivierungs-Animation laufen als **render-time-Math
auf einem Timer-State in `App`** — *nicht* über `tachyonfx::EffectManager`. Begründung:

- Die im Companion gewählten Looks (präzises Landen auf Grau, transparenter Ring) sind
  render-time gerechnet; das Spiel macht das für die Aktivierung schon so
  (`cast_wave`/`draw_cast_ring`, #43) — wir spiegeln das exakt für den Pickup.
- **MP-Korrektheit (Learning #37):** Visuals werden **lokal pro Client** aus
  Spiel-Zustand/-Events berechnet, nie über Netz gesynct. Genau wie der Trail-Fade.
- Daher wird `EffectManager`/`process_effects` in `draw()` **bewusst NICHT** verdrahtet
  (YAGNI). Die tachyonfx-Konstruktoren (`effects::pickup()`/`activation()`) bleiben für
  künftige *buffer-gebundene* One-Shots verfügbar, sind aber nicht der #44-Pfad.

### 3.1 Der `EffectEvent`-Seam (MP-Naht, jetzt lokal)

Effekt-Trigger entstehen **aus beobachtbaren, host-autoritativen Spiel-Events**, nicht
aus der Eingabe-Behandlung. Ein zentrales Enum bündelt sie:

```text
enum EffectEvent {
    Pickup { slot: usize, name: String },   // Wort vollständig getract → ins Inventar
    Activation { tag: EffectTag, name: String },  // Cast-Dispatch
}
```

- **#44 (Single):** `on_char` (Trace `Completed`) und `dispatch_cast` **erzeugen** ein
  `EffectEvent`; `App` wendet es auf den lokalen Animations-Timer-State an
  (`pickup_anim` / `cast_wave`).
- **MP später (NICHT in diesem PR — „Seam jetzt, Draht später"):** der Host serialisiert
  `EffectEvent` in eine `ServerMsg`-Variante und broadcastet; jeder Client wendet
  dasselbe Event lokal an und spielt die Animation render-time. Protokoll wird hier
  **nicht** angefasst (kein `net-sync`-Ripple in #44). Der Seam macht das später additiv.
- **Aktivierung-beeinflusst-andere (Schaden etc.):** der `dispatch_cast`-`match
  effect_tag`-Hook ist die dynamische Andockstelle. Heute nur `Test` (Log + Welle).
  Künftige Tags tragen Spiel-Effekte; in MP wendet der **Host** sie autoritativ auf
  Ziel-Spieler an und broadcastet Zustands-Deltas. #44 baut nur den Hook + den
  Animations-Seam, keine konkreten Schadens-Effekte (vertagt, vgl. Powerup-Spec §9).

### 3.2 `App`-Felder (neu)

```text
pickup_anim: Option<PickupAnim>      // { age: Duration, slot: usize }
inv_visible: bool                    // manuelles Toggle (Default false)
cast_buffer / cast_mode / cast_wave  // existieren
```

`PickupAnim` trägt **keinen** Namen-String (der steht im Inventar bei `slot`) — der
Timer plus Slot-Index genügt der render-time-Math. `age` wird in `draw()` mit `elapsed`
fortgeschrieben (wie `cast_wave`) und nach Ablauf (~0,60 s) auf `None` geräumt.

## 4. Modul-Schnitt & berührte Dateien

| Datei | Änderung |
|-------|----------|
| `src/theme.rs` | neue Konstante `PICKUP_FLASH`. |
| `src/app.rs` | `pickup_anim`/`inv_visible`-Felder; `EffectEvent`-Erzeugung in `on_char` (Pickup) + `dispatch_cast` (bleibt); Anwenden auf Timer-State; Inventar-Toggle; `spawn_powerups`-Aufruf in `new_single` (echtes Spawn). |
| `src/render/mod.rs` | neues `draw_inventory` (Rounded-Skin, TopRight, dynamische Höhe, §8); `pickup_anim`-Fortschritt + render-time-Pickup-Math auf der Slot-Zeile; Shadow-Highlight (`box+dim`) im Auto-Pop bei Cast; Ghost-Styling der Map-Wörter verfeinern (Zeichnen bei ~`:288`). |
| `src/game/powerup.rs` (o. `arena.rs`) | `spawn_powerups(&mut Arena)` — platziert eine kleine feste Start-Menge `PowerupWord`-Entitäten; **host-autoritativer** Andockpunkt für spätere prozedurale Gen. |

**Kollisions-Fläche:** `app.rs` + `render/mod.rs` sind die Hotspots (CLAUDE.md). #44 ist
allein-stehend (kein paralleles Issue auf denselben Dateien außer #25/PR46, das andere
Pfade berührt) — Risiko gering, aber häufig `main` reinmergen.

## 5. Echtes Spawn + Ghost-Styling (Issue D)

- `spawn_powerups(&mut Arena)` ersetzt den heutigen `PRFH_DEBUG`-Einzel-Dash als
  **regulären** Spawn-Pfad: eine kleine, fest definierte Start-Menge Wörter an
  Map-Positionen. Single ruft es in `new_single`; in MP ist der **Host** der Spawner
  (seine `Arena` → bestehende `EntitySpawned`/Snapshot-Deltas tragen sie zum Client;
  kein neuer Protokoll-Code in #44). Das Test-Powerup (Powerup-Spec §10) bleibt optional
  unter `PRFH_DEBUG` als Validierungs-Vehikel.
- **Ghost-Styling:** nicht eingesammelte Wörter werden dezent/ghost gezeichnet
  (gedämpft, klar vom eigenen Trail unterscheidbar) — der im Companion (Szene 4)
  explorierte `WordStyle`. Verfeinerung der bestehenden Zeichnung in `render/mod.rs`.

## 6. Inventar-Sichtbarkeit (UX, Tippen bewegt!)

Buchstaben **schreiben Tiles** — ein Buchstaben-Hotkey fürs Toggle ist unmöglich.
Regel:

- Inventar **erscheint automatisch**, sobald es nicht leer ist, und **poppt** im
  Cast-Modus (mit Shadow-Highlight).
- Manuelles Toggle (Sichtbarkeit erzwingen/verstecken) über eine **Nicht-Buchstaben-
  Taste**; Default-Vorschlag `` ` `` (Backtick) — im Plan finalisieren. `Tab` ist Cast.

## 7. Testbarkeit (TDD wo sinnvoll)

Unit-testbar (Pflicht), reine Logik ohne Rendering:

- **`EffectEvent`-Erzeugung:** Trace-`Completed` in `on_char` erzeugt genau ein
  `EffectEvent::Pickup { slot, name }` mit korrektem Slot (= Index der neuen
  Inventar-Zeile) und Namen. Cast-Dispatch erzeugt `EffectEvent::Activation`.
- **`pickup_anim`-Lebenszyklus als reine Funktion der Zeit:** start → läuft → nach
  Ablauf `None` (analog `cast_wave`-Test). Slot bleibt stabil.
- **`spawn_powerups`:** spawnt die erwartete Menge/Positionen in die `Arena`
  (deterministisch, ohne `PRFH_DEBUG`).
- **Inventar-Sichtbarkeits-Logik:** leer → unsichtbar; nach Pickup sichtbar; Cast-Modus
  poppt; Toggle schaltet. Reine Zustands-Funktion.
- Vorhanden & weiter grün: `prefix_matches`/`get_exact` (Inventar), Trace-FSM,
  Cast-Dispatch.

Nicht unit-testbar (nur „läuft/zeichnet ohne Panik"): das render-time-Pickup-Math, das
Overlay-Zeichnen, der Shadow-Highlight, Ghost-Styling. Werden visuell im Companion
abgesichert; `main` wird **nicht** auf visuelle Korrektheit gegated. Die render-time-Math
(Hue-Lerp, Flash-Decay) kann optional als reine Funktion (Phase → Farbe) ausgelagert und
punktuell getestet werden.

## 8. Bewusst NICHT in diesem PR (vertagt)

- Protokoll-Änderung für MP-Effekt-Broadcast (Seam steht, Draht später).
- Konkrete Schadens-/Spiel-Effekte hinter `effect_tag` (nur Hook + `Test`).
- Prozedurale Powerup-Generierung (nur der `spawn_powerups`-Andockpunkt).
- `EffectManager`/`process_effects`-Live-Verdrahtung (render-time-Pfad gewählt).
- Andere Inventar-Skins außer `rounded` (bleiben im Companion).

## 9. Offene Detail-Punkte (im Plan zu fixieren)

- Konkrete Manuell-Toggle-Taste (Vorschlag `` ` ``).
- Exakte Start-Menge/Positionen in `spawn_powerups`.
- `PICKUP_FLASH`-Hex final.
- Ob die Pickup-render-time-Math als testbare reine Funktion ausgelagert wird.
