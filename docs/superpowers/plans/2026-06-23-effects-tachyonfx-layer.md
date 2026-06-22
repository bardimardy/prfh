# tachyonfx Effekt-Layer (#29 / Issue A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ein dünner, kollisionsfreier tachyonfx-Wrapper (`src/effects/`) mit benannten Effekt-Konstruktoren und ein `process_effects`-Render-Hook — die Bausteine, die C (#31) später live verdrahtet.

**Architecture:** `src/effects/mod.rs` kapselt die verifizierten tachyonfx-Bausteine hinter benannten Konstruktoren (`pickup()`, `activation()`). Die HARTE Non-Overshoot-Regel für `expand` wird im Modul gekapselt (privater Guard) und durch „bis zum Ende prozessiert ohne Panic"-Smoke-Tests abgesichert. `src/render/mod.rs` bekommt einen generischen `process_effects`-Hook (Public API, kein App-Feld), den die Smoke-Tests denselben Pfad fahren lassen wie der spätere Live-Call.

**Tech Stack:** Rust 2021, ratatui 0.30, crossterm 0.29, tachyonfx 0.25 (`tachyonfx::fx`, `tachyonfx::EffectManager`, `tachyonfx::{Motion, Interpolation}`, `tachyonfx::fx::ExpandDirection`).

## Global Constraints

- **Versionen (bereits im Repo, NICHT ändern):** `ratatui = "0.30"`, `crossterm = "0.29"`. tachyonfx 0.25 verlangt genau diese — passt.
- **Neue Dep:** `tachyonfx = "0.25"`.
- **HARTE REGEL (verifizierter Panic):** `expand`/`stretch` paniken bei Overshoot-Easings (`BackIn/Out/InOut`, `ElasticIn/Out/InOut`) — Subtraktions-Overflow in `stretch.rs`. Für `expand` NUR Non-Overshoot-Kurven (`CircOut`, `QuadOut`, `SineOut`, `CubicOut`). Im `effects`-Modul kapseln + per Smoke-Test (Effekt über die Timer-Dauer hinaus prozessieren) absichern.
- **Kollisionsvermeidung (#12):** A bleibt strikt in `Cargo.toml`, `src/effects/`, `src/render/mod.rs`. `app.rs` und der Game-Logik-Layer gehören B (#30) — NICHT anfassen. KEINE `App`-Felder anlegen. Der `EffectManager` lebt in dieser Phase NUR lokal in den Smoke-Tests; die Live-Verdrahtung macht C (#31).
- **Qualität:** `cargo build` + `cargo test` grün und **warnungsfrei**. Kein `#[allow]`, kein toter Code. Public API in einem lib-Crate löst keine `dead_code`-Warnung aus — daher dürfen die Hook/Konstruktoren `pub` und noch ohne Live-Aufruf sein.
- **API-Leitplanke:** Die Spec-Signaturen wurden gegen tachyonfx 0.25 (context7) verifiziert und stimmen material. Bei Compile-Abweichung im echten Crate: echte 0.25-Signatur nehmen, nicht dagegen ankämpfen.
- **cargo-PATH:** `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"`

---

## File Structure

- `Cargo.toml` — neue Dependency `tachyonfx = "0.25"`.
- `src/lib.rs` — `pub mod effects;` ergänzen.
- `src/effects/mod.rs` — **Neu.** Benannte Konstruktoren `pickup()`/`activation()`, privater Non-Overshoot-Guard `is_non_overshoot()` + `safe_expand()`, Smoke-Tests.
- `src/render/mod.rs` — generischer `process_effects`-Hook + Smoke-Test gegen Scratch-Buffer.

---

### Task 1: tachyonfx-Dependency aufnehmen

**Files:**
- Modify: `Cargo.toml` (`[dependencies]`)

**Interfaces:**
- Consumes: nichts.
- Produces: Crate `tachyonfx` 0.25 ist verfügbar (`tachyonfx::fx`, `tachyonfx::EffectManager`, `tachyonfx::{Motion, Interpolation}`, `tachyonfx::fx::ExpandDirection`).

- [ ] **Step 1: Dependency ergänzen**

In `Cargo.toml`, unter `[dependencies]`, nach der `crossterm`-Zeile einfügen:

```toml
tachyonfx = "0.25"
```

- [ ] **Step 2: Auflösen/Bauen verifizieren**

Run: `cargo build`
Expected: PASS — tachyonfx 0.25 löst gegen ratatui 0.30 / crossterm 0.29 auf, keine Versionskonflikte, keine Warnungen.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat(effects): tachyonfx 0.25 als Dependency (#29)"
```

---

### Task 2: effects-Modul + Non-Overshoot-Guard + pickup()

**Files:**
- Modify: `src/lib.rs:1-4` (`pub mod effects;` ergänzen)
- Create: `src/effects/mod.rs`
- Test: `src/effects/mod.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: tachyonfx 0.25 (Task 1).
- Produces:
  - `pub fn pickup() -> tachyonfx::Effect`
  - privat `fn is_non_overshoot(c: Interpolation) -> bool`
  - privat `fn safe_expand(dir: ExpandDirection, style: Style, ms: u32, curve: Interpolation) -> Effect`
  - Test-Helper-Pattern `run_to_end(effect)` (lokaler `EffectManager`, prozessiert über die Dauer hinaus).

- [ ] **Step 1: Modul registrieren**

In `src/lib.rs` nach `pub mod app;` einfügen:

```rust
pub mod effects;
```

- [ ] **Step 2: Failing Smoke-Test schreiben**

Create `src/effects/mod.rs` mit NUR dem Test (Implementierung folgt):

```rust
//! Dünner tachyonfx-Wrapper: benannte Effekt-Konstruktoren.
//!
//! tachyonfx ist visuell/zeitgetrieben — keine echten Unit-Tests, sondern
//! „konstruiert + bis zum Ende prozessiert ohne Panic"-Smoke-Tests gegen einen
//! Scratch-Buffer. `main` wird NICHT auf visuelle Korrektheit gegated.

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::time::Duration;
    use tachyonfx::EffectManager;

    /// Prozessiert einen Effekt über denselben Pfad wie der Render-Hook weit
    /// über seine Timer-Dauer hinaus. Paniken (z.B. expand-Overflow) schlagen
    /// hier zu — genau das ist der Sinn.
    fn run_to_end(effect: tachyonfx::Effect) {
        let mut mgr: EffectManager<()> = EffectManager::default();
        mgr.add_effect(effect);
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 12));
        let area = buf.area;
        let step = Duration::from_millis(50);
        // 200 * 50ms = 10s — länger als jeder Konstruktor-Effekt.
        for _ in 0..200 {
            mgr.process_effects(step.into(), &mut buf, area);
        }
    }

    #[test]
    fn pickup_runs_to_end_without_panic() {
        run_to_end(pickup());
    }
}
```

- [ ] **Step 3: Test ausführen (muss fehlschlagen)**

Run: `cargo test -p prfh effects:: 2>&1 | tail -20`
Expected: FAIL — Compile-Fehler `cannot find function 'pickup' in this scope`.

- [ ] **Step 4: Minimal-Implementierung (Imports + Guard + pickup)**

Oben in `src/effects/mod.rs` (vor dem `#[cfg(test)]`-Block) einfügen:

```rust
use ratatui::style::{Color, Style};
use tachyonfx::fx::ExpandDirection;
use tachyonfx::{fx, Effect, Interpolation, Motion};

/// `expand`/`stretch` paniken bei Overshoot-Easings (Back*/Elastic*) durch
/// Subtraktions-Overflow. Dieser Guard ist die zentrale Leitplanke der
/// Non-Overshoot-Regel — er hält `safe_expand` auf sichere Kurven.
fn is_non_overshoot(c: Interpolation) -> bool {
    !matches!(
        c,
        Interpolation::BackIn
            | Interpolation::BackOut
            | Interpolation::BackInOut
            | Interpolation::ElasticIn
            | Interpolation::ElasticOut
            | Interpolation::ElasticInOut
    )
}

/// Einziger erlaubter Weg, im effects-Modul `expand` zu bauen. Der
/// `debug_assert!` verhindert, dass je eine Overshoot-Kurve durchrutscht.
fn safe_expand(dir: ExpandDirection, style: Style, ms: u32, curve: Interpolation) -> Effect {
    debug_assert!(
        is_non_overshoot(curve),
        "expand panik-Regel verletzt: Overshoot-Kurve {curve:?} ist verboten"
    );
    fx::expand(dir, style, (ms, curve))
}

/// Pickup eines Powerup-Worts: kurzer, freundlicher Farb-Puls + Slide-In.
/// Verwendet nur verifizierte Bausteine (`parallel`, `hsl_shift`, `slide_in`).
pub fn pickup() -> Effect {
    fx::parallel(&[
        fx::hsl_shift(Some([90.0, 25.0, 15.0]), None, (600, Interpolation::SineOut)),
        fx::slide_in(Motion::UpToDown, 6, 0, Color::Black, 600),
    ])
}
```

- [ ] **Step 5: Test ausführen (muss bestehen)**

Run: `cargo test -p prfh effects:: 2>&1 | tail -20`
Expected: PASS — `pickup_runs_to_end_without_panic ... ok`.

- [ ] **Step 6: Warnungsfrei prüfen**

Run: `cargo build 2>&1 | grep -i warning; echo "exit:$?"`
Expected: keine `warning:`-Zeile. (`safe_expand`/`is_non_overshoot` sind in Task 3 genutzt; falls dieser Schritt isoliert läuft und `dead_code` meldet, Task 3 unmittelbar anschließen — sie sind ein zusammenhängender Deliverable.)

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs src/effects/mod.rs
git commit -m "feat(effects): effects-Modul + Non-Overshoot-Guard + pickup() (#29)"
```

---

### Task 3: activation() — expand mit Non-Overshoot-Kurve (zentraler Panik-Test)

**Files:**
- Modify: `src/effects/mod.rs`

**Interfaces:**
- Consumes: `safe_expand`, `is_non_overshoot` (Task 2).
- Produces: `pub fn activation() -> Effect`.

- [ ] **Step 1: Failing Tests schreiben**

In den `#[cfg(test)] mod tests`-Block von `src/effects/mod.rs` ergänzen:

```rust
    #[test]
    fn activation_runs_to_end_without_panic() {
        run_to_end(activation());
    }

    #[test]
    fn guard_rejects_overshoot_curves() {
        assert!(!is_non_overshoot(Interpolation::BackOut));
        assert!(!is_non_overshoot(Interpolation::ElasticOut));
        assert!(is_non_overshoot(Interpolation::CircOut));
        assert!(is_non_overshoot(Interpolation::QuadOut));
        assert!(is_non_overshoot(Interpolation::SineOut));
        assert!(is_non_overshoot(Interpolation::CubicOut));
    }
```

- [ ] **Step 2: Tests ausführen (müssen fehlschlagen)**

Run: `cargo test -p prfh effects:: 2>&1 | tail -20`
Expected: FAIL — Compile-Fehler `cannot find function 'activation' in this scope`.

- [ ] **Step 3: activation() implementieren**

Nach `pickup()` in `src/effects/mod.rs` einfügen:

```rust
/// Aktivierung eines Powerups: horizontale Welle (`expand`) gefolgt von einem
/// Farb-Shift. `expand` läuft bewusst über `safe_expand` mit `CircOut` — einer
/// Non-Overshoot-Kurve — und darf daher nicht paniken.
pub fn activation() -> Effect {
    fx::sequence(&[
        safe_expand(
            ExpandDirection::Horizontal,
            Style::default().bg(Color::Indexed(54)),
            500,
            Interpolation::CircOut,
        ),
        fx::hsl_shift(Some([200.0, 20.0, 10.0]), None, (400, Interpolation::QuadOut)),
    ])
}
```

- [ ] **Step 4: Tests ausführen (müssen bestehen)**

Run: `cargo test -p prfh effects:: 2>&1 | tail -20`
Expected: PASS — `activation_runs_to_end_without_panic ... ok`, `guard_rejects_overshoot_curves ... ok`, `pickup_runs_to_end_without_panic ... ok`.

- [ ] **Step 5: Commit**

```bash
git add src/effects/mod.rs
git commit -m "feat(effects): activation() via safe_expand (Non-Overshoot) + Guard-Test (#29)"
```

---

### Task 4: process_effects-Render-Hook + Smoke-Test

**Files:**
- Modify: `src/render/mod.rs` (oberer Bereich: Hook-Funktion; `#[cfg(test)] mod tests`: Smoke-Test)

**Interfaces:**
- Consumes: `tachyonfx::EffectManager`, `effects::pickup` (Task 2).
- Produces: `pub fn process_effects<K>(manager: &mut EffectManager<K>, elapsed: Duration, buf: &mut Buffer, area: Rect)`.

- [ ] **Step 1: Failing Smoke-Test schreiben**

In `src/render/mod.rs`, in `#[cfg(test)] mod tests`, ergänzen (Imports stehen ggf. schon — sonst lokal im Test importieren):

```rust
    #[test]
    fn process_effects_hook_drives_manager_without_panic() {
        use crate::effects;
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use std::time::Duration;
        use tachyonfx::EffectManager;

        let mut mgr: EffectManager<()> = EffectManager::default();
        mgr.add_effect(effects::pickup());

        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 12));
        let area = buf.area;
        for _ in 0..40 {
            process_effects(&mut mgr, Duration::from_millis(50), &mut buf, area);
        }
    }
```

- [ ] **Step 2: Test ausführen (muss fehlschlagen)**

Run: `cargo test -p prfh render::tests::process_effects_hook 2>&1 | tail -20`
Expected: FAIL — Compile-Fehler `cannot find function 'process_effects' in this scope`.

- [ ] **Step 3: Hook implementieren**

In `src/render/mod.rs` ganz oben die nötigen Use-Statements ergänzen (sofern nicht vorhanden) und die Hook-Funktion nach den bestehenden `use`-Zeilen / vor `pub fn draw` einfügen:

```rust
use ratatui::buffer::Buffer;
use std::time::Duration;
use tachyonfx::EffectManager;

/// Post-Render-Hook: treibt einen `EffectManager` gegen den Frame-Buffer.
/// Generisch über den Key-Typ `K`, damit der spätere Live-Call (C, #31) die
/// Key-Strategie frei wählt — diese Phase legt KEINE `App`-Felder an.
/// In `draw` wird das später als `process_effects(mgr, elapsed, f.buffer_mut(), area)`
/// aufgerufen; hier nur die wiederverwendbare, testbare Funktion.
pub fn process_effects<K>(
    manager: &mut EffectManager<K>,
    elapsed: Duration,
    buf: &mut Buffer,
    area: Rect,
) {
    manager.process_effects(elapsed.into(), buf, area);
}
```

Hinweis: `Rect` ist bereits über den bestehenden `use ratatui::layout::{... Rect}` importiert — keine Doppel-Importe anlegen (sonst Warnung). Falls `Rect` dort fehlt, in der vorhandenen `layout`-Use-Zeile ergänzen statt neu importieren.

- [ ] **Step 4: Test ausführen (muss bestehen)**

Run: `cargo test -p prfh render::tests::process_effects_hook 2>&1 | tail -20`
Expected: PASS — `process_effects_hook_drives_manager_without_panic ... ok`.

- [ ] **Step 5: Volle Suite + warnungsfrei**

Run: `cargo test 2>&1 | tail -15 && cargo build 2>&1 | grep -i warning; echo "warn-grep-exit:$?"`
Expected: alle Tests PASS; keine `warning:`-Zeile (`warn-grep-exit:1` = grep fand nichts).

- [ ] **Step 6: Commit**

```bash
git add src/render/mod.rs
git commit -m "feat(render): process_effects-Hook + Smoke-Test gegen Scratch-Buffer (#29)"
```

---

## Self-Review

**Spec-Coverage (§3, §4, §11, §12):**
- §3 tachyonfx 0.25 als Dep → Task 1. ✓
- §3 verifizierte Bausteine (`slide_in`, `hsl_shift`, `expand`, `parallel`, `sequence`) → Tasks 2/3. ✓
- §3 Non-Overshoot-Regel gekapselt + Smoke-Test → Task 2 (`safe_expand`/`is_non_overshoot`) + Task 3 (`activation_runs_to_end`, `guard_rejects_overshoot_curves`). ✓
- §3 `process_effects`-Hook im Render → Task 4. ✓
- §4 Modul-Schnitt `src/effects/` benannte Konstruktoren → `pickup()`, `activation()`. ✓
- §11 keine echten Unit-Tests, „konstruiert + bis zum Ende prozessiert ohne Panic"-Smoke-Tests gegen Scratch-Buffer, main nicht visuell gegated → `run_to_end`-Pattern. ✓
- §12 Kollision: nur `Cargo.toml`/`effects`/`render`, KEINE `App`-Felder, Manager nur lokal im Test → eingehalten; Hook ist generisch über `K`, Live-Wiring bleibt C. ✓

**Placeholder-Scan:** Kein TBD/TODO; jeder Code-Schritt zeigt vollständigen Code. ✓

**Typ-Konsistenz:** `pickup() -> Effect`, `activation() -> Effect`, `safe_expand(ExpandDirection, Style, u32, Interpolation) -> Effect`, `is_non_overshoot(Interpolation) -> bool`, `process_effects::<K>(&mut EffectManager<K>, Duration, &mut Buffer, Rect)` — über alle Tasks konsistent. `EffectManager<()>` in Tests, generisches `K` im Hook. ✓

**API-Risiko:** Signaturen gegen tachyonfx 0.25 (context7) verifiziert. Restrisiko nur bei `EffectManager::add_effect`/`process_effects`-Arität und `Effect`/`Duration::into()` — wird in Step „Test muss fehlschlagen" durch Compile-Feedback sofort sichtbar; dann echte 0.25-Signatur übernehmen (Global Constraint „API-Leitplanke").
