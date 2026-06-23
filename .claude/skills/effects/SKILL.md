---
name: effects
description: Wissen über den tachyonfx-Effekt-Layer in prfh (src/effects/ + Render-Hook). Use IMMER bevor du Effekte/Animationen anfasst oder tachyonfx-APIs aufrufst — verifizierte 0.25-API, die HARTE Non-Overshoot-Panik-Regel für expand, das Smoke-Test-Muster und der Kollisions-Schnitt. Triggert auf tachyonfx, Effekt, Animation, expand, slide_in, hsl_shift, EffectManager, process_effects.
---

# tachyonfx-Effekt-Layer (prfh)

Verbindliches Wissen für jede Arbeit an `src/effects/` oder am Render-Effekt-Hook.
Eingeführt mit #29 (PR #36). Quelle: `docs/superpowers/specs/2026-06-22-powerup-inventory-effects-design.md` §3/§4/§11/§12 + gegen tachyonfx 0.25 verifiziert.

## ⚠️ HARTE REGEL: expand/stretch + Overshoot-Easing = Panik

`fx::expand` / `fx::stretch` **paniken** (Subtraktions-Overflow in tachyonfx
`stretch.rs`) sobald sie mit einer **Overshoot-Kurve** laufen: `BackIn/Out/InOut`,
`ElasticIn/Out/InOut`. Die Panik schlägt erst **am Ende** der Timer-Dauer zu, nicht beim
Konstruieren — ein naiver „läuft kurz"-Test verfehlt sie.

**Regel:** Für `expand`/`stretch` NUR Non-Overshoot-Kurven: `CircOut`, `QuadOut`,
`SineOut`, `CubicOut`. Im effects-Modul ist das über `safe_expand()` + den privaten
Guard `is_non_overshoot()` gekapselt — `expand` NIE direkt aufrufen, immer `safe_expand`.

## Testbarkeit: „bis zum Ende prozessiert, ohne Panik"-Smoke-Test

tachyonfx ist visuell/zeitgetrieben → **keine echten Unit-Tests**. Jeder neue
Effekt-Konstruktor bekommt einen Smoke-Test, der ihn über einen lokalen
`EffectManager` **weit über seine Timer-Dauer hinaus** gegen einen Scratch-`Buffer`
prozessiert und beweist, dass nichts panikt (siehe `run_to_end()` in
`src/effects/mod.rs`). Das „über das Ende hinaus" ist Pflicht — sonst entgeht die
expand-Panik. `main` wird **nicht** auf visuelle Korrektheit gegated; dieser Smoke-Test
IST das Gate.

## Verifizierte tachyonfx-0.25-API (gegen das Crate geprüft)

- Items liegen unter `tachyonfx::fx::*`; Enums: `tachyonfx::{Motion, Interpolation}`,
  `tachyonfx::fx::ExpandDirection` (`Horizontal | Vertical`).
- `fx::slide_in(Motion, gradient_len: u16, randomness: u16, Color, timer)`
- `fx::hsl_shift(Some([h,s,l]), hsl_bg, timer)` — **panikt, wenn fg UND bg `None`** sind.
- `fx::expand(ExpandDirection, Style, timer)` — nur über `safe_expand` (s. o.).
- `fx::parallel(&[Effect, …])`, `fx::sequence(&[Effect, …])`.
- **Timer:** `u32` → `EffectTimer` mit `Linear`; `(u32, Interpolation)` → mit Kurve.
- **EffectManager:** `EffectManager::<K>::default()`, `.add_effect(effect)`,
  `.process_effects(elapsed.into(), &mut Buffer, area)`. `elapsed` ist
  `std::time::Duration` → tachyonfx-Duration via `.into()`. **Key-Bound:**
  `K: Clone + Debug + Ord`.

## Render-Hook & Kollisions-Schnitt (§12)

- Der Hook lebt in `src/render/mod.rs`:
  `process_effects<K: Clone + Debug + Ord>(&mut EffectManager<K>, Duration, &mut Buffer, Rect)`.
  Generisch über `K`, damit der Live-Call die Key-Strategie frei wählt.
- Effekt-Arbeit (A) bleibt strikt in `Cargo.toml`, `src/effects/`, `src/render/mod.rs`.
  **KEINE `App`-Felder** anlegen — das kollidiert mit dem Game-Logik-Layer (B).
- Der `EffectManager` lebt vorerst nur lokal in Tests. Die **Live-Verdrahtung** der
  Pickup-/Wellen-Animationen (Manager als Frame-übergreifender State, `elapsed`-Messung,
  `process_effects(mgr, elapsed, f.buffer_mut(), area)` in `draw`) gehört C (#31).

## Was NICHT in tachyonfx gehört: der scrollende Trail (Learning #37)

Der **Trail-Fade** (kontinuierliche Transparenz älterer Zeichen + smoothes
längenbasiertes Entfernen) ist **bewusst kein tachyonfx-Effekt**. tachyonfx-
Effekte sind **screen-zell-/Rect-gebunden** (auch mit `CellFilter` nur über das,
was *gerade* an einer Buffer-Position steht) und kennen **keine logische
Zeichen-Identität**. Der Trail wird aber **cursor-zentriert gerendert und scrollt
jeden Frame** (Welt→Screen-Remap) — ein zeitbasierter Zell-Effekt würde über
logisch andere Zeichen **schmieren**.

**Regel:** Der Trail-Fade ist **Modell-Mathematik zur Render-Zeit** —
`trail_brightness(from_tail)` + `apply_trail_fade(&mut Vec<Tile>)` in
`src/game/writing.rs`, geteilt von `WritingEngine::tick_visuals` (Single/Host) und
`WorldView::tick_visuals` (Client). Brightness/Removal sind eine **reine Funktion
der Position vom Kopf**, **lokal auf jedem Knoten** berechnet (nie idle-gekoppelt,
nie über Netzwerk gesynct) — so faden Single und MP identisch ohne neue Messages.
Ein idle-/age-gesyncter Ansatz (MP-Merge #26) hatte genau das gebrochen.

tachyonfx bleibt für **diskrete One-Shot-Effekte** (Pickup, Welle, Glow): die
laufen <~0.5 s, viel schneller als relevantes Scrollen → Smear vernachlässigbar.
Brauchst du doch einen trail-artigen Effekt im Effekt-Graphen, ist `effect_fn`
(Shader-Closure mit eigenem State pro Zelle) die einzige scroll-immune Option —
für einen reinen Helligkeits-Gradient aber Overkill gegenüber dem Render-Lerp.

## Benannte Konstruktoren

Effekte werden als benannte, fachliche Konstruktoren gekapselt (`pickup()`,
`activation()`, …) — nie roh in der Render-/Game-Schicht zusammengebaut. Neue Effekte
dort ergänzen, mit Smoke-Test, und (falls `expand`/`stretch`) zwingend über `safe_expand`.
