# Dash Skill + Targeting Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first real skill `dash` (8-way aimed teleport with animated preview beam) on top of a reusable skill-registry + generic aim/targeting framework that future power-ups plug into.

**Architecture:** A central `SkillDef` registry (`src/game/skill.rs`) is the single source of truth for which skills exist, their `rarity_weight`, their `EffectTag`, and how they activate (`Instant` vs `Targeted`). Casting a `Targeted` skill opens a generic aim-mode (`App.aim: Option<AimState>`) that intercepts arrow keys (rotate ±45° through 8 directions), Enter (fire), Esc (cancel). Dash fires a teleport (`dash_blink`, the default) or trail-burst (A/B in hud_lab). Visuals are render-time math (fg-only, scroll-immune, like the existing `draw_cast_ring`) plus a tachyonfx landing-pop.

**Tech Stack:** Rust 2021, Ratatui, Crossterm, tachyonfx 0.25, serde.

## Global Constraints

- `cargo build` and `cargo test` must be **green and warning-free** after every task (no `#[allow]` to hide warnings; remove dead code).
- `cargo fmt` style. Match surrounding naming, comment density, idioms (German comments are the house style here).
- cargo may not be on `PATH` — prepend the rustup toolchain if `cargo` is not found (per project memory `prfh-cargo-path`). Resolve the real path once at execution start; all `Run:` lines below assume `cargo` resolves.
- `App::new()` builds an **empty** arena (tests); `App::new_single()` seeds via `spawn_powerups`. Do NOT merge them.
- Render `WorldView` (`src/game/world.rs`) must NOT be renamed; sim world is `src/game/arena.rs`.
- tachyonfx `expand`/`stretch` panic on overshoot easings — only build `expand` via `safe_expand`. Over the scrolling field use render-time math, never `explode`/`evolve`/`glitch` (they blank the field).
- Dash is wired in `Mode::Single` only this PR. Multiplayer/net-sync is a deliberate follow-up issue — do NOT touch `src/net/` or `ServerMsg`.
- Commit message convention: `feat(#56): …` / `test(#56): …` / `refactor(#56): …`.

---

## File Structure

- **Create** `src/game/skill.rs` — `SkillDef`, `Activation`, `TargetingSpec`, `DirSet`, `Aim8`, `registry()`, `skill_def()`. One responsibility: the skill catalog + aim-direction type.
- **Modify** `src/game/mod.rs` — register `pub mod skill;`.
- **Modify** `src/game/powerup.rs` — add `EffectTag::Dash`; `spawn_powerups` sources names/metadata from the registry.
- **Modify** `src/game/writing.rs` — add `WritingEngine::dash_blink` + `dash_trail_burst`.
- **Modify** `src/app.rs` — `aim: Option<AimState>`, `AimState`, aim methods, `dispatch_cast` routes `Targeted` → aim, pickup `effect_tag` from registry.
- **Modify** `src/main.rs` — aim-mode input interception in `run()`.
- **Modify** `src/effects/mod.rs` — `dash_landing()`; extend `is_non_overshoot` to reject Bounce*/Spring.
- **Modify** `src/render/mod.rs` — `dash_beam_intensity()` (pure), `draw_dash_beam()`, advance aim age in `draw()`, `controls_line()` refactor.
- **Modify** `examples/hud_lab.rs` — new dash-aim scene (A/B beam styles + both mechanics + fire anim + 8-dir rotate + dynamic hint line).

---

## Task 1: Skill registry + descriptor + Aim8 direction type

**Files:**
- Create: `src/game/skill.rs`
- Modify: `src/game/mod.rs:1-5`
- Modify: `src/game/powerup.rs:24-27` (add `EffectTag::Dash`)

**Interfaces:**
- Consumes: `crate::game::powerup::EffectTag`, `crate::game::writing::Direction`.
- Produces:
  - `pub enum DirSet { Four, Eight }`
  - `pub struct TargetingSpec { pub dirs: DirSet, pub range: u16 }`
  - `pub enum Activation { Instant, Targeted(TargetingSpec) }`
  - `pub struct SkillDef { pub name: &'static str, pub rarity_weight: f32, pub effect_tag: EffectTag, pub activation: Activation }`
  - `pub fn registry() -> &'static [SkillDef]`
  - `pub fn skill_def(name: &str) -> Option<&'static SkillDef>` (case-insensitive)
  - `pub enum Aim8 { N, NE, E, SE, S, SW, W, NW }` with `delta(self) -> (i32,i32)`, `rotate(self, cw: bool) -> Aim8`, `nearest_cardinal(self) -> Direction`, `from_direction(d: Direction) -> Aim8`.

- [ ] **Step 1: Add `EffectTag::Dash`**

In `src/game/powerup.rs`, change the enum (around line 24):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectTag {
    Test,
    Dash,
}
```

- [ ] **Step 2: Register the module**

In `src/game/mod.rs` add the line (keep alphabetical-ish ordering with the others):

```rust
pub mod arena;
pub mod inventory;
pub mod powerup;
pub mod skill;
pub mod world;
pub mod writing;
```

- [ ] **Step 3: Write `src/game/skill.rs` with the types, registry, and failing tests**

```rust
//! Skill-Katalog: zentrale Beschreibung aller Powerups/Skills (Single Source of
//! Truth) + der generische 8-Wege-Zielvektor `Aim8`. `spawn_powerups` und der
//! Cast-Dispatch ziehen hieraus. `rarity_weight` ist als Property schon da —
//! prozedurale, gewichtete Welt-Generierung verdrahtet sie später.

use crate::game::powerup::EffectTag;
use crate::game::writing::Direction;

/// Welche Richtungen ein Targeting erlaubt. `dash` nutzt `Eight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirSet {
    Four,
    Eight,
}

/// Parameter eines gezielten Skills: Richtungs-Granularität + feste Reichweite
/// (in Tiles). Additiv erweiterbar (z.B. später regelbare Range / AoE-Radius).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetingSpec {
    pub dirs: DirSet,
    pub range: u16,
}

/// Wie ein Skill ausgelöst wird. `Instant` feuert sofort beim Cast; `Targeted`
/// öffnet den generischen Aim-Mode (Vorschau-Strahl, drehen, Enter feuert).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Activation {
    Instant,
    Targeted(TargetingSpec),
}

/// Statische Beschreibung eines Skills im Katalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillDef {
    pub name: &'static str,
    pub rarity_weight: f32,
    pub effect_tag: EffectTag,
    pub activation: Activation,
}

/// Der Katalog. Heute: `dash` (gezielt, 8 Richtungen, feste Distanz 6),
/// `revert`/`warp` als Instant-Platzhalter (echte Effekte später). Reihenfolge
/// = Seed-Reihenfolge von `spawn_powerups`.
pub fn registry() -> &'static [SkillDef] {
    &[
        SkillDef {
            name: "dash",
            rarity_weight: 1.0,
            effect_tag: EffectTag::Dash,
            activation: Activation::Targeted(TargetingSpec {
                dirs: DirSet::Eight,
                range: 6,
            }),
        },
        SkillDef {
            name: "revert",
            rarity_weight: 0.6,
            effect_tag: EffectTag::Test,
            activation: Activation::Instant,
        },
        SkillDef {
            name: "warp",
            rarity_weight: 0.3,
            effect_tag: EffectTag::Test,
            activation: Activation::Instant,
        },
    ]
}

/// Skill per Name (case-insensitiv) nachschlagen.
pub fn skill_def(name: &str) -> Option<&'static SkillDef> {
    registry()
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(name))
}

/// 8-Wege-Zielvektor des Aim-Modes. `Direction` (writing.rs) bleibt 4-Wege für
/// Write-to-Move; `Aim8` ist nur fürs Zielen. Reihenfolge im Kreis (im
/// Uhrzeigersinn ab Norden) für `rotate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aim8 {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Aim8 {
    /// Einheitsvektor in Tile-Koordinaten (y wächst nach unten).
    pub fn delta(self) -> (i32, i32) {
        match self {
            Aim8::N => (0, -1),
            Aim8::NE => (1, -1),
            Aim8::E => (1, 0),
            Aim8::SE => (1, 1),
            Aim8::S => (0, 1),
            Aim8::SW => (-1, 1),
            Aim8::W => (-1, 0),
            Aim8::NW => (-1, -1),
        }
    }

    /// Um 45° drehen: `cw` = im Uhrzeigersinn (N→NE→E…), sonst gegen.
    pub fn rotate(self, cw: bool) -> Aim8 {
        const RING: [Aim8; 8] = [
            Aim8::N,
            Aim8::NE,
            Aim8::E,
            Aim8::SE,
            Aim8::S,
            Aim8::SW,
            Aim8::W,
            Aim8::NW,
        ];
        let i = RING.iter().position(|&d| d == self).unwrap_or(0);
        let n = if cw { i + 1 } else { i + 7 } % 8;
        RING[n]
    }

    /// Nächstes Kardinal für die Write-to-Move-Richtung nach dem Dash.
    /// Diagonalen bevorzugen die Horizontale (Default-Lauf ist horizontal).
    pub fn nearest_cardinal(self) -> Direction {
        match self {
            Aim8::N => Direction::Up,
            Aim8::S => Direction::Down,
            Aim8::E | Aim8::NE | Aim8::SE => Direction::Right,
            Aim8::W | Aim8::NW | Aim8::SW => Direction::Left,
        }
    }

    /// Start-Zielrichtung aus der aktuellen Lauf-Richtung ableiten.
    pub fn from_direction(d: Direction) -> Aim8 {
        match d {
            Direction::Up => Aim8::N,
            Direction::Down => Aim8::S,
            Direction::Left => Aim8::W,
            Direction::Right => Aim8::E,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_the_three_starter_skills_in_order() {
        let names: Vec<&str> = registry().iter().map(|d| d.name).collect();
        assert_eq!(names, vec!["dash", "revert", "warp"]);
    }

    #[test]
    fn every_skill_has_a_positive_rarity_weight() {
        assert!(registry().iter().all(|d| d.rarity_weight > 0.0));
    }

    #[test]
    fn dash_is_targeted_eight_way_fixed_range() {
        let d = skill_def("dash").expect("dash registered");
        assert_eq!(d.effect_tag, EffectTag::Dash);
        match d.activation {
            Activation::Targeted(spec) => {
                assert_eq!(spec.dirs, DirSet::Eight);
                assert_eq!(spec.range, 6);
            }
            _ => panic!("dash must be Targeted"),
        }
    }

    #[test]
    fn skill_def_is_case_insensitive() {
        assert_eq!(skill_def("DASH").map(|d| d.name), Some("dash"));
        assert!(skill_def("nope").is_none());
    }

    #[test]
    fn aim8_delta_matches_compass() {
        assert_eq!(Aim8::N.delta(), (0, -1));
        assert_eq!(Aim8::E.delta(), (1, 0));
        assert_eq!(Aim8::SW.delta(), (-1, 1));
    }

    #[test]
    fn aim8_rotate_cycles_both_ways() {
        assert_eq!(Aim8::N.rotate(true), Aim8::NE);
        assert_eq!(Aim8::N.rotate(false), Aim8::NW);
        // Acht Schritte im Uhrzeigersinn = zurück am Start.
        let mut d = Aim8::N;
        for _ in 0..8 {
            d = d.rotate(true);
        }
        assert_eq!(d, Aim8::N);
    }

    #[test]
    fn aim8_nearest_cardinal_favors_horizontal_on_diagonals() {
        assert_eq!(Aim8::N.nearest_cardinal(), Direction::Up);
        assert_eq!(Aim8::NE.nearest_cardinal(), Direction::Right);
        assert_eq!(Aim8::SW.nearest_cardinal(), Direction::Left);
    }

    #[test]
    fn aim8_from_direction_roundtrips_cardinals() {
        assert_eq!(Aim8::from_direction(Direction::Up), Aim8::N);
        assert_eq!(Aim8::from_direction(Direction::Right), Aim8::E);
    }
}
```

- [ ] **Step 4: Run the tests, expect them to pass (module compiles)**

Run: `cargo test --lib skill::`
Expected: PASS (8 tests). If `EffectTag::Dash` or the module registration is missing it fails to compile — fix and rerun.

- [ ] **Step 5: Build warning-free**

Run: `cargo build`
Expected: success, no warnings. (`registry`/`skill_def` are used by later tasks; if the compiler warns "never used" that's expected only until Task 2 — acceptable within this task, resolved by Task 2. If you want it clean now, proceed straight to Task 2 before the final warning gate.)

- [ ] **Step 6: Commit**

```bash
git add src/game/skill.rs src/game/mod.rs src/game/powerup.rs
git commit -m "feat(#56): skill registry + descriptor + Aim8 direction type"
```

---

## Task 2: spawn_powerups sources from the registry; pickup tag from registry

**Files:**
- Modify: `src/game/powerup.rs:168-217` (`spawn_powerups` + its test)
- Modify: `src/app.rs:325-336` (pickup uses registry tag)

**Interfaces:**
- Consumes: `crate::game::skill::{registry, skill_def}`.
- Produces: unchanged public signatures; behavior now registry-driven.

- [ ] **Step 1: Rewrite `spawn_powerups` to iterate the registry**

Replace the body of `spawn_powerups` (and keep the doc comment) in `src/game/powerup.rs`:

```rust
/// Platziert die Start-Menge Powerup-Wörter in die Arena. Welche Skills
/// existieren, kommt aus `skill::registry()` (Single Source of Truth); die
/// Platzierung (Origin/Achse/Reversed) ist die feste Seed-Tabelle — Andockpunkt
/// für spätere prozedurale, `rarity_weight`-gewichtete Generierung (Welt-Spec §4).
pub fn spawn_powerups(arena: &mut Arena) {
    for def in crate::game::skill::registry() {
        if let Some((origin, axis, reversed)) = seed_placement(def.name) {
            arena.spawn(
                origin,
                EntityKind::PowerupWord(PowerupWord {
                    name: def.name.into(),
                    origin,
                    axis,
                    reversed,
                }),
            );
        }
    }
}

/// Feste Seed-Platzierung pro Skill-Name (gestreut, vom Start (0,0) weg).
/// Skills ohne Eintrag werden (noch) nicht auf der Map geseedet.
fn seed_placement(name: &str) -> Option<((i32, i32), Axis, bool)> {
    match name {
        "dash" => Some(((6, 0), Axis::Horizontal, false)),
        "revert" => Some(((0, 5), Axis::Vertical, false)),
        "warp" => Some(((-12, 3), Axis::Horizontal, true)),
        _ => None,
    }
}
```

- [ ] **Step 2: The existing spawn test still asserts the same names/origins**

The test `spawn_powerups_seeds_the_fixed_starter_set` (powerup.rs ~line 195) already asserts `names == ["dash","revert","warp"]` and `origins == [(6,0),(0,5),(-12,3)]`. Registry order matches, so it stays valid. No edit needed.

- [ ] **Step 3: Run it**

Run: `cargo test --lib powerup::tests::spawn_powerups_seeds_the_fixed_starter_set`
Expected: PASS.

- [ ] **Step 4: Pickup writes the registry's effect_tag**

In `src/app.rs`, inside `on_char`, the pickup block currently hardcodes `effect_tag: EffectTag::Test` (~line 330). Replace that `self.inventory.add(...)` call with:

```rust
                let effect_tag = crate::game::skill::skill_def(&name)
                    .map(|d| d.effect_tag.clone())
                    .unwrap_or(EffectTag::Test);
                self.inventory.add(Powerup {
                    id,
                    name: name.clone(),
                    effect_tag,
                });
```

- [ ] **Step 5: Add a test that a picked-up dash carries the Dash tag**

Append to the `w2_tests` module in `src/app.rs` (it already has `spawn_dash`):

```rust
    #[test]
    fn picking_up_dash_stores_the_dash_effect_tag() {
        let mut app = App::new();
        spawn_dash(&mut app);
        for ch in "xxxdash".chars() {
            app.on_char(ch);
        }
        assert_eq!(app.inventory.items[0].effect_tag, EffectTag::Dash);
    }
```

- [ ] **Step 6: Run the app tests**

Run: `cargo test --lib app::w2_tests`
Expected: PASS (including the new one). Note `cast_exact_name_dispatches_and_leaves_cast_mode` still passes here because `dispatch_cast` isn't changed until Task 4.

- [ ] **Step 7: Commit**

```bash
git add src/game/powerup.rs src/app.rs
git commit -m "feat(#56): spawn + pickup sourced from skill registry"
```

---

## Task 3: Engine dash mechanics (blink + trail-burst)

**Files:**
- Modify: `src/game/writing.rs` (add two methods to `impl WritingEngine`, after `on_backspace` ~line 388)

**Interfaces:**
- Produces:
  - `WritingEngine::dash_blink(&mut self, landing: (i32,i32), facing: Direction)` — teleports the cursor, sets direction, leaves a gap (no trail tiles).
  - `WritingEngine::dash_trail_burst(&mut self, dir_delta: (i32,i32), range: u16, facing: Direction)` — writes `range` trail tiles from the cursor along `dir_delta`, cursor ends at the landing tile.

- [ ] **Step 1: Write failing tests**

Append a test module at the end of `src/game/writing.rs`:

```rust
#[cfg(test)]
mod dash_tests {
    use super::*;

    #[test]
    fn blink_teleports_cursor_and_sets_facing_leaving_gap() {
        let mut e = WritingEngine::new((0, 0));
        e.on_char('a'); // one trail tile at (0,0), cursor now (1,0)
        let before = e.trail.len();
        e.dash_blink((7, 0), Direction::Right);
        assert_eq!(e.cursor, (7, 0), "cursor jumps to landing");
        assert_eq!(e.direction, Direction::Right);
        assert_eq!(e.trail.len(), before, "blink leaves a gap (no trail tiles)");
    }

    #[test]
    fn trail_burst_writes_range_tiles_and_lands() {
        let mut e = WritingEngine::new((0, 0)); // cursor (0,0)
        e.dash_trail_burst((1, 0), 4, Direction::Right);
        assert_eq!(e.cursor, (4, 0), "cursor ends at the landing tile");
        assert_eq!(e.trail.len(), 4, "exactly range tiles written");
        // Tiles sit on the stepped path p_0..p_3.
        let positions: Vec<(i32, i32)> = e.trail.iter().map(|t| t.pos).collect();
        assert_eq!(positions, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn trail_burst_keeps_dir_history_in_sync_for_backspace() {
        let mut e = WritingEngine::new((2, 2));
        e.dash_trail_burst((0, 1), 3, Direction::Down); // burst downward
        assert_eq!(e.dir_history.len(), e.trail.len(), "one history entry per tile");
        // Backspacing must not trip the desync debug_assert and walks tiles back.
        e.on_backspace();
        assert_eq!(e.trail.len(), 2);
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test --lib writing::dash_tests`
Expected: FAIL — `dash_blink` / `dash_trail_burst` not found.

- [ ] **Step 3: Implement the two methods**

Add inside `impl WritingEngine` (after `on_backspace`, before the closing `}` of the impl at ~line 388):

```rust
    /// Blink/Teleport: Cursor springt direkt auf `landing`, Lauf-Richtung wird
    /// auf `facing` gesetzt. Es entsteht bewusst eine Lücke (kein Trail zwischen
    /// alter und neuer Position) — der „Sprung"-Charakter des Dash.
    pub fn dash_blink(&mut self, landing: (i32, i32), facing: Direction) {
        self.cursor = landing;
        self.direction = facing;
    }

    /// Trail-Burst: schreibt sofort `range` Trail-Tiles ab dem Cursor entlang
    /// `dir_delta` (lückenlos), Cursor endet auf dem Lande-Tile. Hält
    /// `dir_history` Tile-für-Tile synchron (gleiche `tick`-Identität wie der
    /// Trail), damit `on_backspace` korrekt zurückläuft. Glyph ist ein neutrales
    /// Dash-Zeichen; `facing` ist die Lauf-Richtung danach.
    pub fn dash_trail_burst(&mut self, dir_delta: (i32, i32), range: u16, facing: Direction) {
        for _ in 0..range {
            self.dir_history.push((self.tick, self.direction));
            self.trail.push(Tile {
                pos: self.cursor,
                ch: '·',
                tick: self.tick,
                glow: 0,
                brightness: TILE_MAX_BRIGHTNESS,
                written_pace: self.pace,
            });
            self.cursor = (self.cursor.0 + dir_delta.0, self.cursor.1 + dir_delta.1);
            self.tick = self.tick.saturating_add(1);
        }
        self.direction = facing;
    }
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test --lib writing::dash_tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/game/writing.rs
git commit -m "feat(#56): engine dash mechanics — blink + trail-burst"
```

---

## Task 4: App aim-mode state, methods, and cast dispatch routing

**Files:**
- Modify: `src/app.rs` — imports, `App` struct field, `App::new`/`new_single` initializers, `dispatch_cast`, new aim methods, tests.

**Interfaces:**
- Consumes: `crate::game::skill::{Aim8, Activation, TargetingSpec, skill_def}`, `crate::game::writing::Direction`.
- Produces:
  - `pub struct AimState { pub skill_name: String, pub spec: TargetingSpec, pub dir: Aim8, pub age: Duration }`
  - `App.aim: Option<AimState>`
  - `App::start_aim(&mut self, skill_name: &str, spec: TargetingSpec)`
  - `App::aim_rotate(&mut self, cw: bool)`
  - `App::cancel_aim(&mut self)`
  - `App::fire_aim(&mut self)`
  - `App::advance_aim(&mut self, dt: Duration)`
- DEFAULT mechanic constant: `pub const DASH_DEFAULT_BURST: bool = false;` (false = Blink). Flip to switch the in-game mechanic.

- [ ] **Step 1: Add imports + the AimState struct**

In `src/app.rs`, extend the `use` for skill near the top (after the existing `use crate::game::powerup...` line, line 3):

```rust
use crate::game::skill::{skill_def, Activation, Aim8, TargetingSpec};
```

Add, near `PickupAnim` (after line 16 `pub const PICKUP_ANIM_DUR…`):

```rust
/// Generischer Aim-/Targeting-Zustand: aktiv, während ein gezielter Skill
/// platziert wird. `age` treibt die render-time Strahl-Animation (wie `cast_wave`).
pub struct AimState {
    pub skill_name: String,
    pub spec: TargetingSpec,
    pub dir: Aim8,
    pub age: Duration,
}

/// In-Game-Default-Mechanik des Dash: `false` = Blink/Teleport, `true` =
/// Trail-Burst. Beide sind im hud_lab A/B-bar; hier einzeilig umschaltbar.
pub const DASH_DEFAULT_BURST: bool = false;
```

- [ ] **Step 2: Add the field to `App` and both initializers**

In the `App` struct (after `pickup_anim` ~line 54) add:

```rust
    /// Aktiver Aim-Mode (generisch, von gezielten Skills wie dash genutzt).
    pub aim: Option<AimState>,
```

In BOTH `App::new()` (~line 77) and `App::new_single()` (~line 105), add `aim: None,` to the struct literal (next to `pickup_anim: None,`).

- [ ] **Step 3: Write failing tests**

Append to the `w3_tests` module in `src/app.rs`:

```rust
    #[test]
    fn casting_dash_opens_aim_mode_from_current_facing() {
        use crate::game::powerup::{EffectTag, Powerup};
        use crate::game::skill::Aim8;
        let mut app = App::new();
        app.inventory.add(Powerup {
            id: 0,
            name: "dash".into(),
            effect_tag: EffectTag::Dash,
        });
        app.toggle_cast();
        for ch in "dash".chars() {
            app.on_char(ch);
        }
        assert!(!app.cast_mode, "exact match leaves cast mode");
        let aim = app.aim.as_ref().expect("dash opens aim mode");
        assert_eq!(aim.skill_name, "dash");
        // Default facing is Right → Aim8::E.
        assert_eq!(aim.dir, Aim8::E);
    }

    #[test]
    fn aim_rotate_turns_the_beam() {
        use crate::game::skill::{Aim8, DirSet, TargetingSpec};
        let mut app = App::new();
        app.start_aim("dash", TargetingSpec { dirs: DirSet::Eight, range: 6 });
        app.aim_rotate(true);
        assert_eq!(app.aim.as_ref().unwrap().dir, Aim8::SE); // E → SE
        app.aim_rotate(false);
        app.aim_rotate(false);
        assert_eq!(app.aim.as_ref().unwrap().dir, Aim8::NE); // SE → E → NE
    }

    #[test]
    fn fire_aim_blinks_to_landing_clears_aim_and_pops() {
        use crate::game::skill::{DirSet, TargetingSpec};
        let mut app = App::new(); // cursor (0,0), facing Right → Aim8::E
        app.start_aim("dash", TargetingSpec { dirs: DirSet::Eight, range: 6 });
        app.fire_aim();
        assert!(app.aim.is_none(), "aim cleared after firing");
        assert!(app.cast_wave.is_some(), "landing pop fired");
        assert_eq!(app.local_engine().unwrap().cursor, (6, 0), "blinked 6 tiles east");
    }

    #[test]
    fn cancel_aim_clears_without_moving() {
        use crate::game::skill::{DirSet, TargetingSpec};
        let mut app = App::new();
        app.start_aim("dash", TargetingSpec { dirs: DirSet::Eight, range: 6 });
        app.cancel_aim();
        assert!(app.aim.is_none());
        assert_eq!(app.local_engine().unwrap().cursor, (0, 0), "cancel does not move");
    }

    #[test]
    fn advance_aim_ages_the_beam() {
        use crate::game::skill::{DirSet, TargetingSpec};
        let mut app = App::new();
        app.start_aim("dash", TargetingSpec { dirs: DirSet::Eight, range: 6 });
        app.advance_aim(std::time::Duration::from_millis(50));
        assert_eq!(app.aim.as_ref().unwrap().age, std::time::Duration::from_millis(50));
    }
```

- [ ] **Step 4: Run, expect failure**

Run: `cargo test --lib app::w3_tests`
Expected: FAIL — `start_aim`/`aim_rotate`/`fire_aim`/`cancel_aim`/`advance_aim` not found; `casting_dash_opens_aim_mode…` fails (dispatch unchanged).

- [ ] **Step 5: Route `dispatch_cast` and add the aim methods**

Replace `dispatch_cast` (src/app.rs ~line 244-257) with:

```rust
    /// Aktivierungs-Dispatch (Powerup-Spec §7). Gezielte Skills (`Targeted`)
    /// öffnen den Aim-Mode; `Instant`-Skills feuern sofort (Log + Banner +
    /// render-time-Welle über denselben EffectEvent-Seam wie der Pickup).
    fn dispatch_cast(&mut self, tag: EffectTag, name: &str) {
        if let Some(def) = skill_def(name) {
            if let Activation::Targeted(spec) = def.activation {
                self.start_aim(name, spec);
                self.debug_log(format!("aim open: {name}"));
                return;
            }
        }
        self.notifications
            .push(NotifyKind::Event, "⚡  CAST", name.to_string());
        self.debug_log(format!("cast dispatch: {name} ({tag:?})"));
        self.apply_effect_event(crate::game::powerup::EffectEvent::Activation {
            tag,
            name: name.to_string(),
        });
    }
```

Add the aim methods inside `impl App` (e.g. right after `dispatch_cast`):

```rust
    /// Aim-Mode öffnen: Start-Richtung aus der aktuellen Lauf-Richtung.
    pub fn start_aim(&mut self, skill_name: &str, spec: TargetingSpec) {
        let dir = self
            .local_engine()
            .map(|e| Aim8::from_direction(e.direction))
            .unwrap_or(Aim8::E);
        self.aim = Some(AimState {
            skill_name: skill_name.to_string(),
            spec,
            dir,
            age: Duration::ZERO,
        });
    }

    /// Zielstrahl um 45° drehen (`cw` = im Uhrzeigersinn).
    pub fn aim_rotate(&mut self, cw: bool) {
        if let Some(aim) = self.aim.as_mut() {
            aim.dir = aim.dir.rotate(cw);
        }
    }

    /// Aim-Mode ohne Wirkung verlassen.
    pub fn cancel_aim(&mut self) {
        self.aim = None;
    }

    /// Strahl-Alter fortschreiben (render-getrieben, wie `cast_wave`).
    pub fn advance_aim(&mut self, dt: Duration) {
        if let Some(aim) = self.aim.as_mut() {
            aim.age += dt;
        }
    }

    /// Dash abfeuern: Landepunkt = Cursor + Richtung·Range. Default-Mechanik
    /// Blink (Teleport); `DASH_DEFAULT_BURST` schaltet auf Trail-Burst. Danach
    /// Lande-Pop (Cast-Welle, render-zentriert auf dem neuen Cursor) + Banner.
    pub fn fire_aim(&mut self) {
        let Some(aim) = self.aim.take() else { return };
        let (dx, dy) = aim.dir.delta();
        let range = aim.spec.range as i32;
        let facing = aim.dir.nearest_cardinal();
        if let Mode::Single(e, _) = &mut self.mode {
            if DASH_DEFAULT_BURST {
                e.dash_trail_burst((dx, dy), aim.spec.range, facing);
            } else {
                let landing = (e.cursor.0 + dx * range, e.cursor.1 + dy * range);
                e.dash_blink(landing, facing);
            }
        }
        self.notifications
            .push(NotifyKind::Event, "⚡  DASH", aim.skill_name);
        self.cast_wave = Some(Duration::ZERO);
    }
```

- [ ] **Step 6: Run, expect pass**

Run: `cargo test --lib app::`
Expected: PASS. The earlier `cast_exact_name_dispatches_and_leaves_cast_mode` (w2_tests) casts "dash" — it now opens aim instead of firing the wave. Update that test next.

- [ ] **Step 7: Fix the now-stale w2 cast test**

In `src/app.rs` `w2_tests`, the test `cast_exact_name_dispatches_and_leaves_cast_mode` asserts `app.cast_wave.is_some()`. Dash is now `Targeted`, so casting it opens aim. Change the inventory item to an Instant skill so the test keeps testing the Instant dispatch path. Replace its body's powerup name and assertion:

```rust
    #[test]
    fn cast_exact_name_dispatches_and_leaves_cast_mode() {
        // "revert" ist Instant → feuert sofort die Cast-Welle. (dash ist jetzt
        // Targeted und öffnet stattdessen den Aim-Mode, separat getestet.)
        let mut app = App::new();
        app.inventory.add(Powerup {
            id: 0,
            name: "revert".into(),
            effect_tag: EffectTag::Test,
        });
        app.toggle_cast();
        for ch in "revert".chars() {
            app.on_char(ch);
        }
        assert!(!app.cast_mode, "exact match dispatches and exits cast mode");
        assert!(app.cast_wave.is_some(), "instant dispatch fired the cast wave");
        assert!(app.aim.is_none(), "instant skill does not open aim mode");
    }
```

- [ ] **Step 8: Run the full lib suite**

Run: `cargo test --lib`
Expected: PASS (all green).

- [ ] **Step 9: Commit**

```bash
git add src/app.rs
git commit -m "feat(#56): aim-mode state + cast routing (Targeted opens aim)"
```

---

## Task 5: Aim-mode input interception in the main loop

**Files:**
- Modify: `src/main.rs:135-143` (the `run()` key match)

**Interfaces:**
- Consumes: `App::aim`, `App::aim_rotate`, `App::fire_aim`, `App::cancel_aim`.

- [ ] **Step 1: Intercept keys when aim is active**

In `src/main.rs`, inside `run()`, immediately BEFORE the existing `match key.code {` (line 135), insert:

```rust
                // Aim-Mode fängt Pfeile/Enter/Esc ab und schluckt alles andere:
                // ◄/► drehen, Enter feuert, Esc bricht ab (kein Quit). Normale
                // Schreib-/Cast-Tasten sind währenddessen inaktiv.
                if app.aim.is_some() {
                    match key.code {
                        KeyCode::Left => app.aim_rotate(false),
                        KeyCode::Right => app.aim_rotate(true),
                        KeyCode::Enter => app.fire_aim(),
                        KeyCode::Esc => app.cancel_aim(),
                        _ => {}
                    }
                    app.tick();
                    continue;
                }
```

Note: `continue` returns to the `while` loop top; we call `app.tick()` first so visual state still advances on the swallowed frame (mirrors the normal path which calls `app.tick()` at the loop end). The `event::poll` block is inside the loop, so `continue` correctly skips the normal match.

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: success, no warnings.

- [ ] **Step 3: Manual smoke (interactive — optional but recommended)**

Run: `cargo run` then: type `dash` onto the word at (6,0) to pick it up, press `Tab`, type `dash`, confirm the aim beam appears (after Task 7), press `◄`/`►` to rotate, `Enter` to dash, `Esc` to cancel. (If running before Task 7, the beam is invisible but rotation/fire/cancel still work — verify via the debug overlay `F1` showing `aim open: dash`.)

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(#56): aim-mode input interception in main loop"
```

---

## Task 6: Effects — `dash_landing()` + extend the overshoot guard

**Files:**
- Modify: `src/effects/mod.rs` (`is_non_overshoot`, add `dash_landing`, tests)

**Interfaces:**
- Produces: `pub fn dash_landing() -> Effect` — a localized landing-pop for the hud_lab fire animation (coalesce + warm hue-shift). Pure tachyonfx on a small Rect.

- [ ] **Step 1: Write failing tests**

In `src/effects/mod.rs` `tests` module, extend `guard_rejects_overshoot_curves` and add a smoke test:

```rust
    #[test]
    fn guard_rejects_bounce_and_spring() {
        assert!(!is_non_overshoot(Interpolation::BounceOut));
        assert!(!is_non_overshoot(Interpolation::BounceIn));
        assert!(!is_non_overshoot(Interpolation::BounceInOut));
        assert!(!is_non_overshoot(Interpolation::Spring));
    }

    #[test]
    fn dash_landing_runs_to_end_without_panic() {
        run_to_end(dash_landing());
    }
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test --lib effects::`
Expected: FAIL — `dash_landing` undefined; `guard_rejects_bounce_and_spring` fails (guard doesn't list them yet). If `Interpolation::Spring`/`BounceOut` are not valid variants, adjust to the exact variant names tachyonfx 0.25 exposes (research confirmed `BounceIn/Out/InOut` and `Spring` exist — verify the exact identifiers with `cargo doc -p tachyonfx --no-deps` if compile fails).

- [ ] **Step 3: Extend the guard and add the constructor**

Replace `is_non_overshoot` (lines 14-24) with:

```rust
fn is_non_overshoot(c: Interpolation) -> bool {
    !matches!(
        c,
        Interpolation::BackIn
            | Interpolation::BackOut
            | Interpolation::BackInOut
            | Interpolation::ElasticIn
            | Interpolation::ElasticOut
            | Interpolation::ElasticInOut
            | Interpolation::BounceIn
            | Interpolation::BounceOut
            | Interpolation::BounceInOut
            | Interpolation::Spring
    )
}
```

Add the constructor (after `activation()`):

```rust
/// Dash-Lande-Pop: kurzer, lokalisierter Effekt am Lande-Tile (kleines Rect) —
/// die Zeichen sammeln sich (`coalesce`) mit warmem Hue-Shift. Bewusst KEIN
/// `explode`/`evolve` (die blanken das Feld). Wird im hud_lab über ein kleines
/// Rect prozessiert; In-Game übernimmt die zentrierte Cast-Welle den Pop.
pub fn dash_landing() -> Effect {
    fx::parallel(&[
        fx::coalesce((250, Interpolation::SineOut)),
        fx::hsl_shift_fg([60.0, 30.0, 40.0], (250, Interpolation::QuadOut)),
    ])
}
```

If `fx::hsl_shift_fg` is not exported in 0.25, fall back to `fx::hsl_shift(Some([60.0, 30.0, 40.0]), None, (250, Interpolation::QuadOut))` (verified non-panicking — at least one channel is `Some`).

- [ ] **Step 4: Run, expect pass**

Run: `cargo test --lib effects::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/effects/mod.rs
git commit -m "feat(#56): dash_landing effect + guard rejects bounce/spring"
```

---

## Task 7: Render — animated preview beam + dynamic controls line

**Files:**
- Modify: `src/render/mod.rs` — advance aim age in `draw()`; add `dash_beam_intensity` (pure) + `draw_dash_beam`; render beam when aiming; refactor bottom controls into `controls_line`.

**Interfaces:**
- Consumes: `App::aim`, `crate::game::skill::Aim8`.
- Produces:
  - `fn dash_beam_intensity(i: usize, age: Duration) -> f32` (pure, testable)
  - `fn draw_dash_beam(buf: &mut Buffer, center: (i32,i32), dir: (i32,i32), range: u16, age: Duration, area: Rect)`
  - `fn controls_line(app: &App) -> Line<'static>`

- [ ] **Step 1: Write failing tests for the pure helpers**

Add a test module at the very end of `src/render/mod.rs`:

```rust
#[cfg(test)]
mod dash_render_tests {
    use super::*;

    #[test]
    fn beam_intensity_is_in_unit_range_and_varies_with_age() {
        let a = dash_beam_intensity(2, Duration::from_millis(0));
        let b = dash_beam_intensity(2, Duration::from_millis(120));
        assert!((0.0..=1.0).contains(&a));
        assert!((0.0..=1.0).contains(&b));
        assert!((a - b).abs() > f32::EPSILON, "beam pulses over time");
    }

    #[test]
    fn controls_line_shows_aim_hints_only_while_aiming() {
        use crate::game::skill::{DirSet, TargetingSpec};
        let mut app = App::new();
        let normal = line_text(&controls_line(&app));
        assert!(normal.contains("cast"), "default shows cast hint");
        app.start_aim("dash", TargetingSpec { dirs: DirSet::Eight, range: 6 });
        let aiming = line_text(&controls_line(&app));
        assert!(aiming.contains("dash"), "aim mode shows dash hint");
        assert!(aiming.contains("drehen"), "aim mode shows rotate hint");
    }

    /// Helfer: den sichtbaren Text einer Line zusammensetzen.
    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test --lib render::dash_render_tests`
Expected: FAIL — `dash_beam_intensity` / `controls_line` not found.

- [ ] **Step 3: Add the pure beam intensity helper**

Add near `shimmer_style` in `src/render/mod.rs`:

```rust
/// Helligkeit/Intensität eines Strahl-Tiles `i` Schritte vom Cursor, zum
/// Zeitpunkt `age`. Reine Funktion (analog `trail_brightness`/`popup_pulse_line`)
/// → scroll-immun + unit-testbar. Fließender Sinus-Puls, der nach außen läuft.
fn dash_beam_intensity(i: usize, age: Duration) -> f32 {
    let phase = age.as_secs_f32() * 6.0;
    let wave = 0.5 + 0.5 * (i as f32 * 0.6 - phase).sin();
    (0.55 + 0.45 * wave).clamp(0.0, 1.0)
}
```

- [ ] **Step 4: Add the beam renderer**

Add after `draw_cast_ring` in `src/render/mod.rs`:

```rust
/// Animierter Dash-Vorschau-Strahl: render-time-Math, **fg-only** (wie
/// `draw_cast_ring` → transparent über dem scrollenden Feld). Zeichnet `range`
/// Tiles ab `center` (= Cursor-Bildschirmmitte) entlang `dir`, mit fließendem
/// Hue/Helligkeits-Puls, und ein Reticle `◎` am Lande-Tile.
fn draw_dash_beam(
    buf: &mut Buffer,
    center: (i32, i32),
    dir: (i32, i32),
    range: u16,
    age: Duration,
    area: Rect,
) {
    let in_bounds = |x: i32, y: i32| {
        x >= area.left() as i32
            && x < area.right() as i32
            && y >= area.top() as i32
            && y < area.bottom() as i32
    };
    for i in 1..=range as i32 {
        let x = center.0 + dir.0 * i;
        let y = center.1 + dir.1 * i;
        if !in_bounds(x, y) {
            continue;
        }
        let intensity = dash_beam_intensity(i as usize, age);
        let hue = 200.0 + i as f32 * 8.0 + age.as_secs_f32() * 60.0;
        let last = i == range as i32;
        let (ch, col) = if last {
            ('◎', hsl(hue, 0.6, 0.8))
        } else {
            let glyph = if dir.1 == 0 { '─' } else if dir.0 == 0 { '│' } else { '·' };
            (glyph, hsl(hue, 0.55, 0.45 + 0.35 * intensity))
        };
        if let Some(cell) = buf.cell_mut((x as u16, y as u16)) {
            cell.set_char(ch).set_fg(col);
        }
    }
}
```

- [ ] **Step 5: Advance aim age + render the beam in `draw()`**

In `draw()` (src/render/mod.rs ~line 43), after `app.advance_pickup_anim(elapsed);` add:

```rust
    app.advance_aim(elapsed);
```

Then, after the cast-ring block (after line 73, before `draw_inventory`), add the beam render. The world is cursor-centered, so the player cursor is at screen center:

```rust
    if let Some(aim) = app.aim.as_ref() {
        let center = ((area.width / 2) as i32, (area.height / 2) as i32);
        draw_dash_beam(
            f.buffer_mut(),
            center,
            aim.dir.delta(),
            aim.spec.range,
            aim.age,
            area,
        );
    }
```

Add `use crate::game::skill::Aim8;` is NOT needed (we call `aim.dir.delta()` via the value; `Aim8` is already the field type, method resolves). Ensure no unused import.

- [ ] **Step 6: Refactor the bottom controls into `controls_line`**

In `draw_hud` (src/render/mod.rs ~line 228-237), replace the `let controls = Line::from(vec![…]); f.render_widget(…)` block with:

```rust
    let controls = controls_line(app);
    let w = controls.width() as u16;
    f.render_widget(
        Paragraph::new(controls),
        anchor_rect(area, Anchor::BottomRight, w, 1),
    );
```

`draw_hud` must have access to `app` — check its signature. If `draw_hud(f, area, app, &world)` already passes `app` (it does per the `draw` call at line 63), use it directly. Then add the function near `shimmer_style`:

```rust
/// Untere Steuerzeile, abhängig vom App-Zustand: im Aim-Mode die Aim-Hints,
/// sonst der Default (cast/quit). Reine Funktion → unit-testbar.
fn controls_line(app: &App) -> Line<'static> {
    if app.aim.is_some() {
        Line::from(vec![
            Span::styled("◄ ►", Style::default().fg(theme::ACCENT)),
            Span::styled(" drehen · ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled("Enter", Style::default().fg(theme::ACCENT)),
            Span::styled(" dash · ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled("Esc", Style::default().fg(theme::ACCENT)),
            Span::styled(" ab", Style::default().fg(theme::TEXT_DIM)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(theme::ACCENT)),
            Span::styled(" cast · ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled("[Esc]", Style::default().fg(theme::ACCENT)),
            Span::styled(" quit", Style::default().fg(theme::TEXT_DIM)),
        ])
    }
}
```

- [ ] **Step 7: Run the render tests + full suite**

Run: `cargo test --lib render::dash_render_tests`
Expected: PASS.
Run: `cargo test`
Expected: PASS (whole suite).

- [ ] **Step 8: Build warning-free**

Run: `cargo build`
Expected: no warnings. Remove any now-unused imports flagged.

- [ ] **Step 9: Commit**

```bash
git add src/render/mod.rs
git commit -m "feat(#56): animated dash preview beam + dynamic controls line"
```

---

## Task 8: hud_lab dash-aim scene (A/B both mechanics + beam styles)

**Files:**
- Modify: `examples/hud_lab.rs` — add a new scene.

**Interfaces:** Standalone exploration; no production code depends on it. It MAY import the production helpers (`prfh::game::skill::Aim8`) to mirror in-game behavior.

> This is a throwaway visual sandbox. First READ `examples/hud_lab.rs` to learn its current patterns: the `State` struct (~line 717), the `main` event loop key match (~line 898), the `ui()` scene dispatch (~line 974), the per-scene `layout_*` functions, and `draw_help`. Then add the new scene following those exact patterns. Concrete code below is the scene's content; wire it in using the file's established conventions (next free scene number, a `State` sub-struct, key handlers, a help line).

- [ ] **Step 1: Add scene state**

Add fields to hud_lab's `State` struct mirroring the in-game aim state plus the A/B toggles:

```rust
    // Dash-Aim-Szene
    dash_dir: prfh::game::skill::Aim8,
    dash_age: std::time::Duration,
    dash_burst: bool,        // false = Blink, true = Trail-Burst
    dash_beam_style: u8,     // 0..=2, A/B der Strahl-Stile
    dash_fire: Option<std::time::Duration>, // Abfeuer-Anim-Alter
```

Initialize them in `State::new()` (`dash_dir: prfh::game::skill::Aim8::E`, ages `Duration::ZERO`, `dash_burst: false`, `dash_beam_style: 0`, `dash_fire: None`).

- [ ] **Step 2: Advance ages in `State::update(dt)`**

```rust
        self.dash_age += dt;
        if let Some(age) = self.dash_fire.as_mut() {
            *age += dt;
            if age.as_secs_f32() > 0.6 {
                self.dash_fire = None;
            }
        }
```

- [ ] **Step 3: Key handling for the scene**

In the `main` loop key match, add (use the next free scene digit, e.g. `'7'`, to switch to it), and while on that scene map:
- `Left`/`Right` → `state.dash_dir = state.dash_dir.rotate(false/true)`
- `Enter` → `state.dash_fire = Some(Duration::ZERO)`
- `Char('b')` → `state.dash_burst = !state.dash_burst`
- `Char('s')` → `state.dash_beam_style = (state.dash_beam_style + 1) % 3`

Follow the existing per-scene key dispatch style in the file (the file already branches on `state.scene`).

- [ ] **Step 4: Render function**

Add a `layout_dash_aim(f, area, state)` and call it from `ui()` for the new scene number. The camera here is FIXED (unlike the game), so the streak across the field is visible:

```rust
fn layout_dash_aim(f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &State) {
    use ratatui::style::{Color, Style};
    let buf = f.buffer_mut();
    let center = (
        area.left() as i32 + area.width as i32 / 3,
        area.top() as i32 + area.height as i32 / 2,
    );
    let (dx, dy) = state.dash_dir.delta();
    let range: i32 = 6;

    // Vorschau-Strahl (3 Stile per `s`).
    if state.dash_fire.is_none() {
        for i in 1..=range {
            let x = center.0 + dx * i;
            let y = center.1 + dy * i;
            if x < area.left() as i32 || x >= area.right() as i32
                || y < area.top() as i32 || y >= area.bottom() as i32 { continue; }
            let t = state.dash_age.as_secs_f32();
            let pulse = 0.5 + 0.5 * (i as f32 * 0.6 - t * 6.0).sin();
            let last = i == range;
            let (ch, col) = match state.dash_beam_style {
                0 => { // Flowing Gradient Pulse
                    let hue = 200.0 + i as f32 * 8.0 + t * 60.0;
                    let l = 0.45 + 0.35 * pulse;
                    (if last { '◎' } else if dy == 0 { '─' } else if dx == 0 { '│' } else { '·' },
                     hsl_lab(hue, 0.55, if last { 0.8 } else { l }))
                }
                1 => { // Charging Sweep: heller Kopf wandert
                    let head = ((t * 8.0) as i32 % range) + 1;
                    let bright = if i == head { 1.0 } else { 0.25 };
                    (if last { '◎' } else { '=' }, hsl_lab(190.0, 0.5, 0.35 + 0.5 * bright))
                }
                _ => { // Shimmer/Laser: stabil + Funkeln
                    let hsh = (x as u64).wrapping_mul(2654435761).wrapping_add(state.dash_age.as_millis() as u64);
                    let spark = (hsh % 7) < 1;
                    (if last { '◎' } else { '─' }, hsl_lab(330.0, 0.5, if spark { 0.9 } else { 0.5 }))
                }
            };
            if let Some(cell) = buf.cell_mut((x as u16, y as u16)) { cell.set_char(ch).set_fg(col); }
        }
    }

    // Abfeuer-Anim: Math-Streak (Blink: Geist-Spur; Burst: voller Trail).
    if let Some(age) = state.dash_fire {
        let p = (age.as_secs_f32() / 0.12).clamp(0.0, 1.0);
        let head = (p * range as f32) as i32;
        let lo = if state.dash_burst { 0 } else { (head - 2).max(0) };
        for i in lo..=head {
            let x = center.0 + dx * i;
            let y = center.1 + dy * i;
            if x < area.left() as i32 || x >= area.right() as i32
                || y < area.top() as i32 || y >= area.bottom() as i32 { continue; }
            let bright = if state.dash_burst { 0.8 } else { 1.0 - (head - i) as f32 * 0.3 };
            let col = hsl_lab(50.0, 0.7, (0.4 + 0.5 * bright).clamp(0.0, 1.0));
            if let Some(cell) = buf.cell_mut((x as u16, y as u16)) { cell.set_char('•').set_fg(col); }
        }
    }

    // Spieler-Glyph an center.
    if let Some(cell) = buf.cell_mut((center.0 as u16, center.1 as u16)) {
        cell.set_char('@').set_fg(Color::White);
    }
}

/// Lokale HSL→RGB-Kopie fürs Lab (das Spiel hat `hsl` privat in render).
fn hsl_lab(h: f32, s: f32, l: f32) -> ratatui::style::Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x),
        3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    ratatui::style::Color::Rgb(to(r), to(g), to(b))
}
```

- [ ] **Step 5: Help line**

In `draw_help` (or the scene's help branch), add for the new scene:
`◄ ► drehen · Enter dash · b Mechanik(Blink/Burst) · s Strahl-Stil`. Also show the current toggles (`state.dash_burst`, `state.dash_beam_style`) as text.

- [ ] **Step 6: Build and run the example**

Run: `cargo build --example hud_lab`
Expected: success, no warnings.
Run: `cargo run --example hud_lab` → switch to the new scene → rotate with ◄/►, fire with Enter, toggle mechanic with `b`, cycle beam style with `s`. Confirm all three beam styles animate and both fire variants render.

- [ ] **Step 7: Commit**

```bash
git add examples/hud_lab.rs
git commit -m "feat(#56): hud_lab dash-aim scene — A/B beam styles + both mechanics"
```

---

## Task 9: Final integration gate + conventions note

**Files:**
- Modify: `CLAUDE.md` (add a Skill-Registry/Aim-Mode convention line, if warranted)

- [ ] **Step 1: Full green + warning-free + fmt**

Run: `cargo fmt --all`
Run: `cargo build`
Run: `cargo build --example hud_lab`
Run: `cargo test`
Expected: all green, zero warnings.

- [ ] **Step 2: Clippy (if used in CI)**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no errors. Fix any lints (prefer real fixes over `#[allow]`).

- [ ] **Step 3: Add a conventions line to CLAUDE.md**

Under "Code-Konventionen", add a short entry so the next instance finds the new subsystem:

```markdown
- **Skills/Powerups** (#56): Der Skill-Katalog lebt in `src/game/skill.rs`
  (`SkillDef` mit `rarity_weight` + `Activation::{Instant,Targeted}`, `registry()`
  als Single Source of Truth, `Aim8` als 8-Wege-Zielvektor). `spawn_powerups`
  zieht daraus. Gezielte Skills nutzen den generischen Aim-Mode (`App.aim`,
  Pfeile drehen / Enter feuert / Esc bricht ab); der Vorschau-Strahl ist
  render-time-Math (`draw_dash_beam`, fg-only wie `draw_cast_ring`). Dash ist
  vorerst nur `Mode::Single` verdrahtet — MP-Netz-Sync ist ein Follow-up.
```

- [ ] **Step 4: Commit + push + mark PR ready**

```bash
git add CLAUDE.md
git commit -m "docs(#56): note skill-registry + aim-mode conventions"
git push origin issue-56
```

- [ ] **Step 5: Self-check against the issue's acceptance criteria**

Open issue #56 and verify each checkbox is satisfied (registry, spawn-from-registry, cast routing, aim input, landing math, dynamic HUD, both mechanics, beam, fire anim, guard extension, hud_lab scene, tests green). Then request review (`gh pr ready 57` if still draft; cross-review per CLAUDE.md). Do NOT self-merge without explicit human OK.

---

## Self-Review (plan author)

**Spec coverage:**
- Registry + descriptor + rarity_weight → Task 1. ✓
- spawn_powerups from registry → Task 2. ✓
- Cast dispatch Targeted→aim / Instant→now → Task 4. ✓
- Aim-mode state + input (◄/► rotate, Enter fire, Esc cancel) → Tasks 4 + 5. ✓
- Landing = cursor + dir·range → Task 4 (`fire_aim`). ✓
- Dynamic HUD controls → Task 7 (`controls_line`). ✓
- Both mechanics (blink + trail-burst) → Task 3 (engine) + Task 8 (A/B). ✓
- Animated beam (render math) → Task 7. ✓
- Fire animation (math streak + tachyonfx pop) → Task 6 (`dash_landing`) + Task 8 (hud_lab streak) + Task 4 (in-game cast-wave pop). ✓
- Guard extension Bounce*/Spring → Task 6. ✓
- hud_lab scene → Task 8. ✓
- Tests (registry, aim rotation, landing math, blink/burst pure, beam pure fn(age), dash_landing run_to_end, controls_line) → Tasks 1,3,4,6,7. ✓
- Single-player only / no net touch → Global Constraints. ✓

**Placeholder scan:** No TBD/TODO; every code step shows real code. hud_lab (Task 8) is explicitly a sandbox with concrete scene code + "follow existing patterns" wiring — acceptable for a throwaway example, exact insertion points named.

**Type consistency:** `Aim8`, `TargetingSpec { dirs, range }`, `Activation::Targeted`, `SkillDef { name, rarity_weight, effect_tag, activation }`, `AimState { skill_name, spec, dir, age }`, `dash_blink(landing, facing)`, `dash_trail_burst(dir_delta, range, facing)`, `dash_beam_intensity(i, age)`, `controls_line(app)` — names used consistently across Tasks 1–8.
