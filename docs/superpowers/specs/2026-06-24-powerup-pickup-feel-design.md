# Design: Powerup-Pickup-Gefühl (W2-Verfeinerung)

- **Datum:** 2026-06-24
- **Status:** Entwurf (zur Freigabe)
- **Scope:** Drei Verfeinerungen am bestehenden W2-Pickup-Erlebnis (Trace-FSM +
  Powerup-Rendering), *innerhalb desselben Issues/PR* (#43 / `issue-43` / PR #49).
  Single-Flow. Multiplayer-Hooks (host-autoritatives Despawn) bleiben unangetastet.
- **Baut auf:** `2026-06-22-powerup-inventory-effects-design.md` (§5 Layout,
  §6 Trace-FSM) und `2026-06-23-world-base-engine-design.md` (§4 Arena,
  §7 Render-Transform).

Dieses Dokument wurde gegen die echte Codebase gegengeprüft: die drei Befunde
(A/B/C) sind im Code verifiziert (`writing.rs:409`, `render/mod.rs` Render-
Reihenfolge, `app.rs` on_char).

---

## 1. Ziel

Das Pickup-Erlebnis hat drei konkrete Reibungspunkte (am Code verifiziert):

- **A — Pickup ist zu schwer.** `Trace::observe` (Idle-Arm, `writing.rs:409`)
  verlangt *exaktes* Landen auf dem Eintritts-Tile: `pos == w.entry_tile()` UND
  korrekte Laufrichtung UND erster Buchstabe — alles gleichzeitig. Tile-genaues
  Treffen ist frustrierend.
- **B — Das Powerup-Wort wird optisch überschrieben.** `draw_world` zeichnet die
  Powerup-Entitäten *zuerst* (`render/mod.rs` ~Z. 234), dann die Trails *darüber*
  (~Z. 269) → der eigene Trail überdeckt das Wort.
- **C — Beim Nachschreiben fehlt Feedback.** Nur `shimmer_style` (Idle) + Cast-Ring
  sind verdrahtet. `app.trace` hält `TraceState::Tracing { progress }`, aber
  `draw_world` liest es nie. Kein Fortschritt-Highlight, kein Eintritts-/Next-Tile-
  Marker; die Cursor-Position während des Trace ist unklar.

**Leitentscheidung (A):** Der Skill liegt im **Ansteuern** — das saubere Tippen
bleibt streng (jeder Buchstabe muss sitzen, Reset bei Fehler). Gelockert wird nur
das *Andocken*.

## 2. Leitplanken

- **Trace-FSM bleibt unverändert.** Die gesamte Andock-Toleranz lebt in einem
  expliziten **Snap-on-Arm-Nudge in `app.rs` *vor* `on_char`**. Nach dem Snap sieht
  die FSM ein *exaktes* Eintritts-Tile und armt mit ihrer heutigen Logik. → Kein
  FSM-Umbau, keine Änderung an bestehenden FSM-Tests.
- **Single/Host/Client getrennt.** Snap und Feedback betreffen den Single-Flow
  (`Mode::Single`). Despawn bleibt der host-autoritative MP-Andockpunkt.
- **Visuals = render-time-Math, scroll-immun.** Kein tachyonfx über scrollendem
  Welt-Inhalt (Skill `effects`, Learning #37): das Wort scrollt cursor-zentriert
  mit, ein Zell-Effekt würde über logisch andere Zeichen schmieren.
- **Visuelle Arbeit zuerst im Companion** (`examples/hud_lab.rs`, Szene 4)
  explorieren, dann ins Spiel verdrahten (CLAUDE.md-Norm, Skill `visual-companion`).

---

## 3. A — Tolerantes Andocken via Snap-on-Arm

### 3.1 Mechanik

Bei korrekter Annäherung (richtige Laufrichtung + richtiger erster Buchstabe +
*grob* aufs Eintritts-Tile gezielt, ±1 Tile) wird der Cursor um **≤ 1 Tile** aufs
Eintritts-Tile gesnappt — *bevor* `on_char` schreibt. Danach schreibt `on_char` das
Tile exakt aufs Eintritts-Tile, die FSM armt normal, und der weitere Trace läuft
**exakt auf dem Wort** (Snap passiert nur beim Arming).

Das verbindet: vergebendes Andocken (Toleranz), spürbares „Einrasten" (Magnet,
aber nur ≤1 Tile — kein verwirrender Weitsprung), und perfekt sitzende C-Highlights
(weil du danach direkt auf dem Wort schreibst).

### 3.2 Neue reine Methode auf `PowerupWord` (`powerup.rs`)

```rust
/// Snap-Ziel fürs tolerante Andocken: Some(entry_tile), wenn der Cursor nah
/// genug am Eintritts-Tile ist (Chebyshev <= radius), in Laufrichtung anfährt
/// und der erste Buchstabe stimmt. `dir_delta` als (i32,i32), um keinen
/// Direction-Import (writing.rs) hereinzuziehen.
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

Konstante: `pub const ENTRY_SNAP_RADIUS: i32 = 1;` (in `powerup.rs`).

### 3.3 Verdrahtung in `app.rs` (`on_char`, Single-Branch)

*Vor* `e.on_char(c)`, nur wenn **kein Trace läuft** (`!self.trace.is_tracing()`):

```rust
if !self.trace.is_tracing() {
    let dd = e.direction.delta();
    if let Some(target) = arena.entities.iter().find_map(|ent| match &ent.kind {
        EntityKind::PowerupWord(w) => w.entry_snap(e.cursor, dd, c, ENTRY_SNAP_RADIUS),
    }) {
        e.cursor = target;
    }
}
// danach unverändert: let dir = e.direction; ... let result = e.on_char(c); ... observe
```

Borrow-Hinweis: `e` und `arena` sind disjunkte Bindings aus der `Mode::Single`-
Destrukturierung; `target` ist owned `(i32,i32)` → Arena-Borrow endet vor der
`e.cursor`-Mutation.

### 3.4 Eigenschaften

- **Rückwärts-kompatibel:** bei exaktem Treffer ist `cheb == 0` → Snap-Ziel ist
  die aktuelle Position → no-op. Der bestehende `xxxdash`-Pickup-Test (Cursor
  landet exakt auf `(3,0)`) bleibt grün.
- **Nur im Idle:** während eines laufenden Trace kein Snap (sonst würde ein
  zufällig passender Buchstabe den Cursor wegreißen).
- **1-Buchstaben-Wörter:** `len() <= 1` → Richtungs-Bedingung entfällt (konsistent
  mit der FSM-Sonderbehandlung, `writing.rs:408`).

### 3.5 Tests (TDD)

- `powerup.rs` (rein, unit): Snap bei exaktem Treffer = no-op (Some(entry) == cursor);
  Snap bei ±1 diagonal (eine Reihe versetzt); kein Snap außerhalb radius; kein Snap
  bei falscher Richtung; kein Snap bei falschem Buchstaben; 1-Buchstaben-Wort snappt
  richtungsunabhängig.
- `app.rs` (integration): Anfahrt eine Reihe versetzt sammelt das Wort ein (vorher
  Reset); laufender Trace wird nicht durch Snap gestört.

---

## 4. B — Powerup-Wort als Top-Layer

`draw_world` Render-Reihenfolge umstellen auf:

1. **Spieler-Trails** (nach `tick` sortiert — wie heute).
2. **Nicht-eingesammelte Powerup-Wörter** *darüber* (verschobener Entity-Loop).
3. **Cursor-Marker** (siehe §5 für die Trace-Unterdrückung des eigenen Pfeils).

Reiner Render-Eingriff (Loop-Verschiebung), kein Eingriff in `on_char`. Logisch
liegt beim Drüberschreiben weiterhin ein Trail-Tile unter dem Wort; es ist verdeckt
und fadet weg (`apply_trail_fade`). Das löst die optische Beschwerde minimal-invasiv.

Test: bestehender `draw_world_renders_arena_entity_at_expected_cell` bleibt gültig
(Wort liegt offset vom Cursor); ergänzend ein Test, dass ein Trail-Tile *auf* einem
Powerup-Tile das Wort-Zeichen **nicht** verdeckt (Zelle zeigt den Wort-Buchstaben).

---

## 5. C — Trace-Feedback (render-time, scroll-immun)

### 5.1 Plumbing

`draw_world` erhält zusätzlich die aktive Trace-Info, abgeleitet aus
`app.trace.state`:

```rust
let trace: Option<(u32, usize)> = match app.trace.state {
    TraceState::Tracing { id, progress } => Some((id, progress)),
    TraceState::Idle => None,
};
```

### 5.2 Styling pro Wort (im Top-Layer-Loop aus §4)

- **Idle-Wort** (kein aktiver Trace ODER andere `id`): Eintritts-Marker auf
  `entry_tile()` + Richtungs-Pfeil (Glyph/Platzierung/Farbe in hud_lab klären,
  Kandidat: Pfeil auf `entry_tile - run_direction`, falls on-screen & leer);
  übrige Tiles `shimmer_style` wie heute.
- **Getractes Wort** (`id` matcht):
  - Tiles `0..progress` → `HIGHLIGHT_BG`-Kasten (sichtbarer Fortschritt).
  - Tile `progress` → **nächster Buchstabe in Cursor-BG-Farbe** (`theme::ACCENT`,
    ggf. leicht heller) — telegraphiert das nächste Ziel.
  - Tiles `> progress` → shimmer/gedämpft.

### 5.3 Cursor-Unterdrückung (dein Look-Hinweis)

Im Cursor-Loop: der **eigene** Pfeil wird **unterdrückt, solange ein Trace aktiv
ist** — die Next-Tile-Hervorhebung (§5.2) steht an seiner Stelle. Mitspieler-Cursor
bleiben unberührt.

**Insight (warum das sauber zusammenfällt):** nach Keystroke `k` wird `progress`
zu `k+1`, und der Cursor steht durch Write-to-Move exakt auf `keystroke_tile(k+1)`
= dem nächsten erwarteten Tile. Next-Tile-Highlight und Cursor-Position sind also
**dieselbe Zelle** → „Pfeil weg, nächsten Buchstaben in Cursor-Farbe highlighten"
ergibt genau eine kohärente Zelle.

### 5.4 Tests

- Visuals sind nicht unit-testbar → „rendert N Frames ohne Panik"-Smoke-Test mit
  aktivem Trace-State (analog `cast_flow_renders_many_frames_without_panic`).
- Die Trace-Info-Ableitung (`TraceState` → `Option<(id, progress)>`) ist eine reine
  Abbildung; bei Bedarf als kleiner Helper unit-getestet.

---

## 6. Vorgehen

1. **hud_lab Szene 4 erweitern** (Eintritts-Marker-Look, Highlight-Farben, Pfeil-
   Platzierung) und mit dem User am Bild durchschalten — *bevor* Spiel-Code fällt.
2. **TDD:** A (Snap-Logik in `powerup.rs` + Verdrahtung in `app.rs`) und B/C-Plumbing.
   Logik unit-getestet, Visuals per Smoke-Test.
3. `cargo build` / `cargo test` / `cargo clippy` grün **und warnungsfrei**
   (`cargo` nicht im PATH:
   `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"`).
4. **code-reviewer-Subagent** vor `gh pr ready`. Alles im selben PR #49.

## 7. Offene Punkte (in hud_lab zu klären, nicht jetzt)

- Exakter Eintritts-Marker-Glyph und ob/wo der Richtungs-Pfeil sitzt.
- Genauer Farbton des Next-Tile-Highlights (Cursor-BG vs. leicht heller).
- Ob die `> progress`-Tiles während des Trace gedämpft oder weiter shimmernd sind.

## 8. Nicht-Ziele (YAGNI)

- Kein Magnet-Snap über > 1 Tile, kein Richtungs-„grob"-Matching (4-Wege-Grid →
  Richtung bleibt exakt).
- Keine logische Tile-Besetzung/Schreibsperre (B bleibt render-only).
- Keine Tipp-Fehlertoleranz (Tippen bleibt streng — Skill liegt im Ansteuern).
- Kein Multiplayer-Pickup-Sync (bleibt W3-Andockpunkt).
