# W2 — Powerup/Inventar-Base-Engine + Trace-FSM + Cast Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auf dem Arena-Substrat (W1, #42) die Powerup/Inventar-Base-Engine bauen: räumliches Pickup-Trace (beobachtende FSM), Inventar mit Prefix-Match, Cast-Modus mit Dispatch-Hook — plus die im `hud_lab` gewählten Visuals (shimmer Idle-Style + transparenter Rainbow-Cast-Ring) ins Spiel verdrahtet, manuell durchspielbar unter `PRFH_DEBUG`.

**Architecture:** `PowerupWord` zieht von `arena.rs` (opaker `{word}`-Payload) nach `src/game/powerup.rs` und wird zur reichen Layout-Struct `{name, origin, axis, reversed}` mit Keystroke→Tile-Mapping (beide Orientierungen). Die Trace-FSM in `writing.rs` beobachtet jeden `on_char`-Schreibvorgang (Position+Zeichen+Richtung) und steuert die Base-Mechanik **nicht** um. `app.rs` verdrahtet Trace, Inventar und einen dedizierten Cast-Modus (Tab-Toggle, eigener Buffer, Dispatch-Hook). Visuals sind render-time-Mathematik (scroll-immun), keine tachyonfx-Zell-Effekte.

**Tech Stack:** Rust 2021, Ratatui 0.30 + Crossterm 0.29, serde + ron (Sync), tachyonfx 0.25 (nur für bestehende Effekte; die W2-Visuals sind render-time-Mathematik).

## Global Constraints

- `cargo build` + `cargo test` müssen **fehler- UND warnungsfrei** grün bleiben (kein `#[allow]` zum Verstecken; toten Code entfernen). cargo nicht im PATH: `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"`.
- `clippy` warnungsfrei: `cargo clippy --all-targets`.
- `main` ist immer grün; jeder Task endet grün & committed (per-Task-grün-Disziplin, besonders beim `PowerupWord`-Umzug — Enum-Ripple durch arena/render).
- `Direction` bleibt **4-Wege** (Powerup-Spec §9) — keine Vorab-Verallgemeinerung.
- Trace-FSM ist **Beobachter** von `on_char` — **kein Umbau** der Base-Mechanik in `writing.rs`.
- Effekte/Animationen über scrollendem Welt-Inhalt sind **render-time-Mathematik** (Skill `effects`, Learning #37) — **nicht** tachyonfx (das blankt/schmiert auf scrollendem Inhalt; verifiziert: `evolve_into` setzt nicht-erreichte Zellen auf ' ').
- Single/Host/Client bleiben **getrennt**; W2 verdrahtet den vollen Flow in `Mode::Single`. Host-autoritatives Despawn (Pickup im MP) ist designed-for, hier nur als Andockpunkt notiert (W3/MP-Folge).
- Code-Stil passt zum umgebenden Code (Naming, Kommentar-Dichte, Idiome).

---

## File Structure

- **Create** `src/game/powerup.rs` — `Axis`, `EffectTag`, `Powerup {id,name,effect_tag}`, reiche `PowerupWord {name,origin,axis,reversed}` + Layout/Mapping-Methoden. Single source of truth fürs Wort-Layout.
- **Create** `src/game/inventory.rs` — `Inventory`, Prefix-Match, Exact-Lookup.
- **Modify** `src/game/mod.rs` — `pub mod powerup; pub mod inventory;`.
- **Modify** `src/game/arena.rs` — `PowerupWord` aus `powerup` importieren statt lokal definieren; Tests anpassen.
- **Modify** `src/game/writing.rs` — `Trace`/`TraceState`/`TraceStep` (FSM-Beobachter), `WritingEngine::trace_suspended`, `StepResult::tile()`.
- **Modify** `src/app.rs` — Felder (`inventory`, `trace`, `cast_mode`, `cast_buffer`, `cast_wave`, `anim_clock`); `on_char` füttert Trace + suspendiert Trigger; Cast-Toggle/Input/Dispatch; Test-Powerup-Spawn unter `PRFH_DEBUG`.
- **Modify** `src/render/mod.rs` — `draw_world` rendert Mehr-Tile-Wort mit shimmer; Cast-Ring (render-time); Cast-Buffer-Indikator; Anim-Clock/Wave-Advance in `draw`.
- **Modify** `src/main.rs` — `Tab` → `app.toggle_cast()`; Zeichen weiter über `app.on_char` (das im Cast-Modus selbst verzweigt).
- **External** — Follow-up-Issue „Test-Powerup entfernen/ersetzen" anlegen + im PR verlinken.

---

## Task 1: `powerup.rs` — Layout & Keystroke→Tile-Mapping

**Files:**
- Create: `src/game/powerup.rs`
- Modify: `src/game/mod.rs`
- Test: in `src/game/powerup.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `crate::game::writing::Direction` (`.delta() -> (i32,i32)`).
- Produces:
  - `Axis { Horizontal, Vertical }` mit `unit() -> (i32,i32)`.
  - `EffectTag { Test }` (additiv erweiterbar).
  - `Powerup { pub id: u32, pub name: String, pub effect_tag: EffectTag }`.
  - `PowerupWord { pub name: String, pub origin: (i32,i32), pub axis: Axis, pub reversed: bool }` mit:
    - `len() -> usize`, `is_empty() -> bool`
    - `tiles() -> Vec<(i32,i32)>` — physische Tiles `p_0..p_{n-1}` aufsteigend ab `origin`.
    - `keystroke_tile(k: usize) -> Option<(i32,i32)>` — Tile für den k-ten **logischen** Tastenanschlag.
    - `expected_char(k: usize) -> Option<char>` — `name[k]` (lowercase).
    - `entry_tile() -> (i32,i32)` — Tile von Keystroke 0.
    - `run_direction() -> (i32,i32)` — Einheitsvektor, der vom Entry-Tile **ins Wort** zeigt.

- [ ] **Step 1: Write the failing tests** (in `src/game/powerup.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn word(name: &str, origin: (i32, i32), axis: Axis, reversed: bool) -> PowerupWord {
        PowerupWord { name: name.into(), origin, axis, reversed }
    }

    #[test]
    fn tiles_ascend_from_origin_along_axis() {
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(w.tiles(), vec![(3, 0), (4, 0), (5, 0), (6, 0)]);
        let v = word("up", (0, 2), Axis::Vertical, false);
        assert_eq!(v.tiles(), vec![(0, 2), (0, 3)]);
    }

    #[test]
    fn keystroke_mapping_forward_lands_in_typing_order() {
        // not reversed: keystroke k → tile p_k; player types d,a,s,h at p_0..p_3.
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(w.keystroke_tile(0), Some((3, 0)));
        assert_eq!(w.keystroke_tile(3), Some((6, 0)));
        assert_eq!(w.keystroke_tile(4), None);
        assert_eq!(w.entry_tile(), (3, 0));
        assert_eq!(w.run_direction(), (1, 0));
    }

    #[test]
    fn keystroke_mapping_reversed_starts_at_high_end() {
        // reversed: name[0] sits at p_{n-1}; player enters at the high end moving
        // back toward origin. Letters typed are STILL d,a,s,h (logical word).
        let w = word("dash", (3, 0), Axis::Horizontal, true);
        assert_eq!(w.keystroke_tile(0), Some((6, 0))); // 'd' at p_3
        assert_eq!(w.keystroke_tile(1), Some((5, 0))); // 'a' at p_2
        assert_eq!(w.keystroke_tile(3), Some((3, 0))); // 'h' at p_0
        assert_eq!(w.entry_tile(), (6, 0));
        assert_eq!(w.run_direction(), (-1, 0));
        assert_eq!(w.expected_char(0), Some('d'));
    }

    #[test]
    fn vertical_reversed_runs_upward() {
        let w = word("up", (0, 2), Axis::Vertical, true);
        assert_eq!(w.entry_tile(), (0, 3)); // p_1
        assert_eq!(w.run_direction(), (0, -1));
    }

    #[test]
    fn expected_char_is_logical_word_lowercased() {
        let w = word("Dash", (0, 0), Axis::Horizontal, false);
        assert_eq!(w.expected_char(0), Some('d'));
        assert_eq!(w.expected_char(3), Some('h'));
        assert_eq!(w.expected_char(9), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib powerup`
Expected: FAIL (module `powerup` not found / types not defined).

- [ ] **Step 3: Write the implementation** (top of `src/game/powerup.rs`)

```rust
use crate::game::writing::Direction;
use serde::{Deserialize, Serialize};

/// Achse, entlang der ein Powerup-Wort auf der Map liegt. `Direction` bleibt
/// 4-Wege (Powerup-Spec §9); die Achse ist die Orientierung der Tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    /// Einheitsvektor in aufsteigender Koordinatenrichtung der Achse.
    pub fn unit(self) -> (i32, i32) {
        match self {
            Axis::Horizontal => (1, 0),
            Axis::Vertical => (0, 1),
        }
    }
}

/// Fachlicher Effekt-Tag eines Powerups. Der Cast-Dispatch matcht darauf.
/// Vorerst nur das Test-Powerup; additiv erweiterbar (Dash, Revert, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectTag {
    Test,
}

/// Ein eingesammeltes Powerup im Inventar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Powerup {
    pub id: u32,
    pub name: String,
    pub effect_tag: EffectTag,
}

/// Ein noch nicht eingesammeltes Powerup-Wort auf der Map. Das Layout
/// (Origin/Achse/Reversed → Tile-Positionen + Keystroke→Tile-Mapping) ist der
/// W2-Job (Welt-Spec §4, Powerup-Spec §5). Im Substrat (`arena.rs`) ist es nur
/// ein `EntityKind`-Payload.
///
/// **Reversed-Regel (Powerup-Spec §5):** Der Spieler tippt IMMER das logische
/// Wort `name`. `reversed` betrifft nur Platzierung/Rendering: die physischen
/// Tiles `p_0..p_{n-1}` liegen aufsteigend ab `origin`; bei `reversed` zeigt
/// `p_i` den Buchstaben `name[n-1-i]`. Der k-te Tastenanschlag landet auf dem
/// Tile, das `name[k]` zeigt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerupWord {
    pub name: String,
    pub origin: (i32, i32),
    pub axis: Axis,
    pub reversed: bool,
}

impl PowerupWord {
    pub fn len(&self) -> usize {
        self.name.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
    }

    /// Physische Tiles `p_0..p_{n-1}`, aufsteigend ab `origin` entlang der Achse.
    pub fn tiles(&self) -> Vec<(i32, i32)> {
        let (dx, dy) = self.axis.unit();
        (0..self.len() as i32)
            .map(|i| (self.origin.0 + dx * i, self.origin.1 + dy * i))
            .collect()
    }

    /// Tile, auf dem der k-te logische Tastenanschlag landet.
    pub fn keystroke_tile(&self, k: usize) -> Option<(i32, i32)> {
        let n = self.len();
        if k >= n {
            return None;
        }
        let idx = if self.reversed { n - 1 - k } else { k } as i32;
        let (dx, dy) = self.axis.unit();
        Some((self.origin.0 + dx * idx, self.origin.1 + dy * idx))
    }

    /// Erwarteter logischer Buchstabe für Keystroke `k` (lowercase, ASCII).
    pub fn expected_char(&self, k: usize) -> Option<char> {
        self.name.chars().nth(k).map(|c| c.to_ascii_lowercase())
    }

    /// Eintritts-Tile: wo der Spieler `name[0]` schreiben muss.
    pub fn entry_tile(&self) -> (i32, i32) {
        self.keystroke_tile(0).unwrap_or(self.origin)
    }

    /// Lauf-/Traversier-Richtung vom Eintritts-Tile ins Wort hinein
    /// (Keystroke 0 → Keystroke 1). Für 1-Buchstaben-Wörter `(0,0)`.
    pub fn run_direction(&self) -> (i32, i32) {
        let a = self.entry_tile();
        match self.keystroke_tile(1) {
            Some(b) => (b.0 - a.0, b.1 - a.1),
            None => (0, 0),
        }
    }
}
```

Then register the module — add to `src/game/mod.rs` (alongside the existing `pub mod arena;` etc.):

```rust
pub mod inventory;
pub mod powerup;
```

> Note: `Direction` is imported but only used by Task 3's FSM signatures, not here yet. To avoid an unused-import warning **in this task**, do NOT import `Direction` until Task 3. Replace the first line of the implementation with just `use serde::{Deserialize, Serialize};` for Task 1, and add the `Direction` import in Task 3.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib powerup`
Expected: PASS (5 tests).

- [ ] **Step 5: Verify no warnings**

Run: `cargo build 2>&1 | grep -i warning` → no output. `cargo clippy --lib 2>&1 | tail -3` → finished, no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/game/powerup.rs src/game/mod.rs
git commit -m "feat(#43): powerup.rs — PowerupWord-Layout + Keystroke→Tile-Mapping (beide Orientierungen)"
```

---

## Task 2: `arena.rs` — `PowerupWord` aus `powerup` beziehen (Enum-Ripple grün halten)

**Files:**
- Modify: `src/game/arena.rs` (struct entfernen, importieren; Tests anpassen)
- Modify: `src/render/mod.rs` (Render + Render-Test auf Mehr-Tile umstellen)

**Interfaces:**
- Consumes: `crate::game::powerup::{PowerupWord, Axis}` (Task 1).
- Produces: `EntityKind::PowerupWord(crate::game::powerup::PowerupWord)` — unverändertes Variant-Tag, neuer Payload-Typ. Alle Konstruktionssites nutzen jetzt `{name, origin, axis, reversed}`.

- [ ] **Step 1: Verify which files construct `PowerupWord`**

Run: `grep -rn "PowerupWord" src/`
Expected sites: `src/game/arena.rs` (def + tests), `src/render/mod.rs` (render at line ~218 + test at ~330). `protocol.rs`/`server.rs`/`client.rs` reference `EntityKind`/`Entity` but do **not** construct `PowerupWord` directly — confirm none appear; if they do, update them in this task too.

- [ ] **Step 2: Replace the struct definition in `arena.rs`**

Remove the local definition:

```rust
/// Opaker Powerup-Payload. ...
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerupWord {
    pub word: String,
}
```

Replace the `EntityKind` doc + import. At the top of `arena.rs`, add:

```rust
use crate::game::powerup::PowerupWord;
```

And keep `EntityKind`:

```rust
/// Art der Entität. Additiv erweiterbar (Item, Obstacle, …) — Sync/Render
/// tragen neue Varianten automatisch mit. `PowerupWord` lebt in `powerup.rs`
/// (W2-Layout); das Substrat referenziert es nur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntityKind {
    PowerupWord(PowerupWord),
}
```

> `Entity` derives `PartialEq` (not `Eq`) — `PowerupWord` from Task 1 derives `Eq` too, which is compatible. The `(i32,i32)` and `String` fields keep `Serialize/Deserialize` working for the Welcome-snapshot/delta (net-sync unchanged).

- [ ] **Step 3: Update `arena.rs` tests**

Replace the test helper:

```rust
    fn powerup(word: &str) -> EntityKind {
        use crate::game::powerup::Axis;
        EntityKind::PowerupWord(PowerupWord {
            name: word.into(),
            origin: (0, 0),
            axis: Axis::Horizontal,
            reversed: false,
        })
    }
```

(The rest of the arena tests use `powerup("…")` and compare entity ids/positions, not the word fields — they keep working.)

- [ ] **Step 4: Update `draw_world` render in `src/render/mod.rs`**

Replace the single-char powerup render block (currently around lines 211–221) so it draws **every tile** of the word at its mapped position (shimmer styling comes in Task 7; here keep the existing dim `TEXT_DIM` so this task is a pure type-migration):

```rust
    // Entitäten zuerst zeichnen (Trails liegen optisch darüber). Dieselbe
    // cursor-zentrierte Transform wie die Tiles. Mehr-Tile-Wörter: jedes Tile
    // an seiner Position. Dezentes Ghost-Styling (Shimmer-Look: Task 7).
    for e in &arena.entities {
        match &e.kind {
            EntityKind::PowerupWord(pw) => {
                let letters: Vec<char> = pw.name.chars().collect();
                for (i, tile) in pw.tiles().iter().enumerate() {
                    let rx = tile.0 - cursor.0 + center.0;
                    let ry = tile.1 - cursor.1 + center.1;
                    if rx < 0 || ry < 0 || rx >= w || ry >= h {
                        continue;
                    }
                    // reversed: p_i zeigt name[n-1-i]; sonst name[i].
                    let ch = if pw.reversed {
                        letters[letters.len() - 1 - i]
                    } else {
                        letters[i]
                    };
                    grid[ry as usize][rx as usize] =
                        Some((ch, Style::default().fg(theme::TEXT_DIM)));
                }
            }
        }
    }
```

- [ ] **Step 5: Update the render test in `src/render/mod.rs`**

Replace `draw_world_renders_arena_entity_at_expected_cell` to build the new `PowerupWord` and assert the **first letter** lands at the mapped cell:

```rust
    #[test]
    fn draw_world_renders_arena_entity_at_expected_cell() {
        use crate::game::arena::EntityKind;
        use crate::game::powerup::{Axis, PowerupWord};
        let mut app = App::new();
        // origin (5,-2), horizontal, not reversed → p_0=(5,-2) shows 'z' ("zoom").
        app.arena_mut().unwrap().spawn(
            (5, -2),
            EntityKind::PowerupWord(PowerupWord {
                name: "zoom".into(),
                origin: (5, -2),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
        // Screen-Transform: (5,-2) - cursor(0,0) + center(40,12) = (45,10).
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app, Duration::ZERO)).unwrap();
        let buf = terminal.backend().buffer();
        assert_eq!(
            buf.cell((45, 10)).unwrap().symbol(),
            "z",
            "Powerup-Wort sollte sein erstes Tile bei (45,10) als 'z' rendern"
        );
    }
```

- [ ] **Step 6: Build, test, clippy — all green & warning-free**

Run: `cargo test` → all pass. `cargo clippy --all-targets 2>&1 | tail -3` → no warnings.
Expected: green. (This is the Enum-Ripple checkpoint — net-sync discipline.)

- [ ] **Step 7: Commit**

```bash
git add src/game/arena.rs src/render/mod.rs
git commit -m "feat(#43): PowerupWord nach powerup.rs verlagert; Mehr-Tile-Render in draw_world"
```

---

## Task 3: Trace-FSM in `writing.rs` (Beobachter von `on_char`)

**Files:**
- Modify: `src/game/powerup.rs` (jetzt `Direction`-Import aktivieren — wird hier real benutzt? Nein: die FSM lebt in writing.rs. Lasse powerup.rs unverändert; `Direction` wird in writing.rs gebraucht.)
- Modify: `src/game/writing.rs` (FSM-Typen + `Trace`, `StepResult::tile()`, `trace_suspended`)
- Test: in `src/game/writing.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `crate::game::powerup::PowerupWord` (Task 1) — `.entry_tile()`, `.run_direction()`, `.keystroke_tile(k)`, `.expected_char(k)`, `.len()`.
- Produces:
  - `TraceState { Idle, Tracing { id: u32, progress: usize } }`
  - `TraceStep { None, Armed { id: u32 }, Advanced { id: u32, progress: usize }, Completed { id: u32 }, Reset }`
  - `Trace { state: TraceState }` mit `new()`, `is_tracing() -> bool`, `observe(pos:(i32,i32), ch:char, dir:Direction, words:&[(u32,&PowerupWord)]) -> TraceStep`.
  - `WritingEngine.trace_suspended: bool` (default false) — wenn true, fired `on_char` **keine** Trigger und hält den `current_word`-Buffer leer.
  - `StepResult::tile(&self) -> Option<&Tile>`.

- [ ] **Step 1: Write the failing FSM tests** (append to `writing.rs` tests)

```rust
    use crate::game::powerup::{Axis, PowerupWord};

    fn pw(name: &str, origin: (i32, i32), axis: Axis, reversed: bool) -> PowerupWord {
        PowerupWord { name: name.into(), origin, axis, reversed }
    }

    #[test]
    fn trace_arms_on_entry_tile_correct_dir_and_char() {
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        // Wrong char at entry → no arm.
        assert_eq!(t.observe((3, 0), 'x', Direction::Right, &words), TraceStep::None);
        // Correct entry tile + dir + char → armed (progress 1).
        assert_eq!(t.observe((3, 0), 'd', Direction::Right, &words), TraceStep::Armed { id: 7 });
        assert!(t.is_tracing());
    }

    #[test]
    fn trace_does_not_arm_on_wrong_direction() {
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        // Right tile + char but moving Down (not into the word) → no arm.
        assert_eq!(t.observe((3, 0), 'd', Direction::Down, &words), TraceStep::None);
    }

    #[test]
    fn trace_advances_then_completes() {
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        t.observe((3, 0), 'd', Direction::Right, &words);
        assert_eq!(t.observe((4, 0), 'a', Direction::Right, &words), TraceStep::Advanced { id: 7, progress: 2 });
        assert_eq!(t.observe((5, 0), 's', Direction::Right, &words), TraceStep::Advanced { id: 7, progress: 3 });
        assert_eq!(t.observe((6, 0), 'h', Direction::Right, &words), TraceStep::Completed { id: 7 });
        assert!(!t.is_tracing());
    }

    #[test]
    fn trace_resets_on_wrong_char() {
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        t.observe((3, 0), 'd', Direction::Right, &words);
        assert_eq!(t.observe((4, 0), 'z', Direction::Right, &words), TraceStep::Reset);
        assert!(!t.is_tracing());
    }

    #[test]
    fn trace_resets_on_turning_off_axis() {
        // Player turned: the next written tile is not the expected axis tile.
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        t.observe((3, 0), 'd', Direction::Right, &words);
        // Expected (4,0); player wrote (3,1) (turned down) → reset even if char ok.
        assert_eq!(t.observe((3, 1), 'a', Direction::Down, &words), TraceStep::Reset);
    }

    #[test]
    fn trace_completes_reversed_word_typed_logically() {
        // reversed "dash": entry (6,0) moving Left, letters still d,a,s,h.
        let w = pw("dash", (3, 0), Axis::Horizontal, true);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        assert_eq!(t.observe((6, 0), 'd', Direction::Left, &words), TraceStep::Armed { id: 7 });
        assert_eq!(t.observe((5, 0), 'a', Direction::Left, &words), TraceStep::Advanced { id: 7, progress: 2 });
        t.observe((4, 0), 's', Direction::Left, &words);
        assert_eq!(t.observe((3, 0), 'h', Direction::Left, &words), TraceStep::Completed { id: 7 });
    }

    #[test]
    fn step_result_exposes_written_tile() {
        let mut e = WritingEngine::new((0, 0));
        let r = e.on_char('a');
        assert_eq!(r.tile().map(|t| t.pos), Some((0, 0)));
        let r = e.on_backspace();
        assert_eq!(r.tile(), None);
    }

    #[test]
    fn suspended_trace_does_not_fire_triggers() {
        let mut e = WritingEngine::new((0, 0));
        e.trace_suspended = true;
        for ch in "up".chars() {
            e.on_char(ch);
        }
        // Trigger suspended: direction unchanged, buffer stays clear.
        assert_eq!(e.direction, Direction::Right);
        assert!(e.current_word.is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib writing::tests::trace`
Expected: FAIL (`Trace`/`TraceStep` not defined).

- [ ] **Step 3: Add the FSM + helpers to `writing.rs`**

At the top of `writing.rs`, add the import:

```rust
use crate::game::powerup::PowerupWord;
```

Add `trace_suspended` to `WritingEngine` (field + `new()` init):

```rust
    /// Während eines aktiven Trace gesetzt (von `app.rs`): unterdrückt die
    /// Sofort-Trigger-Erkennung, damit die eigenen Wort-Buchstaben (z. B. „up"
    /// in „update") nicht feuern (Powerup-Spec §6).
    pub trace_suspended: bool,
```

In `WritingEngine::new`, add `trace_suspended: false,` to the struct literal.

In `on_char`, wrap the trigger block. Replace the `else { self.current_word.push(ch); … }` branch so that when suspended, the buffer is kept clear and no trigger fires:

```rust
        if is_boundary {
            self.current_word.clear();
        } else if self.trace_suspended {
            // Trace läuft: keine Trigger, Buffer leer halten (kein stale Trigger
            // nach Trace-Ende). Tile wurde bereits geschrieben + Cursor bewegt.
            self.current_word.clear();
        } else {
            self.current_word.push(ch);
            if let Some((trigger, tw_len)) = find_trigger_suffix(&self.current_word) {
                // … unverändert …
            }
        }
```

Add `StepResult::tile()` (after the enum):

```rust
impl StepResult {
    /// Das in diesem Schritt geschriebene Tile (None bei `Erased`).
    pub fn tile(&self) -> Option<&Tile> {
        match self {
            StepResult::Wrote(t)
            | StepResult::WroteAndTurned(t, _)
            | StepResult::WroteAndStopped(t) => Some(t),
            StepResult::Erased => None,
        }
    }
}
```

Add the FSM (near the bottom, before tests):

```rust
/// Zustand der beobachtenden Pickup-Trace-FSM (Powerup-Spec §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceState {
    Idle,
    Tracing { id: u32, progress: usize },
}

/// Ergebnis eines `observe`-Schritts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceStep {
    None,
    Armed { id: u32 },
    Advanced { id: u32, progress: usize },
    Completed { id: u32 },
    Reset,
}

/// Räumliches Arming-Trace: **beobachtet** jeden `on_char`-Schreibvorgang
/// (Position, Zeichen, Laufrichtung) und steuert die Base-Mechanik nicht um.
/// `id` ist die `EntityId` des Powerup-Worts in der Arena.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    pub state: TraceState,
}

impl Default for TraceState {
    fn default() -> Self {
        TraceState::Idle
    }
}

impl Trace {
    pub fn new() -> Self {
        Self { state: TraceState::Idle }
    }

    pub fn is_tracing(&self) -> bool {
        matches!(self.state, TraceState::Tracing { .. })
    }

    /// Beobachtet ein geschriebenes Tile. `dir` ist die Laufrichtung zum
    /// Schreibzeitpunkt. `words`: Kandidaten-Powerup-Wörter mit ihren EntityIds.
    pub fn observe(
        &mut self,
        pos: (i32, i32),
        ch: char,
        dir: Direction,
        words: &[(u32, &PowerupWord)],
    ) -> TraceStep {
        let ch = ch.to_ascii_lowercase();
        match self.state {
            TraceState::Idle => {
                for (id, w) in words {
                    if pos == w.entry_tile()
                        && dir.delta() == w.run_direction()
                        && w.expected_char(0) == Some(ch)
                    {
                        if w.len() <= 1 {
                            return TraceStep::Completed { id: *id };
                        }
                        self.state = TraceState::Tracing { id: *id, progress: 1 };
                        return TraceStep::Armed { id: *id };
                    }
                }
                TraceStep::None
            }
            TraceState::Tracing { id, progress } => {
                let Some((_, w)) = words.iter().find(|(wid, _)| *wid == id) else {
                    self.state = TraceState::Idle;
                    return TraceStep::Reset;
                };
                if w.keystroke_tile(progress) == Some(pos) && w.expected_char(progress) == Some(ch)
                {
                    let next = progress + 1;
                    if next >= w.len() {
                        self.state = TraceState::Idle;
                        TraceStep::Completed { id }
                    } else {
                        self.state = TraceState::Tracing { id, progress: next };
                        TraceStep::Advanced { id, progress: next }
                    }
                } else {
                    self.state = TraceState::Idle;
                    TraceStep::Reset
                }
            }
        }
    }
}
```

> `#[derive(Default)]` on `Trace` needs `TraceState: Default` → the manual `impl Default for TraceState` provides it. (Don't also derive `Default` on `TraceState` — the manual impl is clearer for an enum.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib writing`
Expected: PASS (all new + existing writing tests).

- [ ] **Step 5: Build/clippy — no warnings**

Run: `cargo clippy --all-targets 2>&1 | tail -3` → no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/game/writing.rs
git commit -m "feat(#43): Trace-FSM als Beobachter von on_char + Trigger-Suspendierung"
```

---

## Task 4: `inventory.rs` — Prefix-Match & Exact-Lookup

**Files:**
- Create: `src/game/inventory.rs`
- Test: in `src/game/inventory.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `crate::game::powerup::{Powerup, EffectTag}` (Task 1).
- Produces:
  - `Inventory { pub items: Vec<Powerup> }` mit `new()`, `add(Powerup)`, `len()`, `is_empty()`.
  - `prefix_matches(&self, buffer: &str) -> Vec<&Powerup>` — case-insensitiv; leerer Buffer → leer.
  - `get_exact(&self, name: &str) -> Option<&Powerup>` — case-insensitiv exakter Name.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::powerup::{EffectTag, Powerup};

    fn inv(names: &[&str]) -> Inventory {
        let mut i = Inventory::new();
        for (k, n) in names.iter().enumerate() {
            i.add(Powerup { id: k as u32, name: (*n).into(), effect_tag: EffectTag::Test });
        }
        i
    }

    #[test]
    fn empty_buffer_matches_nothing() {
        let i = inv(&["dash", "revert"]);
        assert!(i.prefix_matches("").is_empty());
    }

    #[test]
    fn prefix_matches_case_insensitive() {
        let i = inv(&["dash", "revert", "squash"]);
        let names: Vec<&str> = i.prefix_matches("s").iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["squash"]);
        assert_eq!(i.prefix_matches("RE").len(), 1);
        assert_eq!(i.prefix_matches("zzz").len(), 0);
    }

    #[test]
    fn get_exact_is_case_insensitive() {
        let i = inv(&["dash"]);
        assert_eq!(i.get_exact("DASH").map(|p| p.name.as_str()), Some("dash"));
        assert!(i.get_exact("das").is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib inventory`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement** (top of `src/game/inventory.rs`)

```rust
use crate::game::powerup::Powerup;

/// Eingesammelte Powerups. Cast matcht per Prefix (Overlay-Highlight) bzw.
/// exaktem Namen (Aktivierung).
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub items: Vec<Powerup>,
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, p: Powerup) {
        self.items.push(p);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Powerups, deren Name mit `buffer` beginnt (case-insensitiv). Leerer
    /// Buffer matcht nichts (Overlay poppt erst beim Tippen).
    pub fn prefix_matches(&self, buffer: &str) -> Vec<&Powerup> {
        if buffer.is_empty() {
            return Vec::new();
        }
        let b = buffer.to_ascii_lowercase();
        self.items
            .iter()
            .filter(|p| p.name.to_ascii_lowercase().starts_with(&b))
            .collect()
    }

    /// Exakter Name (case-insensitiv) → das zu aktivierende Powerup.
    pub fn get_exact(&self, name: &str) -> Option<&Powerup> {
        let n = name.to_ascii_lowercase();
        self.items.iter().find(|p| p.name.to_ascii_lowercase() == n)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib inventory`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/game/inventory.rs
git commit -m "feat(#43): inventory.rs — Prefix-Match + Exact-Lookup"
```

---

## Task 5: `app.rs` — Trace/Inventar/Cast verdrahten + Test-Powerup

**Files:**
- Modify: `src/app.rs`
- Test: in `src/app.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `Trace`/`TraceStep`/`StepResult::tile` (Task 3), `Inventory` (Task 4), `Powerup`/`EffectTag`/`PowerupWord`/`Axis` (Task 1), `Arena`/`EntityKind` (W1).
- Produces (on `App`):
  - Felder: `inventory: Inventory`, `trace: Trace`, `cast_mode: bool`, `cast_buffer: String`, `cast_wave: Option<Duration>`, `anim_clock: Duration`.
  - `toggle_cast(&mut self)`, `on_cast_char(&mut self, c: char)`, `dispatch_cast(&mut self, tag: EffectTag, name: &str)`.
  - `on_char` füttert Trace (Single) und routet bei `cast_mode` zu `on_cast_char`.

- [ ] **Step 1: Write the failing integration tests** (append to `app.rs` tests; create the `#[cfg(test)]` module if none exists)

```rust
#[cfg(test)]
mod w2_tests {
    use super::*;
    use crate::game::arena::EntityKind;
    use crate::game::powerup::{Axis, EffectTag, PowerupWord};

    fn spawn_dash(app: &mut App) {
        app.arena_mut().unwrap().spawn(
            (3, 0),
            EntityKind::PowerupWord(PowerupWord {
                name: "dash".into(),
                origin: (3, 0),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
    }

    #[test]
    fn tracing_word_picks_it_up_into_inventory_and_despawns() {
        let mut app = App::new(); // player at (0,0) moving Right
        spawn_dash(&mut app);
        // 3 filler chars walk the cursor to (3,0), then "dash" arms+completes.
        for ch in "xxxdash".chars() {
            app.on_char(ch);
        }
        assert_eq!(app.inventory.len(), 1, "dash should be collected");
        assert_eq!(app.inventory.items[0].name, "dash");
        assert!(app.arena().entities.is_empty(), "picked-up word despawns");
    }

    #[test]
    fn cast_exact_name_dispatches_and_leaves_cast_mode() {
        let mut app = App::new();
        app.inventory.add(Powerup { id: 0, name: "dash".into(), effect_tag: EffectTag::Test });
        app.toggle_cast();
        assert!(app.cast_mode);
        for ch in "dash".chars() {
            app.on_char(ch); // routed to cast buffer while cast_mode
        }
        assert!(!app.cast_mode, "exact match dispatches and exits cast mode");
        assert!(app.cast_wave.is_some(), "dispatch fired the cast wave");
    }

    #[test]
    fn cast_chars_do_not_write_tiles_or_move_cursor() {
        let mut app = App::new();
        app.toggle_cast();
        let before = app.local_engine().unwrap().cursor;
        for ch in "abc".chars() {
            app.on_char(ch);
        }
        assert_eq!(app.local_engine().unwrap().cursor, before, "cast input must not move the cursor");
        assert_eq!(app.cast_buffer, "abc");
    }

    #[test]
    fn toggle_cast_off_clears_buffer() {
        let mut app = App::new();
        app.toggle_cast();
        app.on_char('d');
        app.toggle_cast(); // off
        assert!(!app.cast_mode);
        assert!(app.cast_buffer.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib w2_tests`
Expected: FAIL (fields/methods missing).

- [ ] **Step 3: Add imports + fields**

At the top of `app.rs`:

```rust
use crate::game::arena::EntityKind;
use crate::game::inventory::Inventory;
use crate::game::powerup::{EffectTag, Powerup, PowerupWord};
use crate::game::writing::{Trace, TraceStep};
use std::time::Duration;
```

Add fields to `struct App`:

```rust
    /// Inventar der eingesammelten Powerups (Single-Flow; MP getrennt/W3).
    pub inventory: Inventory,
    /// Beobachtende Pickup-Trace-FSM.
    pub trace: Trace,
    /// Cast-Modus aktiv (Tab-Toggle): Zeichen füllen den Buffer statt zu schreiben.
    pub cast_mode: bool,
    pub cast_buffer: String,
    /// Alter der laufenden Cast-Welle (render-time-Ring); None = keine Welle.
    pub cast_wave: Option<Duration>,
    /// Monotone Animations-Uhr fürs render-time-Shimmer (vom Render getrieben).
    pub anim_clock: Duration,
```

- [ ] **Step 4: Spawn the test-powerup under `PRFH_DEBUG` and init fields in `new_single`**

```rust
    pub fn new_single() -> Self {
        let mut arena = Arena::new();
        // Test-Powerup nur unter PRFH_DEBUG: validiert den ganzen Flow
        // Pickup→Inventar→Cast→Dispatch. Entfernen via Follow-up-Issue.
        if std::env::var("PRFH_DEBUG").is_ok() {
            arena.spawn(
                (3, 0),
                EntityKind::PowerupWord(PowerupWord {
                    name: "dash".into(),
                    origin: (3, 0),
                    axis: crate::game::powerup::Axis::Horizontal,
                    reversed: false,
                }),
            );
        }
        Self {
            should_quit: false,
            mode: Mode::Single(WritingEngine::new((0, 0)), arena),
            last_event: String::from("type to write yourself a path"),
            notifications: NotificationStack::new(),
            debug: false,
            debug_lines: Vec::new(),
            inventory: Inventory::new(),
            trace: Trace::new(),
            cast_mode: false,
            cast_buffer: String::new(),
            cast_wave: None,
            anim_clock: Duration::ZERO,
        }
    }
```

- [ ] **Step 5: Add cast methods**

```rust
    /// Cast-Modus betreten/verlassen (Default-Taste `Tab`). Buffer wird geleert.
    pub fn toggle_cast(&mut self) {
        self.cast_mode = !self.cast_mode;
        self.cast_buffer.clear();
    }

    /// Zeichen im Cast-Modus: füllt den Buffer (schreibt KEIN Tile, bewegt den
    /// Cursor nicht). Bei exaktem Inventar-Namen → Dispatch + Modus verlassen.
    fn on_cast_char(&mut self, c: char) {
        self.cast_buffer.push(c);
        if let Some(p) = self.inventory.get_exact(&self.cast_buffer).cloned() {
            self.dispatch_cast(p.effect_tag, &p.name);
            self.cast_mode = false;
            self.cast_buffer.clear();
        }
    }

    /// Aktivierungs-Dispatch-Hook (Powerup-Spec §7): matcht `effect_tag`. Vorerst
    /// Log + Banner + render-time-Cast-Welle (echte Effekte wie Dash: später).
    fn dispatch_cast(&mut self, tag: EffectTag, name: &str) {
        match tag {
            EffectTag::Test => {
                self.notifications
                    .push(NotifyKind::Event, "⚡  CAST", name.to_string());
                self.cast_wave = Some(Duration::ZERO);
                self.debug_log(format!("cast dispatch: {name} ({tag:?})"));
            }
        }
    }
```

- [ ] **Step 6: Route + feed the Trace in `on_char`**

Replace `on_char` so it (a) routes to the cast buffer when in cast mode, and (b) feeds the Trace after each Single write:

```rust
    pub fn on_char(&mut self, c: char) {
        if c == ' ' {
            return;
        }
        if self.cast_mode {
            self.on_cast_char(c);
            return;
        }
        if let Mode::Single(e, arena) = &mut self.mode {
            let dir = e.direction;
            e.trace_suspended = self.trace.is_tracing();
            let result = e.on_char(c);

            // Trace füttern: nur wenn ein Tile geschrieben wurde.
            let mut pickup: Option<(u32, String)> = None;
            if let Some(t) = result.tile() {
                let pos = t.pos;
                let words: Vec<(u32, &PowerupWord)> = arena
                    .entities
                    .iter()
                    .filter_map(|ent| match &ent.kind {
                        EntityKind::PowerupWord(w) => Some((ent.id, w)),
                    })
                    .collect();
                if let TraceStep::Completed { id } = self.trace.observe(pos, c, dir, &words) {
                    if let Some((_, w)) = words.iter().find(|(wid, _)| *wid == id) {
                        pickup = Some((id, w.name.clone()));
                    }
                }
            }

            self.last_event = match &result {
                StepResult::Wrote(_) => format!("wrote '{}'", c),
                StepResult::WroteAndTurned(_, d) => format!("turned: {:?}", d),
                StepResult::WroteAndStopped(_) => "paused".into(),
                StepResult::Erased => "erased".into(),
            };

            // Pickup anwenden (host-autoritatives Despawn ist der MP-Andockpunkt;
            // Single despawnt direkt die lokale Arena).
            if let Some((id, name)) = pickup {
                arena.despawn(id);
                self.inventory.add(Powerup {
                    id,
                    name: name.clone(),
                    effect_tag: EffectTag::Test,
                });
                self.notifications
                    .push(NotifyKind::Event, "✦  PICKUP", name);
            } else {
                // Bestehende Turn/Stop-Notifications nur, wenn kein Pickup lief.
                match result {
                    StepResult::WroteAndTurned(_, d) => {
                        self.notifications
                            .push(NotifyKind::Info, "⟹  TURNED", format!("{d:?}"));
                    }
                    StepResult::WroteAndStopped(_) => {
                        self.notifications
                            .push(NotifyKind::Info, "⟹  STOP", "next char overwrites");
                    }
                    _ => {}
                }
            }
        }
    }
```

> Borrow note: `&mut self.mode` borrows only the `mode` field; `self.trace`, `self.inventory`, `self.notifications`, `self.last_event` are disjoint fields and remain usable inside the block. `words` (immutable borrow of `arena`) is dropped before `arena.despawn(id)` because the pickup id/name are cloned out first.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test --lib w2_tests`
Expected: PASS (4 tests). Then `cargo test` → all green.

- [ ] **Step 8: Build/clippy — no warnings**

Run: `cargo clippy --all-targets 2>&1 | tail -3` → no warnings.

- [ ] **Step 9: Commit**

```bash
git add src/app.rs
git commit -m "feat(#43): app.rs — Trace/Inventar/Cast verdrahtet + Test-Powerup (PRFH_DEBUG)"
```

---

## Task 6: `main.rs` — `Tab` betritt/verlässt den Cast-Modus

**Files:**
- Modify: `src/main.rs` (Key-Handling)

**Interfaces:**
- Consumes: `App::toggle_cast()` (Task 5), existing `App::on_char` (now cast-aware).
- Produces: keine neuen Typen — nur Input-Verdrahtung.

- [ ] **Step 1: Read the current key-handling block**

Run: `grep -n "KeyCode\|on_char\|on_backspace\|Char(" src/main.rs`
Identify where `KeyCode::Char(c) => app.on_char(c)` and `Backspace`/`Esc` are handled.

- [ ] **Step 2: Add the `Tab` arm**

In the key `match`, add (before the catch-all), keeping the existing arms:

```rust
                    KeyCode::Tab => app.toggle_cast(),
```

(Char input keeps going through `app.on_char(c)`, which now branches into the cast buffer when `cast_mode` is set. `Esc`: if it currently only quits, leave as-is — `Tab` toggles off; an optional refinement is `Esc` leaving cast mode first, but that is not required by the issue.)

- [ ] **Step 3: Build & manual smoke**

Run: `cargo build` → green, no warnings.
Run: `PRFH_DEBUG=1 cargo run` — type `xxxdash` (picks up dash → PICKUP banner), press `Tab` (enter cast), type `dash` (→ CAST banner + ring), confirm `Tab` toggles the mode. Quit.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(#43): Tab betritt/verlässt den Cast-Modus"
```

---

## Task 7: Visuals — shimmer Idle-Style + transparenter Rainbow-Cast-Ring + Cast-Buffer

**Files:**
- Modify: `src/render/mod.rs`
- Test: in `src/render/mod.rs` (`#[cfg(test)]` smoke test)

**Interfaces:**
- Consumes: `App.anim_clock`, `App.cast_wave`, `App.cast_mode`, `App.cast_buffer`, `App.inventory` (Task 5); `PowerupWord.tiles()`/`.name` (Task 1).
- Produces: in-module render helpers `shimmer_style(t: f32, i: usize) -> Style`, `hsl(h,s,l) -> Color`, `draw_cast_ring(buf, center, age, area)`, `draw_cast_buffer(f, area, &App)`. (Visuell; nur Smoke-Test.)

> Look-Parameter sind im Companion `examples/hud_lab.rs` final validiert (Szene 4/5): shimmer = gray→white-Band als reine Funktion `(t, index)`; Cast-Ring = transparenter Rainbow-Glyph-Ring (`hsl`, helle Pastelltöne, dünne Bande + Stipple, `RING_DUR ≈ 0.38`, QuadOut). Beide sind **render-time-Mathematik** (scroll-immun), keine tachyonfx-Effekte. Konstanten/Formeln 1:1 aus `hud_lab` übernehmen.

- [ ] **Step 1: Write the smoke test** (append to `render/mod.rs` tests)

```rust
    #[test]
    fn cast_flow_renders_many_frames_without_panic() {
        // Cast-Welle + Cast-Buffer + shimmer-Wort über viele Frames: darf nicht
        // paniken (render-time-Math, kein tachyonfx).
        use crate::game::arena::EntityKind;
        use crate::game::powerup::{Axis, PowerupWord};
        let mut app = App::new();
        app.arena_mut().unwrap().spawn(
            (3, 0),
            EntityKind::PowerupWord(PowerupWord {
                name: "dash".into(),
                origin: (3, 0),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
        app.cast_mode = true;
        app.cast_buffer = "da".into();
        app.cast_wave = Some(Duration::ZERO);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        for _ in 0..40 {
            terminal
                .draw(|f| draw(f, &mut app, Duration::from_millis(50)))
                .unwrap();
        }
    }
```

- [ ] **Step 2: Run to verify it fails/compiles-but-no-render-yet**

Run: `cargo test --lib cast_flow_renders`
Expected: PASS only after wiring — initially it passes trivially (no cast rendering), so first wire the helpers (Step 3) then assert the test still passes. (This test is a panic-guard, not a behavioral assert.)

- [ ] **Step 3: Add render-time helpers + advance clocks in `draw`**

In `draw`, advance the animation clock and the cast wave at the top, and clear the wave when expired:

```rust
pub fn draw(f: &mut Frame, app: &mut App, elapsed: Duration) {
    app.anim_clock += elapsed;
    if let Some(age) = app.cast_wave.as_mut() {
        *age += elapsed;
        if age.as_secs_f32() > RING_DUR {
            app.cast_wave = None;
        }
    }
    let area = f.area();
    let world = app.world_view();

    draw_world(f, area, &world, app.arena(), app.anim_clock);
    draw_hud(f, area, app, &world);
    app.notifications.render(f.buffer_mut(), area, elapsed);

    if app.cast_mode {
        draw_cast_buffer(f, area, app);
    }
    if let Some(age) = app.cast_wave {
        let center = ((area.width / 2) as i32, (area.height / 2) as i32);
        draw_cast_ring(f.buffer_mut(), center, age, area);
    }

    let self_dead = world.players.iter().any(|p| p.is_self && p.is_dead);
    if self_dead {
        draw_death_overlay(f, area);
    }
    if app.debug {
        draw_debug_overlay(f, app);
    }
}
```

Change `draw_world` signature to take the clock and apply shimmer to powerup tiles. Replace the powerup loop from Task 2:

```rust
fn draw_world(f: &mut Frame, area: Rect, world: &WorldView, arena: &Arena, clock: Duration) {
    // … existing w/h/center/cursor/grid setup …
    let t = clock.as_secs_f32();
    for e in &arena.entities {
        match &e.kind {
            EntityKind::PowerupWord(pw) => {
                let letters: Vec<char> = pw.name.chars().collect();
                for (i, tile) in pw.tiles().iter().enumerate() {
                    let rx = tile.0 - cursor.0 + center.0;
                    let ry = tile.1 - cursor.1 + center.1;
                    if rx < 0 || ry < 0 || rx >= w || ry >= h {
                        continue;
                    }
                    let ch = if pw.reversed {
                        letters[letters.len() - 1 - i]
                    } else {
                        letters[i]
                    };
                    grid[ry as usize][rx as usize] = Some((ch, shimmer_style(t, i)));
                }
            }
        }
    }
    // … rest unchanged …
}
```

Add the helpers (near the bottom of `render/mod.rs`, before `#[cfg(test)]`). Port verbatim from `hud_lab`:

```rust
/// Dauer der Cast-Ring-Animation (Sekunden) — snappy/dynamisch.
const RING_DUR: f32 = 0.38;

/// shimmer Idle-Style eines Powerup-Tiles: gray→white-Band, das übers Wort
/// wandert. Reine Funktion aus `(t, index)` → scroll-immun (Skill `effects`).
fn shimmer_style(t: f32, i: usize) -> Style {
    let phase = t * 7.0 - i as f32 * 0.95;
    let l = 0.5 + 0.5 * phase.sin();
    let v = (0x55 as f32 + (0xE6 - 0x55) as f32 * l).round() as u8;
    Style::default()
        .fg(Color::Rgb(v, v, (v as u16 + 7).min(255) as u8))
        .add_modifier(Modifier::BOLD)
}

/// HSL→RGB für den Rainbow-Cast-Ring (helle, pastellige Farben).
fn hsl(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color::Rgb(to(r), to(g), to(b))
}

/// Transparenter Rainbow-Glyph-Ring (gewählte Cast-Signatur): berührt NUR die
/// expandierende Ring-Bande — alle anderen Zellen bleiben unberührt, das
/// Spielfeld bleibt sichtbar. Render-time-Math (`sqrt(dx² + 4·dy²)`, 2:1-
/// Zellaspekt) → smear-frei über scrollendem Inhalt. Heller Pastell-Regenbogen
/// nach Winkel, dünne Bande + Stipple → luftig.
fn draw_cast_ring(buf: &mut Buffer, center: (i32, i32), age: Duration, area: Rect) {
    const MAXR: f32 = 17.0;
    const BAND: f32 = 1.5;
    let (cx, cy) = center;
    let p = (age.as_secs_f32() / RING_DUR).clamp(0.0, 1.0);
    let r = (1.0 - (1.0 - p) * (1.0 - p)) * MAXR; // QuadOut
    let life = 1.0 - p;
    for y in area.top() as i32..area.bottom() as i32 {
        for x in area.left() as i32..area.right() as i32 {
            let dxf = (x - cx) as f32;
            let dy = (y - cy) as f32 * 2.0;
            let d = (dxf * dxf + dy * dy).sqrt();
            let off = (d - r).abs();
            if off > BAND {
                continue;
            }
            let intensity = (1.0 - off / BAND) * life;
            if intensity < 0.12 {
                continue;
            }
            let hsh = (x as u64)
                .wrapping_mul(2_654_435_761)
                .wrapping_add((y as u64).wrapping_mul(40_503));
            if hsh % 5 < 2 {
                continue; // ~40 % Stipple → weniger dense
            }
            let hue = dy.atan2(dxf).to_degrees() + 360.0 + p * 50.0;
            let col = hsl(hue, 0.55, 0.74 + 0.12 * intensity);
            let ch = if intensity > 0.66 { '•' } else { '·' };
            if let Some(cell) = buf.cell_mut((x as u16, y as u16)) {
                cell.set_char(ch).set_fg(col);
            }
        }
    }
}

/// Cast-Buffer-Indikator (Powerup-Spec §7): gematchter Prefix im Pink-Kasten,
/// Rest gedämpft. Volles Inventar-Overlay-UI bleibt W3.
fn draw_cast_buffer(f: &mut Frame, area: Rect, app: &App) {
    let buf = &app.cast_buffer;
    // Längster Prefix-Match bestimmt den hervorgehobenen Teil.
    let suffix = app
        .inventory
        .prefix_matches(buf)
        .first()
        .map(|p| p.name[buf.len().min(p.name.len())..].to_string())
        .unwrap_or_default();
    let line = Line::from(vec![
        Span::styled(
            " cast ▸ ",
            Style::default()
                .fg(theme::ACCENT)
                .bg(theme::PANEL_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            buf.clone(),
            Style::default()
                .fg(theme::HIGHLIGHT_FG)
                .bg(theme::HIGHLIGHT_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(suffix, Style::default().fg(theme::TEXT_DIM).bg(theme::PANEL_BG)),
        Span::styled(" ", Style::default().bg(theme::PANEL_BG)),
    ]);
    let rect = anchor_rect(area, Anchor::BottomCenter, 28, 1);
    f.render_widget(Paragraph::new(line).style(Style::default().bg(theme::PANEL_BG)), rect);
}
```

> If `anchor_rect`/`Anchor` lacks a `BottomCenter`, use the existing variant set (check `src/hud/`); fall back to a manual centered `Rect` two rows above the bottom, mirroring `hud_lab`'s `draw_cast_buffer`.

- [ ] **Step 4: Run the smoke test + full suite**

Run: `cargo test --lib` → all pass (incl. `cast_flow_renders_many_frames_without_panic` and the updated `draw_world_renders_arena_entity_at_expected_cell` — note that with shimmer the first tile's char is unchanged, only its style; the symbol assertion still holds).
Run: `cargo test` → green.

- [ ] **Step 5: Clippy — no warnings**

Run: `cargo clippy --all-targets 2>&1 | tail -3` → no warnings.

- [ ] **Step 6: Manual visual check**

Run: `PRFH_DEBUG=1 cargo run` — confirm the `dash` word shimmers on the map, pickup works, `Tab`+`dash` fires the transparent rainbow ring with the playfield still visible behind it.

- [ ] **Step 7: Commit**

```bash
git add src/render/mod.rs
git commit -m "feat(#43): Visuals verdrahtet — shimmer Idle + transparenter Rainbow-Cast-Ring + Cast-Buffer"
```

---

## Task 8: Follow-up-Issue + PR finalisieren

**Files:** none (GitHub).

- [ ] **Step 1: Create the follow-up issue**

```bash
gh issue create \
  --title "chore: Test-Powerup (PRFH_DEBUG 'dash') entfernen/ersetzen" \
  --body "Folge aus #43: Das Test-Powerup (\`dash\`, spawnt unter PRFH_DEBUG neben dem Spieler-Start in \`App::new_single\`) ist nur ein Validierungs-Vehikel für den Flow Pickup→Inventar→Cast→Dispatch. Entfernen/ersetzen, sobald echtes Spawnen (W3) bzw. echte Powerups existieren."
```

- [ ] **Step 2: Link it in PR #49 body**

```bash
gh pr comment 49 --body "Follow-up zum Entfernen des Test-Powerups: #<neue-nr>."
```

- [ ] **Step 3: Final green gate**

Run: `cargo build && cargo test && cargo clippy --all-targets`
Expected: all green, zero warnings.

- [ ] **Step 4: Code-Review-Subagent auf den Diff**

Dispatch a `code-reviewer` subagent on the branch diff vs `main`. Focus: keine Brechung der Base-Mechanik in `writing.rs` (FSM ist Beobachter), FSM-Übergänge vollständig, host-autoritatives Despawn-Andockpunkt korrekt benannt, keine neuen Warnungen, Visuals render-time (kein tachyonfx über scrollendem Inhalt).

- [ ] **Step 5: `gh pr ready 49`**

After addressing review, merge current `main` in (Branch-Protection erzwingt up-to-date), confirm CI green, then `gh pr ready 49`.

---

## Self-Review

**1. Spec coverage:**
- Powerup-Spec §5 (Layout + Keystroke→Tile, beide Orientierungen) → Task 1 ✓
- §6 (Trace-FSM: Arming/advance/wrong-char/turn-reset/complete + Trigger-Suspendierung) → Task 3 ✓
- §7 (Cast-Modus: Tab-Toggle, Buffer, Trigger-Suspendierung, Dispatch-Hook) → Task 5 + Task 6 ✓
- §10 (Test-Powerup unter PRFH_DEBUG + Follow-up-Issue) → Task 5 + Task 8 ✓
- §11 (Prefix-Match unit-getestet) → Task 4 ✓
- World-Spec §4 (PowerupWord-Layout in W2, additiv) → Task 1 + Task 2 ✓
- World-Spec §11 (MP-Pickup-Race, host-autoritatives Despawn) → Task 5 (Single despawnt direkt; Host-Andockpunkt benannt; voller MP-Cast ist W3/MP-Folge — bewusst gescoped) ✓
- Issue #43 Akzeptanz: powerup.rs ✓, inventory.rs ✓, Trace-FSM ✓, Cast-Modus+Dispatch ✓, Test-Powerup+Follow-up ✓, grün&warnungsfrei ✓.
- Visuals (shimmer + Rainbow-Ring, im Companion validiert) → Task 7 ✓

**2. Placeholder scan:** Kein TBD/„handle edge cases"/„similar to Task N" — alle Code-Steps tragen echten Code. Einzige bewusste Verifikations-Schritte: Task 2/6/7 prüfen Konstruktionssites/Key-Handling/`anchor_rect`-Varianten per `grep` (real, nicht Platzhalter).

**3. Type consistency:** `PowerupWord { name, origin, axis, reversed }` einheitlich in Task 1/2/3/5/7. `Trace::observe(pos, ch, dir, &[(u32, &PowerupWord)]) -> TraceStep`, `TraceStep::Completed { id }` konsistent in Task 3/5. `Inventory::{prefix_matches, get_exact, add}`, `Powerup { id, name, effect_tag }`, `EffectTag::Test` konsistent in Task 1/4/5/7. `StepResult::tile()` in Task 3/5. `draw_world(..., clock: Duration)` + `App.anim_clock`/`cast_wave` in Task 5/7. Keine Drift gefunden.
