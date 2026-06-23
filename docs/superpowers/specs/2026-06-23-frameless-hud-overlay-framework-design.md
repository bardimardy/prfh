# Design: Frameless HUD + Overlay-Framework + dynamische Notifications

- **Datum:** 2026-06-23
- **Issue:** #39 · **Branch:** `issue-39` · **PR:** #40
- **Status:** Umgesetzt (Look im visuellen Companion validiert & eingefroren)
- **Scope:** Das **Fundament** für eine erweiterbare HUD-/Overlay-UI: frameless
  full-screen-Welt, ein anker-basiertes Overlay-Framework, ein dynamisches
  Notification-System (ersetzt das statische `trigger_banner`) und der
  `career.md`-Altlast-Cleanup. Konkrete Overlays/Animationen (Inventar-Panel,
  Pickup-/Wellen-Effekte) aus #31 setzen **darauf auf** (#31 → Consumer).

---

## 1. Ziel

Das Interface soll **rahmenlos** werden (die Welt nimmt den ganzen Screen ein,
HUD-Teile schweben als Overlays darüber) und eine **gute, modular erweiterbare
Basis** bekommen, auf der UI, Windows und Overlays sauber aufbauen. Die statische
Turn-„Notification" wird durch ein **dynamisches, animiertes Notification-System**
ersetzt. Altlasten (`/work/repo/career.md`-Titel) fallen weg.

## 2. Look & Feel (eingefroren über `examples/hud_lab.rs`)

Validiert im visuellen Companion (siehe §7), per Tastendruck A/B-verglichen:

- **Frameless:** keine `Borders`, kein `career.md`-Titel. Welt = `f.area()`.
- **HUD-Layout (Variante 1, „Ecken"):** `dir` oben-links, `combo` oben-rechts,
  Spielerliste unten-links, `[Esc] quit` unten-rechts. Akzentfarbe aus `theme.rs`.
- **Cursor:** **Block-Stil** — gefülltes Richtungs-Dreieck (`▲▼◀▶`), dunkle fg
  (`HIGHLIGHT_FG`) auf `ACCENT`-bg, fett. Mitspieler in ihrer Spielerfarbe.
- **Notification-Signatur (gewählte Kombi „full-fx + coalesce"):**
  1. **Rein:** Panel erscheint mit horizontaler `expand`-Welle aus der Mitte
     (`effects::notify_panel`, `CircOut`/Non-Overshoot).
  2. **Text:** sammelt sich per `coalesce` (`effects::notify_reveal`).
  3. **Halten:** kurze Standzeit.
  4. **Raus:** **center-in Collapse** — Panel **und** Text ziehen sich zur Mitte
     zusammen und geben die Welt darunter frei (reine Geometrie, schneller als rein).
- **Typ-getriebene Größe:** `Info` = 1 Zeile (häufige Hinweise), `Event`/`Major`
  = 2-Zeilen-Karte (Titel + Detail). Gemischte Größen stapeln vertikal oben-mitte.

Tempo (eingefroren): Build 240 ms · Text 260 ms · Hold 1500 ms · Collapse 140 ms.

## 3. Architektur & Modul-Schnitt

| Modul | Zweck |
|-------|-------|
| `src/hud/mod.rs` | **Neu.** Anker-Overlay-Framework: `Anchor` (TopLeft … Center) + `anchor_rect(area, anchor, w, h)`. Geklemmt/saturierend, getestet. |
| `src/hud/notify.rs` | **Neu.** `NotifyKind` (Info/Event/Major), `Notification`, `NotificationStack`. Lebenszyklus = reine Funktion des Alters; hält pro Notification die tachyonfx-Effekte. |
| `src/effects/mod.rs` | **+** benannte Konstruktoren `notify_panel(ms)` (über `safe_expand`/`CircOut`) und `notify_reveal(ms)` (`coalesce`), je mit Panik-Smoke-Test. |
| `src/render/mod.rs` | Frameless `draw(f, &mut App, elapsed)`: `draw_world` (full-screen, Block-Cursor), `draw_hud` (Layout 1 an Ankern), Notification-Render, Death-/Debug-Overlay. `process_effects`-Hook bleibt (für #31). |
| `src/app.rs` | `trigger_banner`/`_ticks` raus → `pub notifications: NotificationStack`. `on_char` pusht Turn/Stop als `Info`. |
| `src/main.rs` | `elapsed` pro Frame messen, an `draw` durchreichen (alle 3 Loops: Single/Host/Client). |
| `examples/hud_lab.rs` | **Neu.** Visueller Companion (wegwerfbar, isoliert). |

### Warum die Notification-Animation **nicht** rein über `process_effects`/EffectManager läuft
Die Notifications **sind** der frame-persistente State (sie leben im Stack über
Frames). Jede hält ihre eigenen `Option<Effect>` und prozessiert sie beim Rendern
mit der Frame-`elapsed`. Der generische `EffectManager`/`process_effects`-Hook
bleibt für die **diskreten, keyed** One-Shot-Effekte aus #31 (Pickup, Welle). Die
**Phasen-Geometrie** (center-out/center-in, Breiten-Faktor) ist Render-Mathematik
als Funktion des Alters — analog zum Trail-Fade (Learning #37): zeit-/positions-
basierte Render-Logik gehört nicht in den zell-gebundenen Effekt-Graphen.

### `draw` ist jetzt `&mut App` + `elapsed`
Tachyonfx-Effekt-Verarbeitung mutiert Effekt-State **und** Buffer gemeinsam und
braucht die Frame-`elapsed`. Deshalb ist `draw` zeitgetrieben. `world_view()`
liefert ein **owned** `WorldView` → die immutablen Reads (Welt, HUD) laufen vor
dem mutablen `notifications.render(...)` ab, kein Borrow-Konflikt.

## 4. Erweiterbarkeit (das eigentliche Ziel)

- **Neues HUD-Element:** an einem `Anchor` mit `anchor_rect` platzieren + rendern —
  keine Layout-Operation, kein Umbau bestehender Teile.
- **Neuer Notification-Typ:** `NotifyKind`-Variante + Höhe/Akzent; der Stack mischt
  Größen automatisch.
- **Reservierte Slots:** pace/day/doubt/Boss/Objective docken später als weitere
  Overlays an freien Ankern an; das Inventar-Panel (#31) ist ein Center-Overlay.

## 5. Effekt-Norm-Einhaltung

- `expand` ausschließlich über `effects::safe_expand` (`CircOut`, Non-Overshoot) —
  die HARTE Panik-Regel der `effects`-Skill. Beide neuen Konstruktoren haben einen
  „bis über die Timer-Dauer hinaus prozessiert, ohne Panik"-Smoke-Test.
- Farben ausschließlich aus `src/theme.rs`.

## 6. Testbarkeit

Unit-getestet (Pflicht):
- `anchor_rect`: Ecken/Center korrekt, Übergrößen geklemmt (kein Panic).
- Notification-Lebenszyklus: Phasen folgen dem Alter, gemischtes Stacking rendert
  ohne Panic, Text erscheint nach dem Aufbau, Entfernung nach Gesamtdauer.
- Effekt-Smoke-Tests (`notify_panel`/`notify_reveal`): kein Panic über die Dauer.
- Render: frameless (kein `career.md`, keine Box-Ecken), nur `dir`+`combo` in der
  Topbar, `last_event` nur im Debug-Overlay, Quit-Hinweis vorhanden, **voll
  verdrahteter Pfad** (Turn → Notification → 60 Frames `draw`) ohne Panic.

Nicht testbar (visuell): exaktes Aussehen der tachyonfx-Effekte, Overlay-Layout.

## 7. Visueller Companion (`examples/hud_lab.rs`)

Eigenständiger `cargo`-Example, **null Einfluss aufs Hauptspiel**, teilt
`prfh::theme` + `prfh::hud` + `tachyonfx`. A/B per Tastendruck (Render-Modus
manual/hybrid/full-fx, Text-Reveal coalesce/sweep+glow/fade, Cursor-Stile,
Layouts, Frames an/aus). War das Werkzeug, mit dem die Signatur in §2 entschieden
wurde. Details + Erweiterungs-Rezept: Skill `.claude/skills/visual-companion/`.

## 8. Offene Punkte / Folgen

- #31 wird re-scoped: konkrete Overlays/Animationen (Inventar-Panel,
  Pickup-/Wellen-Effekte) setzen auf diesem Framework auf.
- Companion bleibt als Design-Sandbox erhalten (Doku des Prozesses + Basis für die
  nächste visuelle Frage).
