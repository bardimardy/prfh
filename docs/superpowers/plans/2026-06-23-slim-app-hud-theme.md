# Schlanke App/HUD-State + `src/theme.rs` Palette — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eine zentrale Farb-Palette `src/theme.rs` einführen und `App`/HUD von Altbeständen befreien, damit die render-berührenden Folge-Issues (A, C) keine divergierenden Farben hardcoden.

**Architecture:** `theme.rs` wird ein neues `pub mod` mit `Color`-Konstanten (Single Source of Truth). Die Topbar (`draw_hud`) wird auf `dir` + `combo` reduziert und nutzt die Palette. `last_event` rendert nur noch im Debug-Overlay; die verbose Trigger-Hilfe und das Titel-Banner verschwinden. Das `day`-Feld auf `App` wird ersatzlos entfernt.

**Tech Stack:** Rust 2021, Ratatui 0.30, Crossterm 0.29. Render-Tests über `ratatui::backend::TestBackend` (kein zusätzliches Crate nötig).

## Global Constraints

- `cargo build` und `cargo test` müssen grün **und warnungsfrei** sein (kein `#[allow]` zum Verstecken).
- `prfh` ist ein Library-Crate (`src/lib.rs`) plus Binary (`src/main.rs`). `pub` Konstanten in `theme.rs` sind Teil der Crate-API → sie lösen **keine** `dead_code`-Warnung aus, auch wenn 0b sie noch nicht alle verwendet.
- Palette-Werte **verbatim** aus Spec §2: `ACCENT #5AA9FF`, `HIGHLIGHT_BG #FF49A0`, `HIGHLIGHT_FG #141012`, `PANEL_BG #26262B`, `PICKUP_BASE #FF4080`, `TEXT #C8CCD4`, `TEXT_DIM #6A6E78`, `DANGER #E54B4B`.
- Scope-Grenze: **nur** App/HUD-Slimming + `theme.rs`. Die offenen Detail-Punkte aus Spec §13 (Cast-Umschalttaste, Inventar-Hotkey, Ghost-Styling) gehören zu B/C und werden hier **nicht** fixiert. World-Rendering-Farben (`draw_world`) bleiben unangetastet (Kollisionsvermeidung mit Issue C).
- cargo läuft via `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"`.

## File Structure

- **Create** `src/theme.rs` — Dark-Mode-Palette, eine Quelle der Wahrheit für alle Farben.
- **Modify** `src/lib.rs` — `pub mod theme;` registrieren.
- **Modify** `src/app.rs` — `day`-Feld + dessen Initialisierung entfernen.
- **Modify** `src/render/mod.rs` — `draw_hud` schlank (dir + combo, Palette); `draw_bottom` → schmaler Footer (nur Quit-Hinweis); `last_event` ins Debug-Overlay; ungenutzte Imports raus.

---

### Task 1: `src/theme.rs` Palette

**Files:**
- Create: `src/theme.rs`
- Modify: `src/lib.rs:1-3`
- Test: `src/theme.rs` (`#[cfg(test)]`-Modul)

**Interfaces:**
- Consumes: `ratatui::style::Color`.
- Produces: `pub const ACCENT/HIGHLIGHT_BG/HIGHLIGHT_FG/PANEL_BG/PICKUP_BASE/TEXT/TEXT_DIM/DANGER: Color` im Modul `crate::theme`.

- [ ] **Step 1: Write the failing test**

Lege `src/theme.rs` zunächst nur mit dem Test an (die Konstanten existieren noch nicht → Compile-Fehler = roter Test):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn palette_matches_spec_hex() {
        // Werte aus Spec §2 — Single source of truth, gegen Tippfehler abgesichert.
        assert_eq!(ACCENT, Color::Rgb(0x5A, 0xA9, 0xFF));
        assert_eq!(HIGHLIGHT_BG, Color::Rgb(0xFF, 0x49, 0xA0));
        assert_eq!(HIGHLIGHT_FG, Color::Rgb(0x14, 0x10, 0x12));
        assert_eq!(PANEL_BG, Color::Rgb(0x26, 0x26, 0x2B));
        assert_eq!(PICKUP_BASE, Color::Rgb(0xFF, 0x40, 0x80));
        assert_eq!(TEXT, Color::Rgb(0xC8, 0xCC, 0xD4));
        assert_eq!(TEXT_DIM, Color::Rgb(0x6A, 0x6E, 0x78));
        assert_eq!(DANGER, Color::Rgb(0xE5, 0x4B, 0x4B));
    }
}
```

Registriere das Modul in `src/lib.rs` (sonst wird die Datei nicht kompiliert):

```rust
pub mod app;
pub mod game;
pub mod render;
pub mod theme;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib theme`
Expected: FAIL — Compile-Fehler `cannot find value ACCENT in this scope` (Konstanten fehlen noch).

- [ ] **Step 3: Write minimal implementation**

Füge oben in `src/theme.rs` (vor dem Test-Modul) die Palette ein:

```rust
//! Dark-Mode-Palette für `prfh` — Single Source of Truth für alle Farben.
//!
//! Render-berührende Issues sollen Farben hier referenzieren statt eigene
//! Hex-Werte zu hardcoden. Werte eingefroren in Spec §2.

use ratatui::style::Color;

/// Blau — HUD/Overlay-Text, Überschriften, Akzente.
pub const ACCENT: Color = Color::Rgb(0x5A, 0xA9, 0xFF);
/// Pink — Highlighting (getippter Prefix).
pub const HIGHLIGHT_BG: Color = Color::Rgb(0xFF, 0x49, 0xA0);
/// Dunkler Text auf dem Pink-Kasten.
pub const HIGHLIGHT_FG: Color = Color::Rgb(0x14, 0x10, 0x12);
/// Panel-/Overlay-Füllung.
pub const PANEL_BG: Color = Color::Rgb(0x26, 0x26, 0x2B);
/// Gesättigte Basis für den Pickup-Regenbogen.
pub const PICKUP_BASE: Color = Color::Rgb(0xFF, 0x40, 0x80);
/// Lesbarer Body-Text.
pub const TEXT: Color = Color::Rgb(0xC8, 0xCC, 0xD4);
/// Gedämpfter Text, Borders.
pub const TEXT_DIM: Color = Color::Rgb(0x6A, 0x6E, 0x78);
/// Warn-/Fehlerakzent.
pub const DANGER: Color = Color::Rgb(0xE5, 0x4B, 0x4B);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib theme`
Expected: PASS (`palette_matches_spec_hex ... ok`).

- [ ] **Step 5: Commit**

```bash
git add src/theme.rs src/lib.rs
git commit -m "feat(theme): zentrale Dark-Mode-Palette (Spec §2)"
```

---

### Task 2: Schlanke Topbar (`dir` + `combo`) + `day`-Feld entfernen

**Files:**
- Modify: `src/render/mod.rs:80-130` (`draw_hud`)
- Modify: `src/app.rs:6,21` (Feld `day` + Init)
- Test: `src/render/mod.rs` (`#[cfg(test)]`-Modul)

**Interfaces:**
- Consumes: `crate::theme::{ACCENT, TEXT, TEXT_DIM}`, `app.writing.direction`, `app.writing.combo`.
- Produces: `draw_hud` rendert nur noch Richtungs-Pfeil + `combo xN`. `App` hat kein `day`-Feld mehr.

- [ ] **Step 1: Write the failing test**

Füge am Ende von `src/render/mod.rs` an:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_string(app: &App) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn topbar_shows_only_dir_and_combo() {
        let app = App::new();
        let out = render_to_string(&app);
        // combo bleibt sichtbar ...
        assert!(out.contains("combo"), "combo fehlt in der Topbar");
        // ... aber der ganze Altbestand ist raus:
        assert!(!out.contains("PULL REQUEST"), "Titel-Banner noch da");
        assert!(!out.contains("word:"), "word-Anzeige noch in der Topbar");
        assert!(!out.contains("doubt"), "doubt noch in der Topbar");
        assert!(!out.contains("day"), "day noch in der Topbar");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib render::tests::topbar_shows_only_dir_and_combo`
Expected: FAIL — Assertion `Titel-Banner noch da` (die alte HUD rendert noch „PULL REQUEST FROM HELL").

- [ ] **Step 3: Write minimal implementation**

Ersetze die komplette Funktion `draw_hud` in `src/render/mod.rs` durch:

```rust
fn draw_hud(f: &mut Frame, area: Rect, app: &App) {
    let arrow = match app.writing.direction {
        Direction::Up => "↑",
        Direction::Down => "↓",
        Direction::Left => "←",
        Direction::Right => "→",
    };

    let hud = Paragraph::new(Line::from(vec![
        Span::styled("dir ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("{arrow} "),
            Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("combo ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("x{}", app.writing.combo),
            Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(hud, area);
}
```

Passe die `use`-Zeilen oben in `src/render/mod.rs` an: entferne `buffer_ends_with_trigger` aus dem `writing`-Import (wird nicht mehr gebraucht) und ergänze den theme-Import:

```rust
use crate::app::App;
use crate::game::writing::Direction;
use crate::theme;
```

Entferne in `src/app.rs` das `day`-Feld aus dem Struct (Zeile 6) und aus `App::new()` (Zeile 21):

```rust
pub struct App {
    pub should_quit: bool,
    pub writing: WritingEngine,
    pub last_event: String,
    /// Sticky trigger banner — set when a trigger fires, decremented per tick.
    pub trigger_banner: Option<String>,
    pub trigger_banner_ticks: u32,
    pub debug: bool,
    pub debug_lines: Vec<String>,
}
```

```rust
    pub fn new() -> Self {
        Self {
            should_quit: false,
            writing: WritingEngine::new((0, 0)),
            last_event: String::from("type to write yourself a path"),
            trigger_banner: None,
            trigger_banner_ticks: 0,
            debug: false,
            debug_lines: Vec::new(),
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib render::tests::topbar_shows_only_dir_and_combo`
Expected: PASS.

- [ ] **Step 5: Verify warning-free build**

Run: `cargo build 2>&1 | grep -i warning; echo "exit-grep:$?"`
Expected: keine Warnung (grep findet nichts → `exit-grep:1`). Insbesondere kein „unused import: `buffer_ends_with_trigger`".

- [ ] **Step 6: Commit**

```bash
git add src/render/mod.rs src/app.rs
git commit -m "refactor(hud): schlanke Topbar (dir + combo), day-Feld raus"
```

---

### Task 3: `last_event` → Debug-Overlay, verbose Trigger-Hilfe + Titel-Banner raus

**Files:**
- Modify: `src/render/mod.rs:32-64` (`draw_debug_overlay`), `:211-229` (`draw_bottom`), Layout in `draw`
- Test: `src/render/mod.rs` (`#[cfg(test)]`-Modul, neue Tests)

**Interfaces:**
- Consumes: `app.last_event`, `app.debug`.
- Produces: `draw_bottom` rendert nur noch einen Quit-Hinweis; `last_event` erscheint ausschließlich im Debug-Overlay (`PRFH_DEBUG`).

- [ ] **Step 1: Write the failing test**

Ergänze im `#[cfg(test)] mod tests` von `src/render/mod.rs`:

```rust
    #[test]
    fn last_event_only_in_debug_overlay() {
        // Sichtbarer, eindeutiger Marker als last_event.
        let mut app = App::new();
        app.last_event = "ZZMARKERZZ".into();

        // Ohne Debug: Marker darf nirgends auftauchen.
        app.debug = false;
        assert!(
            !render_to_string(&app).contains("ZZMARKERZZ"),
            "last_event leakt ohne PRFH_DEBUG"
        );

        // Mit Debug: Marker erscheint im Overlay.
        app.debug = true;
        assert!(
            render_to_string(&app).contains("ZZMARKERZZ"),
            "last_event fehlt im Debug-Overlay"
        );
    }

    #[test]
    fn no_verbose_trigger_help() {
        let app = App::new();
        let out = render_to_string(&app);
        assert!(
            !out.contains("up down left right"),
            "verbose Trigger-Hilfe noch da"
        );
        assert!(out.contains("Esc"), "Quit-Hinweis fehlt");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib render::tests::last_event_only_in_debug_overlay render::tests::no_verbose_trigger_help`
Expected: FAIL — `last_event leakt ohne PRFH_DEBUG` (draw_bottom rendert `last_event`) und `verbose Trigger-Hilfe noch da`.

- [ ] **Step 3: Write minimal implementation**

Ersetze `draw_bottom` in `src/render/mod.rs` durch einen schlanken Footer (kein Border, nur Quit-Hinweis, kein `last_event`, keine Trigger-Liste):

```rust
fn draw_bottom(f: &mut Frame, area: Rect, _app: &App) {
    let p = Paragraph::new(Line::from(vec![
        Span::styled("[Esc]", Style::default().fg(theme::ACCENT)),
        Span::styled(" quit", Style::default().fg(theme::TEXT_DIM)),
    ]));
    f.render_widget(p, area);
}
```

Da der Footer keinen Border mehr hat, schrumpft sein Layout-Chunk von `Length(5)` auf `Length(1)`. Passe die `constraints` in `draw` an:

```rust
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(f.area());
```

Hänge `last_event` als erste Zeile in das Debug-Overlay. In `draw_debug_overlay`, direkt **nach** der bestehenden `dir=… word=… cur=…`-Zeile (vor der `for l in &app.debug_lines`-Schleife), einfügen:

```rust
    lines.push(Line::from(Span::styled(
        format!("last: {}", app.last_event),
        Style::default().fg(theme::TEXT_DIM),
    )));
```

Da das Overlay nun eine Zeile mehr hat, erhöhe die Höhenberechnung um 1, damit die Zeile nicht abgeschnitten wird. Ändere in `draw_debug_overlay`:

```rust
    let h = (app.debug_lines.len() as u16 + 5).min(area.height);
```

Falls `Wrap` durch das Entfernen aus `draw_bottom` ungenutzt wird: aus dem `widgets`-Import in `src/render/mod.rs` entfernen.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib`
Expected: PASS (alle Render-Tests grün).

- [ ] **Step 5: Verify warning-free build**

Run: `cargo build 2>&1 | grep -i warning; echo "exit-grep:$?"`
Expected: keine Warnung (`exit-grep:1`). Insbesondere kein „unused import: `Wrap`".

- [ ] **Step 6: Commit**

```bash
git add src/render/mod.rs
git commit -m "refactor(hud): last_event ins Debug-Overlay, Trigger-Hilfe + Footer-Border raus"
```

---

### Task 4: Voll-Suite + Plan abschließen

- [ ] **Step 1: Volle Test-Suite**

Run: `cargo test`
Expected: PASS — alle Tests grün (Lib + ggf. Integration).

- [ ] **Step 2: Warnungs-Gate**

Run: `cargo build 2>&1 | grep -i warning; echo "exit-grep:$?"`
Expected: `exit-grep:1` (keine Warnung).

- [ ] **Step 3: Plan-Datei mit abgehakten Schritten committen**

```bash
git add docs/superpowers/plans/2026-06-23-slim-app-hud-theme.md
git commit -m "docs: Plan für #28 (schlanke App/HUD + theme.rs)"
```

---

## Self-Review

**Spec-/AK-Abdeckung:**
- `src/theme.rs` mit Palette §2 → Task 1. ✅ (alle 8 Konstanten verbatim getestet)
- HUD-Clutter raus (`day`, `doubt`, Titel-Banner, verbose Trigger-Hilfe) → Task 2 (`day`/`doubt`/Titel/`word`) + Task 3 (Trigger-Hilfe). ✅
- `last_event` → Debug-Overlay → Task 3. ✅
- Topbar schlank (nur `dir` + `combo`) → Task 2. ✅
- `cargo build`/`cargo test` grün & warnungsfrei → Warnungs-Gates in Task 2/3/4. ✅
- Kleines `App`-Skelett für B/C/D → das geslimmte `App`-Struct bleibt das Fundament; Felder werden nur entfernt, nicht erweitert (B landet die neuen Felder). ✅

**Placeholder-Scan:** Keine TBD/TODO; jeder Code-Schritt zeigt vollständigen Code. ✅

**Typ-Konsistenz:** `theme::{ACCENT, TEXT, TEXT_DIM}` in Task 2/3 identisch zu den in Task 1 definierten Namen; `draw_hud`/`draw_bottom`/`draw_debug_overlay`-Signaturen unverändert (nur Körper getauscht). ✅

**Scope-Grenze §13:** Cast-Umschalttaste, Inventar-Hotkey, Ghost-Styling **nicht** angefasst — gehören zu B/C. ✅
