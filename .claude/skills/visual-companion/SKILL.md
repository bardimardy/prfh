---
name: visual-companion
description: Der visuelle Rust-Companion (examples/hud_lab.rs) für prfh — eine wegwerfbare, vom Hauptspiel isolierte Sandbox zum Explorieren von HUD/Overlay/Notification/Cursor-Looks mit echtem ratatui+tachyonfx-Code. Use IMMER, wenn der User visuell/UX/Design an der UI arbeiten will (HUD, Overlay, Window, Panel, Notification, Cursor, Animation, „wie sieht das aus", „probier mal", Layout-Varianten, Look&Feel, A/B von Effekten) ODER tachyonfx-Effekte visuell vergleichen will. Schlägt dem User den Companion proaktiv vor, erklärt ihn kurz und weiß genau, wie er läuft, aufgesetzt und erweitert wird.
---

# Visual Companion (`examples/hud_lab.rs`)

Ein **eigenständiger, wegwerfbarer Rust-Build**, mit dem visuelle Konzepte für
`prfh` exploriert werden — **bevor** sie ins Spiel wandern. Er teilt sich die
echten Crate-Bausteine (`prfh::theme`, `prfh::hud`, `tachyonfx`), läuft aber als
`cargo`-**Example** und **beeinflusst das Hauptspiel in keiner Weise**.

Eingeführt mit Issue #39 (frameless HUD + dynamische Notifications). Er war das
Werkzeug, mit dem die finale Notification-Signatur und der Cursor-Stil **am
lebenden Bild** entschieden wurden, statt sie zu erraten.

## Wann diesen Skill anwenden — und was tun

Sobald der User an der **visuellen/UX-Seite** der UI arbeitet (neuer Overlay,
HUD-Element, Notification-Variante, Cursor-/Animations-Look, Layout-Frage,
„wie wirkt das?", A/B von tachyonfx-Effekten):

1. **Schlage den Companion proaktiv vor**, bevor du Spiel-Code anfasst. Visuelle
   Entscheidungen trifft man am Bild, nicht in der Vorstellung — und die
   `draw(&App)`-Render-Schicht ist mühsam zum Durchprobieren, der Companion nicht.
2. **Erkläre ihn in einem Satz** (siehe unten) und **biete an**, eine Variante
   darin zu bauen, die der User dann per Tastendruck vergleicht.
3. **Iteriere im Companion**, bis der User eine Variante wählt. Erst **dann** die
   gewählte Signatur ins Spiel (`src/hud/`, `src/render/`, `src/app.rs`) verdrahten.

Der Companion ist **kein** Pflicht-Gate — bei rein logischen/mechanischen UI-Änderungen
(Hotkey umlegen, Text ändern) überspringen. Maßstab: *Würde der User das lieber
sehen als beschrieben bekommen?* Dann Companion.

## Vorschlags-Formulierung (Beispiel)

> „Das ist eine visuelle Entscheidung — die triffst du am besten am Bild. Ich
> habe dafür den `hud_lab`-Companion (`cargo run --example hud_lab`): eine
> isolierte Sandbox, die das Hauptspiel nicht anfasst. Ich baue dir die
> Varianten da rein, du schaltest per Taste durch und sagst, was zündet. Soll ich?"

## So läuft er

```sh
cargo run --example hud_lab
```

Falls `cargo` nicht im PATH ist (häufig in dieser Umgebung):
```sh
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
# oder:
rustup run stable cargo run --example hud_lab
```

Er öffnet einen Vollbild-Terminal-Screen mit einer scrollenden „Welt" als
Hintergrund (damit man Overlays über **bewegtem** Inhalt sieht) und einer
Steuerleiste unten. Beenden mit `q` / `Esc`.

## Aufbau (wie er gebaut ist)

- **Ein einzelnes `examples/hud_lab.rs`** — kein eigenes Crate, kein `[[bin]]`.
  `cargo run --example hud_lab` baut nur diesen Example gegen die `prfh`-lib.
- **Teilt echte Bausteine:** importiert `prfh::theme` (Palette, single source of
  truth) und `prfh::hud::{Anchor, anchor_rect}` (das Anker-Overlay-Framework) und
  `tachyonfx::fx`. So sieht man **denselben** Look wie im Spiel, ohne Spiel-State.
- **Eigener, isolierter State** (`struct State`) + eine eigene `Notif`/Szenen-
  Repräsentation — **bewusst getrennt** von `prfh::hud::notify`, damit man im
  Companion frei experimentiert, ohne die Spiel-Typen zu verbiegen.
- **A/B per Tastendruck:** Varianten liegen als `enum` mit `next()` vor (z. B.
  `RenderMode`, `Reveal`, `CursorStyle`); eine Taste zykelt sie live durch. Das
  ist das Kern-Muster — neue Optionen = neue enum-Variante + ein `match`-Arm.
- **Steuerleiste** unten zeigt immer den aktuellen Zustand aller Schalter.

Typische Schalter (Stand #39, können sich entwickeln — immer die Steuerleiste
im laufenden Companion als Wahrheit nehmen):

| Taste | Wirkung |
|---|---|
| `1`/`2`/`3` | Frameless-Layout-Vorschlag wechseln |
| `n` | Notification feuern (rotiert Typen → gemischtes Stacking sichtbar) |
| `m` | Render-Modus der Notification (z. B. manual / hybrid / full-fx) |
| `i` | Text-Reveal-Stil (z. B. coalesce / sweep+glow / fade) |
| `c` | Cursor-Stil (block / chevron / pulse / comet) |
| `v` | Inventar-Overlay-Demo ein/aus |
| `f` | Frames (Rahmen) an/aus — frameless vs. gerahmt vergleichen |
| `←↑↓→` | Laufrichtung (für Cursor-Ansicht) |
| `q`/`Esc` | beenden |

## Eine neue Variante hinzufügen (Rezept)

1. Enum für die Design-Achse finden oder anlegen (z. B. `CursorStyle`).
2. Neue Variante + `label()`-Arm + `next()`-Arm ergänzen (zykelt sie in die
   Tasten-Rotation).
3. Im zugehörigen Render-Pfad (`match style { … }`) das Aussehen bauen.
4. Default ggf. in `State::new()` setzen, damit der User sofort die neue Variante
   sieht.
5. `cargo build --example hud_lab` + `cargo clippy --example hud_lab` müssen
   **warnungsfrei** sein (Projekt-Norm, auch für den Example).

## Effekte im Companion

Beim Bauen von tachyonfx-Effekten gilt **auch hier** die `effects`-Skill: die
HARTE Non-Overshoot-Panik-Regel für `expand`/`stretch` (nur `CircOut`/`QuadOut`/
`SineOut`/`CubicOut`). Im Spiel laufen Effekte über die benannten Konstruktoren in
`src/effects/`; der Companion darf zum **schnellen Probieren** auch direkt
`tachyonfx::fx::*` aufrufen — aber sobald eine Variante gewinnt, wandert sie als
**benannter Konstruktor** nach `src/effects/` (mit Smoke-Test) und wird von dort
verdrahtet, nie roh in der Render-/Game-Schicht.

## Vom Companion ins Spiel

Wenn der User eine Variante gewählt hat:
- Look in `src/hud/` (Overlay/Notification) bzw. `src/render/` (HUD/Welt/Cursor)
  bauen, Effekte als Konstruktoren in `src/effects/`.
- Logik-Phasen (z. B. Notification-Lebenszyklus) als **reine Funktion der Zeit/des
  Alters** halten → unit-testbar; nur das Zeichnen bleibt untestbar.
- Den Companion als Sandbox **stehen lassen** (er ist die Doku des Design-Prozesses
  und die Basis für die nächste visuelle Frage) — es sei denn, der User will ihn weg.
