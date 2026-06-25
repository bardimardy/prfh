# HUD-Inventar + Pickup-Animation + echtes Powerup-Spawn (W3 / #44) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** #44 verdrahten — Inventar-Overlay (oben rechts, wächst nach unten), Pickup-Animation auf der neuen Zeile, Shadow-Autocomplete-Highlight, und echtes Spawnen von Powerup-Wörtern in die Arena.

**Architecture:** Pickup- und Aktivierungs-Animation laufen als render-time-Math auf einem Timer-State in `App` (Spiegel von `cast_wave`/`draw_cast_ring`, #43), **nicht** über `EffectManager`. Effekt-Trigger entstehen aus host-autoritativen Spiel-Events, gebündelt in einem `EffectEvent`-Enum (in #44 lokal erzeugt+angewendet; MP-Broadcast später additiv — Seam jetzt, Draht später). Echtes Spawn über `spawn_powerups(&mut Arena)` ersetzt den `PRFH_DEBUG`-Dash.

**Tech Stack:** Rust 2021, ratatui 0.30, crossterm 0.29, tachyonfx 0.25 (hier nicht im Pickup-Pfad). Companion: `examples/hud_lab.rs` Szene 6 (validierte Looks zum Portieren).

**Design doc:** `docs/superpowers/specs/2026-06-24-hud-inventory-pickup-spawn-design.md`

## Global Constraints

- `cargo build`, `cargo test`, `cargo clippy` müssen **grün UND warnungsfrei** sein (Projekt-Norm; kein `#[allow]` zum Verstecken).
- `cargo` ist nicht im PATH: `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"` vor jedem cargo-Aufruf.
- Alle Farben aus `src/theme.rs` (Single Source of Truth) — keine rohen `Color::Rgb`/`Indexed` im Game-Code.
- Effekte über/nahe scrollendem Feld = render-time-Math, nie tachyonfx über das scrollende Feld (Learning #37). Pickup-Panel ist ein **statisches** Overlay → render-time-Math ist hier die gewählte (nicht erzwungene) Konsistenz-Wahl.
- TDD wo unit-testbar (Logik); Render/Effekte über „zeichnet/läuft ohne Panik"-Smoke-Tests. `main` wird NICHT auf visuelle Korrektheit gegated.
- Häufig committen (ein Commit pro Task). Commit-Messages enden mit `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Branch `issue-44`. Nicht auf `main` pushen.

---

## File Structure

- `src/theme.rs` — + Konstante `PICKUP_FLASH`.
- `src/game/powerup.rs` — + `EffectEvent`-Enum, + `spawn_powerups(&mut Arena)`.
- `src/app.rs` — + Felder `pickup_anim: Option<PickupAnim>`, `inv_visible: bool`; + `PickupAnim`-Struct; + `apply_effect_event`, `advance_pickup_anim`, `toggle_inventory`, `inventory_open`; `new_single` ruft `spawn_powerups`; `on_char`/`dispatch_cast` erzeugen `EffectEvent`.
- `src/render/mod.rs` — + `draw_inventory` (Rounded/TopRight/dynamisch/§8 + Shadow-Highlight + render-time-Pickup-Math); `draw` schreibt `pickup_anim` fort; Ghost-Styling der Map-Wörter verfeinert.
- Key-Dispatch (Event-Loop, vmtl. `src/main.rs` — per `grep -rn "toggle_cast\|KeyCode::Tab" src/` lokalisieren) — + Backtick → `toggle_inventory`.

---

### Task 1: `spawn_powerups` — echtes Spawn (Issue D)

**Files:**
- Modify: `src/game/powerup.rs` (+ Funktion + Test)
- Modify: `src/app.rs:56-70` (`new_single`)
- Test: `src/game/powerup.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `pub fn spawn_powerups(arena: &mut crate::game::arena::Arena)` — platziert eine feste Start-Menge `PowerupWord`-Entitäten. Reihenfolge/Positionen deterministisch.
- Consumes: `Arena::spawn((i32,i32), EntityKind::PowerupWord(PowerupWord{..})) -> EntityId` (arena.rs:48), `EntityKind::PowerupWord` (arena.rs:25).

- [ ] **Step 1: Failing test** — in `powerup.rs` tests:

```rust
#[test]
fn spawn_powerups_seeds_the_fixed_starter_set() {
    use crate::game::arena::{Arena, EntityKind};
    let mut a = Arena::new();
    spawn_powerups(&mut a);
    // Drei Starter-Wörter an festen Positionen (Andockpunkt für spätere prozedurale Gen).
    let names: Vec<&str> = a
        .entities
        .iter()
        .map(|e| match &e.kind {
            EntityKind::PowerupWord(w) => w.name.as_str(),
        })
        .collect();
    assert_eq!(names, vec!["dash", "revert", "warp"]);
    // Positionen deterministisch und vom Start (0,0) weg gestreut.
    let origins: Vec<(i32, i32)> = a
        .entities
        .iter()
        .map(|e| match &e.kind {
            EntityKind::PowerupWord(w) => w.origin,
        })
        .collect();
    assert_eq!(origins, vec![(6, 0), (0, 5), (-12, 3)]);
}
```

- [ ] **Step 2: Run, verify FAIL**

Run: `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"; cargo test -p prfh spawn_powerups_seeds 2>&1 | tail -20`
Expected: FAIL — `cannot find function spawn_powerups`.

- [ ] **Step 3: Implement** — in `powerup.rs` (oben, vor `#[cfg(test)]`):

```rust
use crate::game::arena::{Arena, EntityKind};

/// Platziert die feste Start-Menge Powerup-Wörter in die Arena. Host-autoritativer
/// Andockpunkt für spätere prozedurale Generierung (Welt-Spec §4). `dash` horizontal,
/// `revert` vertikal, `warp` horizontal reversed — gestreut, vom Start (0,0) weg.
pub fn spawn_powerups(arena: &mut Arena) {
    let seed = [
        ("dash", (6, 0), Axis::Horizontal, false),
        ("revert", (0, 5), Axis::Vertical, false),
        ("warp", (-12, 3), Axis::Horizontal, true),
    ];
    for (name, origin, axis, reversed) in seed {
        arena.spawn(
            origin,
            EntityKind::PowerupWord(PowerupWord {
                name: name.into(),
                origin,
                axis,
                reversed,
            }),
        );
    }
}
```

- [ ] **Step 4: Run, verify PASS**

Run: `cargo test -p prfh spawn_powerups_seeds 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Wire into `new_single`** — ersetze den `PRFH_DEBUG`-Block in `src/app.rs:58-70` durch:

```rust
        // Echtes Spawn (Issue D): reguläre Start-Menge. Host-autoritativ; in MP
        // seedet der Host, Clients erhalten die Wörter über EntitySpawned/Snapshot.
        crate::game::powerup::spawn_powerups(&mut arena);
```

(Der `PRFH_DEBUG`-gegated Dash entfällt — `spawn_powerups` ist jetzt der reguläre Pfad. Die bestehenden `w2_tests` in `app.rs` spawnen ihre Wörter selbst via `spawn_dash`, bleiben also grün.)

- [ ] **Step 6: Run full suite + clippy, verify green**

Run: `cargo test 2>&1 | tail -20 && cargo clippy 2>&1 | tail -20`
Expected: alle Tests grün, keine Warnungen. Falls ein bestehender Test auf den alten `PRFH_DEBUG`-Dash baute (`grep -rn PRFH_DEBUG src/`), prüfen und anpassen.

- [ ] **Step 7: Commit**

```bash
git add src/game/powerup.rs src/app.rs
git commit -m "feat(#44): spawn_powerups — echtes Powerup-Spawn ersetzt PRFH_DEBUG-Dash

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: `EffectEvent`-Seam + `pickup_anim`-Lebenszyklus (reine Logik)

**Files:**
- Modify: `src/game/powerup.rs` (+ `EffectEvent`)
- Modify: `src/app.rs` (+ `PickupAnim`, Felder, Methoden, Init)
- Test: `src/app.rs` `#[cfg(test)] mod w2_tests`

**Interfaces:**
- Produces:
  - `pub enum EffectEvent { Pickup { slot: usize, name: String }, Activation { tag: EffectTag, name: String } }` (powerup.rs)
  - `pub struct PickupAnim { pub age: Duration, pub slot: usize }` (app.rs)
  - `App.pickup_anim: Option<PickupAnim>`, `App.inv_visible: bool`
  - `fn apply_effect_event(&mut self, ev: EffectEvent)` — Pickup→setzt `pickup_anim`, Activation→setzt `cast_wave`
  - `pub const PICKUP_ANIM_DUR: Duration` (= 600 ms) + `pub fn advance_pickup_anim(&mut self, dt: Duration)` — altert + räumt nach Ablauf auf `None`
- Consumes: `Duration` (`std::time::Duration`), `EffectTag` (powerup.rs:24), `cast_wave: Option<Duration>` (app.rs:39).

- [ ] **Step 1: Failing test** — in `app.rs` `w2_tests`:

```rust
#[test]
fn apply_pickup_event_starts_anim_on_slot() {
    use crate::game::powerup::EffectEvent;
    let mut app = App::new_single();
    app.apply_effect_event(EffectEvent::Pickup { slot: 2, name: "warp".into() });
    let a = app.pickup_anim.as_ref().expect("anim started");
    assert_eq!(a.slot, 2);
    assert_eq!(a.age, std::time::Duration::ZERO);
}

#[test]
fn apply_activation_event_fires_cast_wave() {
    use crate::game::powerup::{EffectEvent, EffectTag};
    let mut app = App::new_single();
    app.apply_effect_event(EffectEvent::Activation { tag: EffectTag::Test, name: "dash".into() });
    assert!(app.cast_wave.is_some());
}

#[test]
fn pickup_anim_advances_then_clears_after_duration() {
    use crate::game::powerup::EffectEvent;
    let mut app = App::new_single();
    app.apply_effect_event(EffectEvent::Pickup { slot: 0, name: "dash".into() });
    app.advance_pickup_anim(std::time::Duration::from_millis(100));
    assert_eq!(app.pickup_anim.as_ref().unwrap().age, std::time::Duration::from_millis(100));
    app.advance_pickup_anim(std::time::Duration::from_millis(600)); // über PICKUP_ANIM_DUR
    assert!(app.pickup_anim.is_none(), "anim cleared after its duration");
}
```

- [ ] **Step 2: Run, verify FAIL**

Run: `cargo test -p prfh apply_pickup_event_starts_anim 2>&1 | tail -20`
Expected: FAIL — `apply_effect_event`/`pickup_anim` existieren nicht.

- [ ] **Step 3: Implement** —
  - In `powerup.rs`:

```rust
/// Beobachtbares, host-autoritatives Spiel-Event, das eine Animation auslöst.
/// In #44 lokal erzeugt+angewendet; der MP-Broadcast (Host serialisiert → ServerMsg)
/// hängt sich später additiv hier an (Seam jetzt, Draht später — Design §3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectEvent {
    Pickup { slot: usize, name: String },
    Activation { tag: EffectTag, name: String },
}
```

  - In `app.rs`: Struct (oberhalb `impl App`) + Felder im `App`-Struct + Init in `new_single` + Methoden in `impl App`:

```rust
/// Render-time-Pickup-Animation: Timer + Inventar-Slot der neuen Zeile (Design §3).
pub struct PickupAnim {
    pub age: Duration,
    pub slot: usize,
}

pub const PICKUP_ANIM_DUR: Duration = Duration::from_millis(600);
```

```rust
    // im App-Struct (neben cast_wave):
    pub pickup_anim: Option<PickupAnim>,
    pub inv_visible: bool,
```

```rust
    // in new_single Self{..}: (neben cast_wave: None)
            pickup_anim: None,
            inv_visible: false,
```

```rust
    /// Wendet ein host-autoritatives EffectEvent auf den lokalen Animations-State
    /// an (Design §3.1). Pickup → render-time-Pickup-Anim auf der Slot-Zeile;
    /// Activation → render-time-Cast-Welle.
    pub fn apply_effect_event(&mut self, ev: crate::game::powerup::EffectEvent) {
        use crate::game::powerup::EffectEvent;
        match ev {
            EffectEvent::Pickup { slot, .. } => {
                self.pickup_anim = Some(PickupAnim { age: Duration::ZERO, slot });
            }
            EffectEvent::Activation { .. } => {
                self.cast_wave = Some(Duration::ZERO);
            }
        }
    }

    /// Schreibt die Pickup-Animation fort und räumt sie nach `PICKUP_ANIM_DUR` ab.
    /// Reine Funktion der Zeit (analog cast_wave) → unit-testbar.
    pub fn advance_pickup_anim(&mut self, dt: Duration) {
        if let Some(a) = self.pickup_anim.as_mut() {
            a.age += dt;
            if a.age >= PICKUP_ANIM_DUR {
                self.pickup_anim = None;
            }
        }
    }
```

- [ ] **Step 4: Run, verify PASS**

Run: `cargo test -p prfh "apply_pickup_event_starts_anim apply_activation_event pickup_anim_advances" 2>&1 | tail -20`
Expected: alle drei PASS. (Bei Bedarf einzeln aufrufen.)

- [ ] **Step 5: Commit**

```bash
git add src/game/powerup.rs src/app.rs
git commit -m "feat(#44): EffectEvent-Seam + pickup_anim-Lebenszyklus (render-time, MP-ready)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Trigger-Produktion verdrahten (on_char Pickup, dispatch_cast)

**Files:**
- Modify: `src/app.rs:256-263` (Pickup-Zweig), `src/app.rs:183-192` (`dispatch_cast`)
- Test: `src/app.rs` `w2_tests`

**Interfaces:**
- Consumes: `apply_effect_event` (Task 2), `EffectEvent` (Task 2), `Inventory.len()` (inventory.rs:19), bestehender Trace-`Completed`-Pfad.
- Produces: nach vollständigem Trace ist `pickup_anim` gesetzt mit `slot == inventory.len()-1`.

- [ ] **Step 1: Failing test** — in `w2_tests` (nutzt das bestehende `spawn_dash` + Trace-Helper-Muster; siehe `tracing_word_picks_it_up_into_inventory_and_despawns` als Vorlage):

```rust
#[test]
fn completing_a_trace_starts_pickup_anim_on_the_new_slot() {
    let mut app = App::new_single();
    spawn_dash(&mut app); // legt "dash" bei (3,0) horizontal
    for c in "dash".chars() {
        app.on_char(c);
    }
    assert_eq!(app.inventory.len(), 1);
    let a = app.pickup_anim.as_ref().expect("pickup anim fired");
    assert_eq!(a.slot, 0, "slot == index der neuen (ersten) Inventar-Zeile");
}
```

- [ ] **Step 2: Run, verify FAIL**

Run: `cargo test -p prfh completing_a_trace_starts_pickup_anim 2>&1 | tail -20`
Expected: FAIL — `pickup_anim` bleibt `None` (Trigger noch nicht verdrahtet).

- [ ] **Step 3: Implement** — im Pickup-Zweig `app.rs:256-263`, nach `self.inventory.add(...)` und vor/statt der Notification, das Event erzeugen+anwenden:

```rust
            if let Some((id, name)) = pickup {
                arena.despawn(id);
                self.inventory.add(Powerup {
                    id,
                    name: name.clone(),
                    effect_tag: EffectTag::Test,
                });
                self.notifications.push(NotifyKind::Event, "✦  PICKUP", name.clone());
                // Host-autoritatives Event → lokale render-time-Pickup-Anim auf der
                // gerade hinzugefügten Zeile (Design §3.1). Slot = letzter Index.
                let slot = self.inventory.len() - 1;
                self.apply_effect_event(crate::game::powerup::EffectEvent::Pickup { slot, name });
            } else {
```

  Und `dispatch_cast` (`app.rs:183-192`) über denselben Seam führen — `self.cast_wave = Some(..)` ersetzen durch das Event:

```rust
    fn dispatch_cast(&mut self, tag: EffectTag, name: &str) {
        match &tag {
            EffectTag::Test => {
                self.notifications
                    .push(NotifyKind::Event, "⚡  CAST", name.to_string());
                self.debug_log(format!("cast dispatch: {name} ({tag:?})"));
            }
        }
        // Aktivierungs-Welle über denselben EffectEvent-Seam wie der Pickup.
        self.apply_effect_event(crate::game::powerup::EffectEvent::Activation {
            tag,
            name: name.to_string(),
        });
    }
```

  Hinweis: `dispatch_cast` wird in `on_cast_char` mit `p.effect_tag`/`p.name` aufgerufen (app.rs:175) — Signatur bleibt `(EffectTag, &str)`.

- [ ] **Step 4: Run, verify PASS** (neuer Test + bestehender Cast-Wave-Test `dispatch fired the cast wave` muss grün bleiben)

Run: `cargo test -p prfh "completing_a_trace_starts_pickup_anim dispatch" 2>&1 | tail -20`
Expected: PASS; `assert!(app.cast_wave.is_some())` weiterhin grün.

- [ ] **Step 5: Full suite + clippy**

Run: `cargo test 2>&1 | tail -10 && cargo clippy 2>&1 | tail -10`
Expected: grün, warnungsfrei.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat(#44): Pickup/Cast-Trigger über EffectEvent-Seam verdrahtet

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Inventar-Sichtbarkeits-Logik + Toggle-Taste

**Files:**
- Modify: `src/app.rs` (+ `inventory_open`, `toggle_inventory`)
- Modify: Key-Dispatch (Event-Loop; `grep -rn "toggle_cast" src/` → dieselbe Stelle)
- Test: `src/app.rs` `w2_tests`

**Interfaces:**
- Produces: `pub fn inventory_open(&self) -> bool` (auto-sichtbar wenn nicht leer ODER Cast-Modus ODER manuell `inv_visible`); `pub fn toggle_inventory(&mut self)`.
- Consumes: `Inventory.is_empty()` (inventory.rs:23), `cast_mode` (app.rs:36), `inv_visible` (Task 2).

- [ ] **Step 1: Failing test**

```rust
#[test]
fn inventory_visibility_rules() {
    let mut app = App::new_single();
    assert!(!app.inventory_open(), "leer + kein cast + kein toggle → versteckt");
    app.inventory.add(crate::game::powerup::Powerup {
        id: 1, name: "dash".into(), effect_tag: crate::game::powerup::EffectTag::Test,
    });
    assert!(app.inventory_open(), "nicht leer → sichtbar");
}

#[test]
fn cast_mode_pops_inventory_even_when_empty() {
    let mut app = App::new_single();
    app.toggle_cast(); // Cast an
    assert!(app.inventory_open(), "Cast-Modus poppt das Inventar");
}

#[test]
fn manual_toggle_forces_visibility_when_empty() {
    let mut app = App::new_single();
    assert!(!app.inventory_open());
    app.toggle_inventory();
    assert!(app.inventory_open(), "manuelles Toggle erzwingt Sichtbarkeit");
    app.toggle_inventory();
    assert!(!app.inventory_open());
}
```

- [ ] **Step 2: Run, verify FAIL**

Run: `cargo test -p prfh "inventory_visibility_rules cast_mode_pops manual_toggle" 2>&1 | tail -20`
Expected: FAIL — `inventory_open`/`toggle_inventory` fehlen.

- [ ] **Step 3: Implement** — in `impl App`:

```rust
    /// Ob das Inventar-Overlay sichtbar ist: automatisch sobald nicht leer, oder im
    /// Cast-Modus (Auto-Pop, §8), oder manuell erzwungen. Buchstaben bewegen → kein
    /// Buchstaben-Hotkey; manuelles Toggle liegt auf einer Nicht-Buchstaben-Taste.
    pub fn inventory_open(&self) -> bool {
        self.inv_visible || self.cast_mode || !self.inventory.is_empty()
    }

    /// Manuelles Ein-/Ausblenden des Inventar-Overlays (Nicht-Buchstaben-Taste).
    pub fn toggle_inventory(&mut self) {
        self.inv_visible = !self.inv_visible;
    }
```

- [ ] **Step 4: Run, verify PASS**

Run: `cargo test -p prfh "inventory_visibility_rules cast_mode_pops manual_toggle" 2>&1 | tail -20`
Expected: alle PASS.

- [ ] **Step 5: Toggle-Taste verdrahten** — Key-Dispatch lokalisieren: `grep -rn "toggle_cast\|KeyCode::Tab" src/`. Neben dem `Tab`→`toggle_cast`-Arm einen Arm für Backtick ergänzen:

```rust
                        KeyCode::Char('`') => app.toggle_inventory(),
```

  (Backtick kollidiert nicht mit Tippen-bewegt-Buchstaben. Falls der Char-Handler `'`'` sonst an `on_char` durchreicht, sicherstellen, dass dieser Arm **vor** dem generischen `Char(c) => app.on_char(c)` steht.)

- [ ] **Step 6: Build + clippy** (Key-Wiring nicht unit-getestet)

Run: `cargo build 2>&1 | tail -10 && cargo clippy 2>&1 | tail -10`
Expected: grün, warnungsfrei.

- [ ] **Step 7: Commit**

```bash
git add src/app.rs src/main.rs
git commit -m "feat(#44): Inventar-Sichtbarkeit (auto/cast-pop/toggle) + Backtick-Toggle

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: `PICKUP_FLASH` + Inventar-Overlay zeichnen (Rounded/TopRight/dynamisch/§8)

**Files:**
- Modify: `src/theme.rs` (+ `PICKUP_FLASH`)
- Modify: `src/render/mod.rs` (+ `draw_inventory`, Aufruf in `draw`)
- Test: `src/render/mod.rs` Smoke-Test (zeichnet ohne Panik)

**Interfaces:**
- Produces: `pub const PICKUP_FLASH: Color` (theme.rs); `fn draw_inventory(f: &mut Frame, area: Rect, app: &App)` (render).
- Consumes: `app.inventory_open()` (Task 4), `app.inventory.items` (inventory.rs), `Anchor::TopRight`/`anchor_rect(area, anchor, w, h)` (hud), `theme::{ACCENT,PANEL_BG,TEXT,TEXT_DIM}`. **Vorlage zum Portieren:** `examples/hud_lab.rs` Szene 6 `draw_inventory` + `InvSkin::Rounded`-Arm (lesen!).

- [ ] **Step 1: theme-Konstante** — in `src/theme.rs` (warmes Off-White, kein Reinweiß; Design §2.1):

```rust
/// Heller Flash beim Pickup-Landen (pop-pulse). Bewusster Look-Zusatz über §2;
/// warmes Off-White statt Reinweiß, damit es in die Dark-Palette passt.
pub const PICKUP_FLASH: Color = Color::Rgb(0xFF, 0xF4, 0xE6);
```

- [ ] **Step 2: Failing smoke test** — in `render/mod.rs` Tests (Vorlage: bestehende Render-Smoke-Tests, die `App` + `Buffer`/`TestBackend` aufsetzen; vorhandenes Muster im File übernehmen):

```rust
#[test]
fn draw_inventory_renders_without_panic_when_open() {
    let mut app = App::new_single();
    app.inventory.add(crate::game::powerup::Powerup {
        id: 1, name: "dash".into(), effect_tag: crate::game::powerup::EffectTag::Test,
    });
    assert!(app.inventory_open());
    // ganzer draw-Pfad darf nicht paniken (Inventar oben rechts, dynamische Höhe)
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| crate::render::draw(f, &mut app, std::time::Duration::from_millis(16)))
        .unwrap();
}
```

  (Falls die bestehenden Render-Tests ein anderes Setup nutzen — z. B. direkt `Buffer::empty` + interne Draw-Funktion — diesem Muster folgen statt `TestBackend`.)

- [ ] **Step 3: Run, verify FAIL/compile-error** (Funktion/Aufruf fehlt)

Run: `cargo test -p prfh draw_inventory_renders_without_panic 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 4: Implement** — `draw_inventory` portieren aus Companion Szene 6 (`InvSkin::Rounded`), an die echten Typen angepasst:
  - Rect via `anchor_rect(area, Anchor::TopRight, w, h)`, `w` fest (z. B. 34), `h = header_rows + items + breathing_rows` (dynamisch).
  - `Block` mit `BorderType::Rounded`, `style fg=ACCENT bg=PANEL_BG`, Titel ` POWERUPS `.
  - Je 1 PANEL_BG-Leerzeile über Header / unter Zeilen (§8).
  - Zeilen aus `app.inventory.items` (Name fett `TEXT`, ggf. Beschreibung `TEXT_DIM`). Bei leerem Inventar (nur im Cast/Toggle sichtbar) eine `— leer —`-Zeile.
  - `Clear` vor dem Block (Overlay über Welt).
  - In `draw` (render/mod.rs:34) am Ende, nach Welt/HUD: `if app.inventory_open() { draw_inventory(f, area, app); }`.
  - **Wichtig:** Zeilen-Layout der Companion-Variante übernehmen, in der der Highlight-Layout-Shift bereits gefixt ist (Name-Feld feste Breite, kein Trailing-Space) — relevant für Task 7.

- [ ] **Step 5: Run, verify PASS + build/clippy**

Run: `cargo test -p prfh draw_inventory_renders_without_panic 2>&1 | tail -10 && cargo clippy 2>&1 | tail -10`
Expected: PASS, warnungsfrei.

- [ ] **Step 6: Commit**

```bash
git add src/theme.rs src/render/mod.rs
git commit -m "feat(#44): Inventar-Overlay (rounded/top-right/dynamisch/§8) + PICKUP_FLASH

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Render-time-Pickup-Animation (pop-pulse) auf der Slot-Zeile

**Files:**
- Modify: `src/render/mod.rs` (`draw` schreibt `pickup_anim` fort; `draw_inventory` rendert die Anim auf der Slot-Zeile)
- Test: `src/render/mod.rs` Smoke-Test

**Interfaces:**
- Consumes: `app.pickup_anim` (Task 2), `app.advance_pickup_anim(dt)` (Task 2), `theme::{PICKUP_BASE, PICKUP_FLASH, TEXT, PANEL_BG}`. **Vorlage:** Companion Szene 6 `PopPulse`/`rainbow_fg`/`animated_pickup_line` (lesen + portieren).
- Produces: pop-pulse-Look auf der Zeile `pickup_anim.slot` (Flash-Decay `(1-p/0.3)²` → Doppel-Hue-Puls über `PICKUP_BASE` → `TEXT` bei p=1). Reine render-time-Math (kein tachyonfx).

- [ ] **Step 1: `pickup_anim` in `draw` fortschreiben** — in `render/mod.rs:34` `draw`, analog zum `cast_wave`-Block (`:37-47`):

```rust
    app.advance_pickup_anim(elapsed);
```

  (Direkt nach dem bestehenden `cast_wave`-Advance einfügen.)

- [ ] **Step 2: Failing smoke test**

```rust
#[test]
fn pickup_anim_renders_and_clears_without_panic() {
    use crate::game::powerup::{EffectEvent, Powerup, EffectTag};
    let mut app = App::new_single();
    app.inventory.add(Powerup { id: 1, name: "dash".into(), effect_tag: EffectTag::Test });
    app.apply_effect_event(EffectEvent::Pickup { slot: 0, name: "dash".into() });
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    // mehrere Frames über die Anim-Dauer hinaus — darf nicht paniken, Anim klärt
    for _ in 0..50 {
        terminal
            .draw(|f| crate::render::draw(f, &mut app, std::time::Duration::from_millis(16)))
            .unwrap();
    }
    assert!(app.pickup_anim.is_none(), "Anim nach Ablauf geräumt");
}
```

- [ ] **Step 3: Run, verify FAIL** (Anim wird noch nicht gerendert/fortgeschrieben; Test schlägt bei `is_none()` oder kompiliert nicht)

Run: `cargo test -p prfh pickup_anim_renders_and_clears 2>&1 | tail -20`

- [ ] **Step 4: Implement** — in `draw_inventory` die Slot-Zeile, wenn `app.pickup_anim`/slot passt, über die render-time-Math aus dem Companion einfärben: Phase `p = age/PICKUP_ANIM_DUR`; Flash `fg=PICKUP_FLASH` über `bg=ACCENT` mit Decay `(1 - p/0.30).max(0).powi(2)`; danach Hue-Puls `(1-p)*(0.5+0.5*sin(4π p))` über `PICKUP_BASE`, blendend nach `TEXT`. Den `hsl()`+`blend()`-Helper aus dem Companion (oder den bestehenden `draw_cast_ring`-`hsl`) wiederverwenden/portieren. **Kein tachyonfx.**

- [ ] **Step 5: Run, verify PASS + clippy**

Run: `cargo test -p prfh pickup_anim_renders_and_clears 2>&1 | tail -10 && cargo clippy 2>&1 | tail -10`
Expected: PASS, warnungsfrei.

- [ ] **Step 6: Commit**

```bash
git add src/render/mod.rs
git commit -m "feat(#44): render-time pop-pulse Pickup-Animation auf der Inventar-Zeile

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 7: Shadow-Autocomplete-Highlight (box+dim) im Cast-Modus

**Files:**
- Modify: `src/render/mod.rs` (`draw_inventory` Highlight-Pfad)
- Test: `src/render/mod.rs` Smoke-Test

**Interfaces:**
- Consumes: `app.cast_mode`, `app.cast_buffer`, `app.inventory.prefix_matches(&str)` (inventory.rs:29), `theme::{HIGHLIGHT_BG, HIGHLIGHT_FG, TEXT, TEXT_DIM}`. **Vorlage:** Companion Szene 6 `ShadowStyle::BoxDim` + `inv_row` (mit dem bereits gefixten Layout-Shift — Name-Feld feste Breite, kein Trailing-Space).
- Produces: im Cast-Modus wird auf gematchten Zeilen der getippte Prefix als Pink-Kasten (`HIGHLIGHT_BG/FG`) gerendert, Rest lesbar (`TEXT`); nicht-gematchte Zeilen `TEXT_DIM` (BG bleibt `PANEL_BG`).

- [ ] **Step 1: Failing smoke test**

```rust
#[test]
fn shadow_highlight_renders_in_cast_mode_without_panic() {
    use crate::game::powerup::{Powerup, EffectTag};
    let mut app = App::new_single();
    app.inventory.add(Powerup { id: 1, name: "dash".into(), effect_tag: EffectTag::Test });
    app.inventory.add(Powerup { id: 2, name: "revert".into(), effect_tag: EffectTag::Test });
    app.toggle_cast();
    for c in "da".chars() { app.on_char(c); } // füllt cast_buffer "da"
    assert_eq!(app.cast_buffer, "da");
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| crate::render::draw(f, &mut app, std::time::Duration::from_millis(16)))
        .unwrap();
}
```

- [ ] **Step 2: Run, verify FAIL/compile** (Highlight-Pfad noch nicht da — Test kompiliert, dürfte aber zunächst durchlaufen; daher zuerst die Highlight-Logik als Verhalten sichern)

Run: `cargo test -p prfh shadow_highlight_renders_in_cast_mode 2>&1 | tail -20`
Hinweis: Da es ein Panik-Smoke-Test ist, läuft er ggf. schon grün. Der eigentliche Beleg ist visuell (hud_lab Task 9). Den Test trotzdem als Regression gegen künftige Panics behalten.

- [ ] **Step 3: Implement** — in `draw_inventory`: wenn `app.cast_mode`, `prefix = &app.cast_buffer`, `matches = app.inventory.prefix_matches(prefix)` (Namen). Pro Zeile: ist der Name in `matches` und `prefix` nicht leer → Prefix-Spans in `HIGHLIGHT_BG/HIGHLIGHT_FG`, Suffix `TEXT`; sonst (kein Match) Zeile `TEXT_DIM`. **Name-Feld auf konstante Breite halten (kein Trailing-Space im Highlight-Zweig)** — der im Companion dokumentierte Layout-Shift-Invariant. Nicht im Cast-Modus → normale Zeilen (Task 5).

- [ ] **Step 4: Run, verify PASS + clippy**

Run: `cargo test -p prfh shadow_highlight_renders_in_cast_mode 2>&1 | tail -10 && cargo clippy 2>&1 | tail -10`
Expected: PASS, warnungsfrei.

- [ ] **Step 5: Commit**

```bash
git add src/render/mod.rs
git commit -m "feat(#44): Shadow-Autocomplete-Highlight (box+dim) im Cast-Modus

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 8: Ghost-Styling der nicht eingesammelten Map-Wörter

**Files:**
- Modify: `src/render/mod.rs` (`PowerupWord`-Zeichnung, ~`:288`)
- Test: `src/render/mod.rs` Smoke-Test (vorhandene Word-Render-Tests erweitern/grün halten)

**Interfaces:**
- Consumes: bestehende `EntityKind::PowerupWord(pw)`-Zeichnung (render/mod.rs:288), `theme::TEXT_DIM` (Ghost-Ton). **Vorlage:** Companion Szene 4 `WordStyle` (gewählter Ghost-Look).
- Produces: nicht eingesammelte Wörter dezent/ghost (gedämpft, klar vom eigenen Trail unterscheidbar).

- [ ] **Step 1: Prüfen, was schon da ist** — `sed -n '280,320p' src/render/mod.rs` lesen. Ist bereits ein Ghost-Ton gesetzt? Falls ja und es entspricht dem Companion-Look: nur Smoke-Test ergänzen. Falls nein: Styling auf den gewählten Ghost-Look anheben.

- [ ] **Step 2: Smoke-Test** — Word-Render ohne Panik (vorhandene `PowerupWord`-Render-Tests im File als Vorlage; mit gespawntem Wort `draw` aufrufen, kein Panik). Falls schon abgedeckt: bestehenden Test grün halten.

- [ ] **Step 3: Implement** — Ghost-Ton/-Glyph der Map-Wörter an den gewählten `WordStyle` angleichen (z. B. `TEXT_DIM`-fg, ggf. dezenter Modifier). Reine render-time-Färbung.

- [ ] **Step 4: Run tests + clippy**

Run: `cargo test 2>&1 | tail -10 && cargo clippy 2>&1 | tail -10`
Expected: grün, warnungsfrei.

- [ ] **Step 5: Commit**

```bash
git add src/render/mod.rs
git commit -m "feat(#44): Ghost-Styling der nicht eingesammelten Powerup-Wörter

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 9: Integration — voller grüner Lauf, visuelle Abnahme, Review

**Files:** keine neuen; Verifikation + ggf. Doku.

- [ ] **Step 1: Voller Lauf**

Run: `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"; cargo build 2>&1 | tail -5 && cargo test 2>&1 | tail -15 && cargo clippy 2>&1 | tail -15`
Expected: alles grün, **null** Warnungen.

- [ ] **Step 2: Visuelle Abnahme im echten Spiel** — `cargo run` (ggf. `PRFH_DEBUG=1`): Wörter erscheinen ghost-styled auf der Map; Einsammeln spielt pop-pulse auf der neuen Inventar-Zeile (oben rechts, wächst); `Tab` → Cast poppt Inventar mit box+dim-Shadow-Highlight; Backtick toggelt Inventar. (Manuell, nicht automatisiert.)

- [ ] **Step 3: AK-Abgleich gegen Issue #44** — jedes Akzeptanzkriterium gegen das Ergebnis prüfen (Overlay §8 ✓, Shadow ✓, Pickup-/Aktivierungs-Anim ✓, echtes Spawn + Ghost ✓, build/test grün ✓).

- [ ] **Step 4: code-reviewer-Subagent** (CLAUDE.md-Norm vor `gh pr ready`) auf den Diff `main..issue-44`. Gemeldete High-Confidence-Findings beheben.

- [ ] **Step 5: Learnings festhalten** (CLAUDE.md-Norm) — falls nicht-offensichtlich: render-time-Pickup-Anim-Muster + `EffectEvent`-Seam ins `effects`-/`visual-companion`-Skill bzw. die `PICKUP_FLASH`-Deviation dokumentieren. Im selben PR.

- [ ] **Step 6: `main` reinmergen, pushen, PR ready**

```bash
git fetch origin && git merge origin/main
cargo test 2>&1 | tail -5
git push
gh pr ready 51
```

---

## Self-Review (gegen Spec)

**Spec coverage:** §2 Looks → Task 5/6/7 (rounded/top-right/dynamisch/pop-pulse/box+dim); §2.1 PICKUP_FLASH → Task 5; §3 render-time + kein EffectManager → Task 2/6; §3.1 EffectEvent-Seam → Task 2/3; §3.2 App-Felder → Task 2; §4 Datei-Schnitt → alle; §5 Spawn+Ghost → Task 1/8; §6 Sichtbarkeit/Toggle → Task 4; §7 Tests → Task 1-4 (unit) + 5-8 (smoke); §8 Vertagtes → nicht eingeplant (korrekt). Keine Lücke.

**Placeholder scan:** Konkrete Tests/Code in jedem Logik-Step. Render-Steps verweisen auf die exakte Companion-Vorlage (Szene 6) statt Code zu duplizieren — bewusst, da der Look dort schon validiert ist; der Implementierer liest+portiert. Offene §9-Detailpunkte sind in den Tasks konkret entschieden (Spawn-Set, Backtick, PICKUP_FLASH-Hex).

**Type consistency:** `EffectEvent::Pickup{slot,name}`/`Activation{tag,name}`, `PickupAnim{age,slot}`, `apply_effect_event`, `advance_pickup_anim`, `PICKUP_ANIM_DUR`, `inventory_open`, `toggle_inventory`, `spawn_powerups`, `PICKUP_FLASH` — über alle Tasks identisch verwendet.
