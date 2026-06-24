# Powerup-Pickup-Gefühl Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Das Powerup-Pickup-Erlebnis verbessern: tolerantes Andocken (A), das Wort vorm optischen Überschreiben schützen (B), und Trace-Feedback verdrahten (C).

**Architecture:** A lebt als reine `entry_snap`-Methode auf `PowerupWord` (unit-testbar) plus ein ≤1-Tile-Nudge in `app.rs` *vor* `on_char` — die Trace-FSM bleibt unangetastet und sieht nach dem Snap ein exaktes Eintritts-Tile. B+C sind reine Render-Eingriffe in `draw_world` (Reihenfolge umstellen, Trace-Info durchreichen, render-time-Math). Visuals werden zuerst im Companion `examples/hud_lab.rs` Szene 4 exploriert.

**Tech Stack:** Rust 2021, Ratatui, Crossterm, tachyonfx (nur indirekt). Tests via `cargo test`.

## Global Constraints

- `cargo build` / `cargo test` / `cargo clippy` müssen grün **und warnungsfrei** sein. Kein `#[allow]` zum Verstecken.
- `cargo` ist nicht im PATH — jeder Befehl braucht zuvor: `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"`
- Rust 2021, `cargo fmt`-Stil. Code passt zu Naming/Kommentar-Dichte des umgebenden Codes (deutsche Kommentare).
- Trace-FSM (`writing.rs`) bleibt **Beobachter** und wird **nicht** umgebaut. Bestehende FSM-Tests bleiben unverändert grün.
- Visuals = **render-time-Math, scroll-immun** — kein tachyonfx über scrollendem Welt-Inhalt.
- Alles im selben Issue/Branch/PR: #43 / `issue-43` / PR #49.
- Farben nur aus `src/theme.rs` referenzieren, keine eigenen Hex-Werte.

---

### Task 1: A — `entry_snap` auf `PowerupWord` (reine Logik)

**Files:**
- Modify: `src/game/powerup.rs` (neue Konstante + Methode + Tests im bestehenden `tests`-Modul)

**Interfaces:**
- Produces: `PowerupWord::entry_snap(&self, cursor: (i32,i32), dir_delta: (i32,i32), ch: char, radius: i32) -> Option<(i32,i32)>` und `pub const ENTRY_SNAP_RADIUS: i32 = 1;`
- Consumes: bestehende `PowerupWord::{entry_tile, run_direction, expected_char, len}`.

- [ ] **Step 1: Failing tests schreiben**

In `src/game/powerup.rs`, im `#[cfg(test)] mod tests`, ans Ende einfügen:

```rust
#[test]
fn entry_snap_exact_hit_is_noop() {
    // Exakter Treffer → Snap-Ziel == aktuelle Position (no-op, heute-kompatibel).
    let w = word("dash", (3, 0), Axis::Horizontal, false);
    assert_eq!(w.entry_snap((3, 0), (1, 0), 'd', ENTRY_SNAP_RADIUS), Some((3, 0)));
}

#[test]
fn entry_snap_pulls_from_one_row_off() {
    // Eine Reihe versetzt, richtige Richtung + Buchstabe → snappt aufs Eintritts-Tile.
    let w = word("dash", (3, 0), Axis::Horizontal, false);
    assert_eq!(w.entry_snap((3, 1), (1, 0), 'd', ENTRY_SNAP_RADIUS), Some((3, 0)));
    assert_eq!(w.entry_snap((2, 1), (1, 0), 'd', ENTRY_SNAP_RADIUS), Some((3, 0)));
}

#[test]
fn entry_snap_rejects_out_of_radius() {
    let w = word("dash", (3, 0), Axis::Horizontal, false);
    assert_eq!(w.entry_snap((3, 2), (1, 0), 'd', ENTRY_SNAP_RADIUS), None);
}

#[test]
fn entry_snap_rejects_wrong_direction() {
    let w = word("dash", (3, 0), Axis::Horizontal, false);
    // Nah + richtiger Buchstabe, aber läuft nach unten statt nach rechts.
    assert_eq!(w.entry_snap((3, 1), (0, 1), 'd', ENTRY_SNAP_RADIUS), None);
}

#[test]
fn entry_snap_rejects_wrong_char() {
    let w = word("dash", (3, 0), Axis::Horizontal, false);
    assert_eq!(w.entry_snap((3, 1), (1, 0), 'x', ENTRY_SNAP_RADIUS), None);
}

#[test]
fn entry_snap_single_char_ignores_direction() {
    // 1-Buchstaben-Wort hat keine Lauf-Achse → Richtung egal.
    let w = word("x", (2, 2), Axis::Horizontal, false);
    assert_eq!(w.entry_snap((2, 3), (0, -1), 'x', ENTRY_SNAP_RADIUS), Some((2, 2)));
}
```

- [ ] **Step 2: Test laufen lassen → schlägt fehl**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test -p prfh --lib game::powerup 2>&1 | tail -20
```
Erwartet: FAIL — `no method named entry_snap` / `cannot find value ENTRY_SNAP_RADIUS`.

- [ ] **Step 3: Implementierung**

In `src/game/powerup.rs`, nach den anderen Konstanten/`use`-Zeilen oben (vor `impl PowerupWord` reicht eine `pub const`):

```rust
/// Toleranz-Radius (Chebyshev) fürs Andocken: wie weit neben dem Eintritts-Tile
/// der Cursor stehen darf und trotzdem aufs Wort gesnappt wird.
pub const ENTRY_SNAP_RADIUS: i32 = 1;
```

In `impl PowerupWord`, neue Methode (nach `run_direction`):

```rust
/// Snap-Ziel fürs tolerante Andocken: `Some(entry_tile)`, wenn der Cursor nah
/// genug am Eintritts-Tile ist (Chebyshev ≤ `radius`), in Laufrichtung anfährt
/// und der erste Buchstabe stimmt. Sonst `None`. `dir_delta` als `(i32,i32)`,
/// um keinen `Direction`-Import (writing.rs) hereinzuziehen. 1-Buchstaben-Wörter
/// haben keine Lauf-Achse → Richtungs-Bedingung entfällt (wie in der Trace-FSM).
pub fn entry_snap(
    &self,
    cursor: (i32, i32),
    dir_delta: (i32, i32),
    ch: char,
    radius: i32,
) -> Option<(i32, i32)> {
    let entry = self.entry_tile();
    let cheb = (cursor.0 - entry.0).abs().max((cursor.1 - entry.1).abs());
    let dir_ok = self.len() <= 1 || dir_delta == self.run_direction();
    let char_ok = self.expected_char(0) == Some(ch.to_ascii_lowercase());
    (cheb <= radius && dir_ok && char_ok).then_some(entry)
}
```

- [ ] **Step 4: Tests laufen lassen → grün**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test -p prfh --lib game::powerup 2>&1 | tail -20
```
Erwartet: PASS (alle `entry_snap_*` plus die bestehenden).

- [ ] **Step 5: Commit**

```bash
git add src/game/powerup.rs
git commit -m "feat(#43): PowerupWord::entry_snap — toleranter Andock-Radius (Pickup-Gefühl A)"
```

---

### Task 2: A — Snap-on-Arm in `app.rs` verdrahten

**Files:**
- Modify: `src/app.rs` (`on_char`, Single-Branch; Import; neuer Test im `w2_tests`-Modul)

**Interfaces:**
- Consumes: `PowerupWord::entry_snap` + `ENTRY_SNAP_RADIUS` (Task 1); `Trace::is_tracing`; `WritingEngine::{cursor, direction}`; `Direction::delta`.
- Produces: keine neue öffentliche API — Verhaltensänderung im Single-Pickup-Flow.

- [ ] **Step 1: Failing test schreiben**

In `src/app.rs`, im `#[cfg(test)] mod w2_tests`, ans Ende einfügen:

```rust
#[test]
fn snap_picks_up_word_when_approaching_one_row_off() {
    // Spieler läuft Right, aber eine Reihe UNTER dem Wort (y=1 statt y=0).
    // Ohne Snap würde "dash" nie armen; mit Snap rastet 'd' aufs Eintritts-Tile.
    let mut app = App::new(); // Cursor (0,0), Richtung Right
    app.arena_mut().unwrap().spawn(
        (3, 0),
        EntityKind::PowerupWord(PowerupWord {
            name: "dash".into(),
            origin: (3, 0),
            axis: Axis::Horizontal,
            reversed: false,
        }),
    );
    // Cursor auf (2,1) bringen: 3 Filler im Idle (eine Reihe unter dem Wort).
    if let Mode::Single(e, _) = &mut app.mode {
        e.cursor = (2, 1);
    }
    for ch in "dash".chars() {
        app.on_char(ch);
    }
    assert_eq!(app.inventory.len(), 1, "Snap sollte das Andocken erlauben");
    assert_eq!(app.inventory.items[0].name, "dash");
    assert!(app.arena().entities.is_empty(), "Wort despawnt nach Pickup");
}
```

- [ ] **Step 2: Test laufen lassen → schlägt fehl**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test -p prfh --lib app::w2_tests::snap_picks_up_word 2>&1 | tail -20
```
Erwartet: FAIL — `inventory.len()` == 0 (kein Snap, Trace armt nie).

- [ ] **Step 3: Implementierung**

In `src/app.rs` oben den Import erweitern:

```rust
use crate::game::powerup::{EffectTag, Powerup, PowerupWord, ENTRY_SNAP_RADIUS};
```

In `on_char`, im `if let Mode::Single(e, arena) = &mut self.mode {`-Block, **direkt nach** `let dir = e.direction;` und **vor** `e.trace_suspended = ...`:

```rust
            // Toleranter Snap-on-Arm (Pickup-Gefühl A): steht der Cursor ≤1 Tile
            // neben einem Eintritts-Tile, fährt in Laufrichtung an und passt der
            // erste Buchstabe, rastet er aufs Eintritts-Tile ein — BEVOR on_char
            // schreibt. Nur im Idle (laufender Trace soll nicht weggerissen
            // werden). Bei exaktem Treffer ist es ein no-op. Die Trace-FSM bleibt
            // Beobachter und sieht danach ein exaktes Eintritts-Tile.
            if !self.trace.is_tracing() {
                let dd = dir.delta();
                if let Some(target) = arena.entities.iter().find_map(|ent| match &ent.kind {
                    EntityKind::PowerupWord(w) => w.entry_snap(e.cursor, dd, c, ENTRY_SNAP_RADIUS),
                }) {
                    e.cursor = target;
                }
            }
```

- [ ] **Step 4: Tests laufen lassen → grün**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test -p prfh --lib app:: 2>&1 | tail -20
```
Erwartet: PASS — neuer Test grün, `tracing_word_picks_it_up_into_inventory_and_despawns` (exakter Pfad, Snap = no-op) bleibt grün.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat(#43): Snap-on-Arm in on_char — toleranter Pickup, FSM bleibt Beobachter (A)"
```

---

### Task 3: B — Powerup-Wort als Top-Layer rendern

**Files:**
- Modify: `src/render/mod.rs` (`draw_world`: Entity-Loop hinter den Trail-Loop verschieben; neuer Test im `tests`-Modul)

**Interfaces:**
- Consumes: bestehende `draw_world`-Signatur (unverändert in diesem Task).
- Produces: Render-Reihenfolge `Trails → Powerup-Wörter → Cursor`.

- [ ] **Step 1: Failing test schreiben**

In `src/render/mod.rs`, im `#[cfg(test)] mod tests`, ans Ende einfügen:

```rust
#[test]
fn powerup_word_not_hidden_by_trail_tile() {
    use crate::app::Mode;
    use crate::game::arena::EntityKind;
    use crate::game::powerup::{Axis, PowerupWord};
    use crate::game::writing::{Tile, TILE_MAX_BRIGHTNESS};
    let mut app = App::new();
    // Wort offset vom Cursor (Cursor-Marker soll nicht stören): origin (5,-2).
    app.arena_mut().unwrap().spawn(
        (5, -2),
        EntityKind::PowerupWord(PowerupWord {
            name: "zoom".into(),
            origin: (5, -2),
            axis: Axis::Horizontal,
            reversed: false,
        }),
    );
    // Ein Trail-Tile genau AUF das erste Wort-Tile (5,-2) legen.
    if let Mode::Single(e, _) = &mut app.mode {
        e.trail.push(Tile {
            pos: (5, -2),
            ch: 'Q',
            tick: 99,
            glow: 0,
            brightness: TILE_MAX_BRIGHTNESS,
            written_pace: 0.0,
        });
    }
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw(f, &mut app, Duration::ZERO)).unwrap();
    let buf = terminal.backend().buffer();
    // Screen-Transform: (5,-2) - (0,0) + (40,12) = (45,10).
    assert_eq!(
        buf.cell((45, 10)).unwrap().symbol(),
        "z",
        "Powerup-Wort muss über dem Trail liegen (Top-Layer)"
    );
}
```

- [ ] **Step 2: Test laufen lassen → schlägt fehl**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test -p prfh --lib render::tests::powerup_word_not_hidden 2>&1 | tail -20
```
Erwartet: FAIL — Zelle zeigt `"Q"` statt `"z"` (Trail liegt heute über dem Wort).

- [ ] **Step 3: Implementierung — Entity-Loop verschieben**

In `src/render/mod.rs`, `draw_world`: den kompletten Entity-Loop (`for e in &arena.entities { ... }`, aktuell vor dem Trail-Loop) **ausschneiden** und **nach** dem Trail-Loop (`for (tile, color, is_self) in &all_tiles { ... }`) **vor** dem Cursor-Loop (`for player in &world.players { ... }`) wieder einfügen. Den Kommentar oben am Entity-Loop anpassen:

```rust
    // Powerup-Wörter NACH den Trails zeichnen → Top-Layer: der eigene Trail
    // überdeckt das Wort nicht mehr (Pickup-Gefühl B). Cursor-zentrierte
    // Transform wie die Tiles; jedes Tile an seiner Position, Shimmer-Idle-Look.
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
```

- [ ] **Step 4: Tests laufen lassen → grün**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test -p prfh --lib render:: 2>&1 | tail -20
```
Erwartet: PASS — neuer Test grün; `draw_world_renders_arena_entity_at_expected_cell` bleibt grün.

- [ ] **Step 5: Commit**

```bash
git add src/render/mod.rs
git commit -m "feat(#43): Powerup-Wort als Top-Layer (über Trails) — kein Überschreiben mehr (B)"
```

---

### Task 4: C — hud_lab Szene 4: Next-Tile-Telegraph + Cursor-Unterdrückung explorieren

**Files:**
- Modify: `examples/hud_lab.rs` (`layout_powerup`, Szene 4)

**Interfaces:**
- Rein visuell/explorativ. Kein Spiel-Code, keine Tests. Dient als A/B-Bild für den User, bevor Task 5 den Look ins Spiel verdrahtet.

**Kontext:** Szene 4 hat bereits Trace-Highlight (`traced`-Tiles im `HIGHLIGHT_BG`-Kasten) und den Eintritts-Marker (`run_glyph` auf `entry - run_dir`, `ACCENT`-bg). NEU zu zeigen: das **nächste** erwartete Tile (Tile `traced`, = Cursor-Position) hebt sich vom bereits-getracten Block ab — in Cursor-BG-Farbe (`ACCENT`) bzw. leicht heller — und an dieser Zelle steht **kein** Cursor-Pfeil.

- [ ] **Step 1: Next-Tile-Highlight in `layout_powerup` ergänzen**

In `examples/hud_lab.rs`, in `layout_powerup`, im `for i in 0..n`-Loop die `traced`/`style`-Logik so erweitern, dass das nächste Tile (`i == state.traced`, nur wenn noch nicht alles getract ist) einen eigenen Stil bekommt. Den `style`-Block ersetzen durch:

```rust
        // Nächstes erwartetes Tile = wo der nächste Buchstabe hin muss
        // (in Tipp-Reihenfolge das Tile direkt hinter dem getracten Block).
        let is_next = if reversed {
            state.traced < n && i == n - 1 - state.traced
        } else {
            state.traced < n && i == state.traced
        };
        let next_style = Style::default()
            .fg(theme::HIGHLIGHT_FG)
            .bg(theme::ACCENT)
            .add_modifier(Modifier::BOLD);
        let style = if is_next {
            next_style
        } else if traced {
            traced_style
        } else {
            state.word_style.style_at(state.frame, i, n)
        };
```

- [ ] **Step 2: Companion bauen & starten, mit User durchschalten**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo build --example hud_lab 2>&1 | tail -5
```
Dann den User bitten, `cargo run --example hud_lab` zu starten, zu Szene 4 zu wechseln und den Trace mit der zugehörigen Taste durchzuschalten. **Checkpoint:** Mit dem User klären:
- Next-Tile-Farbe: `ACCENT`-bg (wie Cursor) ODER „leicht heller" als der Highlight-Block?
- Bleibt der Eintritts-Marker sichtbar, sobald der Trace läuft, oder nur im Idle?
- Sehen die `> traced`-Tiles gut aus mit Shimmer, oder gedämpft?

Die Antworten in `style`-Konstanten festhalten (ggf. anpassen). **Nicht** weiter, bis der User den Look bestätigt.

- [ ] **Step 3: Commit (gewählter Look)**

```bash
git add examples/hud_lab.rs
git commit -m "feat(#43): hud_lab Szene 4 — Next-Tile-Telegraph (Pickup-Feedback C explored)"
```

---

### Task 5: C — Trace-Feedback in `draw_world` verdrahten

**Files:**
- Modify: `src/render/mod.rs` (`draw`: Trace-Info ableiten + durchreichen; `draw_world`: Signatur + Entity-Loop-Styling + Cursor-Unterdrückung; neuer Smoke-Test)

**Interfaces:**
- Consumes: `app.trace.state` (`TraceState::{Idle, Tracing{id, progress}}`); die Top-Layer-Reihenfolge aus Task 3; den in Task 4 bestätigten Look (`HIGHLIGHT_BG`-Block, `ACCENT` Next-Tile).
- Produces: `draw_world(f, area, world, arena, clock, trace)` mit `trace: Option<(u32, usize)>` (EntityId + progress des aktiven Trace).

- [ ] **Step 1: Failing smoke-test schreiben**

In `src/render/mod.rs`, im `#[cfg(test)] mod tests`, ans Ende einfügen:

```rust
#[test]
fn trace_feedback_renders_many_frames_without_panic() {
    // Aktiver Trace-State (getractes Wort + Next-Tile-Highlight + unterdrückter
    // Cursor) über viele Frames: reine render-time-Math, darf nicht paniken.
    use crate::game::arena::EntityKind;
    use crate::game::powerup::{Axis, PowerupWord};
    use crate::game::writing::TraceState;
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
    // Trace mitten im Wort: id muss zur gespawnten Entität passen.
    let id = app.arena().entities[0].id;
    app.trace.state = TraceState::Tracing { id, progress: 2 };
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    for _ in 0..40 {
        terminal
            .draw(|f| draw(f, &mut app, Duration::from_millis(50)))
            .unwrap();
    }
}
```

- [ ] **Step 2: Test laufen lassen → schlägt fehl (Kompilierfehler)**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test -p prfh --lib render::tests::trace_feedback 2>&1 | tail -20
```
Erwartet: FAIL — `TraceState` / `app.trace.state`-Zugriff bzw. spätere Signatur kompiliert noch nicht (der Test erzwingt das Feature). (Falls `TraceState`/`trace.state` schon pub sind, schlägt der Test erst nach Step-3-Signaturänderung sinnvoll an; entscheidend ist grün nach Step 4.)

- [ ] **Step 3: `draw` — Trace-Info ableiten und durchreichen**

In `src/render/mod.rs` den Import erweitern:

```rust
use crate::game::writing::{Direction, TraceState};
```

In `draw`, vor dem `draw_world`-Aufruf, die aktive Trace-Info ableiten und übergeben:

```rust
    let trace: Option<(u32, usize)> = match app.trace.state {
        TraceState::Tracing { id, progress } => Some((id, progress)),
        TraceState::Idle => None,
    };

    draw_world(f, area, &world, app.arena(), clock, trace);
```

- [ ] **Step 4: `draw_world` — Signatur, Wort-Styling, Cursor-Unterdrückung**

`draw_world`-Signatur erweitern:

```rust
fn draw_world(
    f: &mut Frame,
    area: Rect,
    world: &WorldView,
    arena: &Arena,
    clock: Duration,
    trace: Option<(u32, usize)>,
) {
```

Im (in Task 3 verschobenen) Entity-Loop das Styling Trace-bewusst machen. Den `for (i, tile) in pw.tiles()...`-Block ersetzen durch:

```rust
                // Aktiver Trace auf GENAU diesem Wort? Dann Fortschritt + Next-Tile.
                let active = trace.filter(|(tid, _)| *tid == e.id).map(|(_, p)| p);
                let next_style = Style::default()
                    .fg(theme::HIGHLIGHT_FG)
                    .bg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD);
                let traced_style = Style::default()
                    .fg(theme::HIGHLIGHT_FG)
                    .bg(theme::HIGHLIGHT_BG)
                    .add_modifier(Modifier::BOLD);
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
                    // Keystroke k landet auf keystroke_tile(k); bei reversed ist
                    // das Tile-Index n-1-k. „logischer Fortschritt" dieses Tiles:
                    let logical = if pw.reversed {
                        letters.len() - 1 - i
                    } else {
                        i
                    };
                    let style = match active {
                        Some(p) if logical < p => traced_style,
                        Some(p) if logical == p => next_style,
                        _ => shimmer_style(t, i),
                    };
                    grid[ry as usize][rx as usize] = Some((ch, style));
                }
```

Im Cursor-Loop den eigenen Pfeil während aktivem Trace unterdrücken. Die finale `if`-Bedingung ersetzen:

```rust
        // Eigener Cursor-Pfeil wird während eines aktiven Trace unterdrückt —
        // die Next-Tile-Hervorhebung (oben) steht an seiner Stelle (Pickup C).
        let suppress_self = player.is_self && trace.is_some();
        if (player.is_self || !player.is_dead) && !suppress_self {
            grid[ry as usize][rx as usize] = Some((arrow_ch, style));
        }
```

- [ ] **Step 5: Build + Tests laufen lassen → grün**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo build 2>&1 | tail -5 && cargo test -p prfh --lib render:: 2>&1 | tail -20
```
Erwartet: build ok (alle `draw_world`-Aufrufe nutzen die neue Signatur — es gibt nur den einen in `draw`); alle render-Tests grün inkl. neuem Smoke-Test.

- [ ] **Step 6: Commit**

```bash
git add src/render/mod.rs
git commit -m "feat(#43): Trace-Feedback verdrahtet — Fortschritt-Highlight, Next-Tile, Cursor-Suppression (C)"
```

---

### Task 6: Abschluss — Voller grüner Lauf + Review

**Files:** keine (Verifikation).

- [ ] **Step 1: Voller Test + clippy, warnungsfrei**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo build 2>&1 | tail -5
cargo test 2>&1 | tail -20
cargo clippy --all-targets 2>&1 | tail -20
```
Erwartet: build/test grün, clippy **ohne** Warnungen. Bei Warnungen: beheben (kein `#[allow]`), erneut laufen.

- [ ] **Step 2: code-reviewer-Subagent über das Diff**

Diff gegen `main` an den `feature-dev:code-reviewer`-Subagent geben (Fokus: Snap-Borrow-Korrektheit in `app.rs`, Render-Reihenfolge, reversed-Pfad im Trace-Styling). Befunde adressieren.

- [ ] **Step 3: PR aktualisieren**

```bash
git push
gh pr view 49 --json url -q .url
```
PR #49 ist bereits ready; ggf. Beschreibung um die A/B/C-Verfeinerung ergänzen.

---

## Self-Review-Notiz

- **Spec-Coverage:** A → Task 1+2; B → Task 3; C → Task 4 (Explore) + Task 5 (Wire); Leitplanken (warnungsfrei, FSM unangetastet) → Global Constraints + Task 6.
- **Reversed-Konsistenz:** Task 5 nutzt `logical = n-1-i` bei reversed, damit Fortschritt/Next-Tile der Tipp-Reihenfolge folgen (passt zu `keystroke_tile`-Mapping in powerup.rs).
- **Borrow:** Task 2 entnimmt `target` (owned) via `find_map`, Arena-Borrow endet vor `e.cursor =`.
- **Signatur-Drift:** `draw_world` wird nur in `draw` aufgerufen (ein Call-Site) — Signaturänderung in Task 5 ist lokal.
