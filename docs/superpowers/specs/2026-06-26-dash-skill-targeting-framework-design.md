# Design: Skill „dash" + wiederverwendbarer Targeting-/Aim-Mode + Skill-Registry

- **Issue:** #56 (supersedes #50)
- **Datum:** 2026-06-26
- **Status:** Approved (brainstorming) → bereit für Implementation-Plan

## 1. Zusammenfassung & Ziel

`dash` wird der **erste echte Skill** des Spiels. Bisher sind `dash`/`revert`/`warp`
nur `EffectTag::Test` ohne Mechanik. Dieses Feature liefert drei Dinge in einem Zug:

1. **dash** — eine echte, spürbare Fähigkeit mit animierter Ziel-Vorschau.
2. einen **generischen Aim/Targeting-Mode**, den jeder künftige „platzieren + abfeuern"-Skill wiederverwendet.
3. eine **Skill-Registry mit Descriptor** (inkl. `rarity_weight`-Property) als Single Source
   of Truth, aus der Spawn (und später prozedurale, gewichtete Generierung) zieht.

Aus Spielersicht: Skill `dash` aufnehmen (durch Schreiben des Wortes, wie heute) →
casten (Tab → „dash" tippen) → es feuert **nicht sofort**, sondern öffnet eine
**Ziel-Vorschau** (animierter Richtungsstrahl). Mit Pfeil-links/rechts drehe ich den
Strahl frei in 8 Richtungen, mit **Enter** dashe ich, mit **Esc** breche ich ab.

## 2. Designentscheidungen (aus dem Brainstorming)

| Entscheidung | Wahl | Begründung |
|---|---|---|
| Dash-Mechanik | **Beide** Varianten (Blink/Teleport **und** Trail-Burst) im hud_lab A/B-bar; In-Game-Default Blink, leicht umschaltbar | Feel erst sehen, dann festnageln |
| Ziel-Steuerung | **Frei drehen, 8 Richtungen, feste Distanz** | Reticle-Look wie Referenzbild; feste Range hält die Mechanik simpel |
| Foundation-Scope | Skill-Descriptor + Registry + generischer Aim-Mode; `rarity_weight` als Feld **jetzt**, prozeduraler Spawn **später** | Genau die Grundlage, ohne YAGNI-Überbau |
| Multiplayer | **Nicht** in diesem PR — nur `Mode::Single` | Host-autoritativer Netz-Sync ist eigene, sorgfältige Aufgabe (Follow-up-Issue, `net-sync` Skill) |

## 3. Architektur

### 3.1 Skill-Registry & Descriptor — `src/game/skill.rs` (neu)

Der Katalog, in den sich künftige Powerups einklinken:

```rust
pub struct SkillDef {
    pub name: &'static str,        // "dash"
    pub rarity_weight: f32,        // Spawn-Gewichtung — jetzt Feld, prozedural später
    pub activation: Activation,
}

pub enum Activation {
    Instant,                        // feuert sofort beim Cast (z.B. künftiges "revert")
    Targeted(TargetingSpec),        // öffnet den Aim-Mode
}

pub struct TargetingSpec {
    pub dirs: DirSet,               // Four | Eight
    pub range: u16,                 // feste Distanz in Tiles
    // später erweiterbar: AdjustableRange { max }, AoE-Radius, …
}

pub enum DirSet { Four, Eight }
```

- `registry() -> &'static [SkillDef]` (bzw. `const SKILLS: &[SkillDef]`) ist die **Single
  Source of Truth**. `skill_def(name) -> Option<&'static SkillDef>` ist der Lookup.
- `dash` ist `Targeted(TargetingSpec { dirs: Eight, range: 6 })` (feste Default-Distanz
  6 Tiles; im hud_lab beim Feel-Test ggf. nachjustieren).
- **Verhältnis zu `EffectTag`:** `EffectTag` bleibt der Dispatch-Key für die konkrete
  Wirkung. Neu kommt `EffectTag::Dash` hinzu. Die *Aktivierungsart* (Instant vs. Targeted)
  kommt dagegen aus `SkillDef.activation` — `EffectTag` sagt „was passiert", `SkillDef`
  sagt „wie wird ausgelöst". `Powerup` trägt weiterhin `name` (für den Lookup) + `effect_tag`.

### 3.2 Spawn aus der Registry — `src/game/powerup.rs`

`spawn_powerups` zieht Namen/Metadaten aus `registry()` statt aus einem hartcodierten
Tupel-Array. Die Seed-**Positionen** bleiben vorerst fix (Andockpunkt für spätere
prozedurale Generierung). `rarity_weight` wird gelesen/durchgereicht, aber noch nicht zur
Gewichtung benutzt — das Feld existiert „schon", wie gewünscht.

### 3.3 Generischer Aim-Mode — `src/app.rs` + `src/main.rs`

Neuer optionaler Sub-State auf `App`:

```rust
pub aim: Option<AimState>,

pub struct AimState {
    pub skill_name: String,
    pub spec: TargetingSpec,
    pub dir: Aim8,            // 8-Richtungs-Enum, NEU
    pub age: Duration,        // treibt die Strahl-Animation
}
```

- `Aim8` ist ein **neues** 8-Richtungs-Enum mit `delta() -> (i32,i32)` und
  `rotate(cw: bool)` (±45°). Das bestehende 4-Wege-`Direction` bleibt unangetastet
  (Write-to-Move ist weiter 4-Wege).
- **Input-Interception** (`src/main.rs`, vor der normalen Tastenbehandlung):
  Ist `app.aim.is_some()`, dann:
  - Pfeil-links → `dir.rotate(false)` (−45°), Pfeil-rechts → `dir.rotate(true)` (+45°)
  - Enter → `app.fire_aim()`
  - Esc → `app.cancel_aim()` (NICHT Quit, solange Aim aktiv)
  - alle anderen Tasten werden im Aim-Mode geschluckt (kein Schreiben/Casten).
- **Cast-Dispatch-Anbindung:** `dispatch_cast` schlägt `skill_def(name)` nach. Bei
  `Targeted(spec)` wird `app.aim = Some(AimState { … })` gesetzt statt sofort zu feuern;
  bei `Instant` läuft der bisherige Sofort-Pfad.
- **Feuern:** `landing = cursor + dir.delta() * spec.range`. Danach die gewählte
  Dash-Mechanik anwenden, die Abfeuer-Animation triggern, `app.aim = None`.

Jeder künftige Targeted-Skill nutzt denselben Aim-Mode, indem er nur eine
`TargetingSpec` liefert und beim Feuern seine eigene Wirkung ausführt (Dispatch über
`EffectTag`).

### 3.4 Dash-Mechanik — `src/game/writing.rs` (Engine-Methoden)

Beide als pure, unit-testbare Engine-Methoden:

- **Blink/Teleport:** `cursor = landing`; `direction` auf das nächste Kardinal der
  Aim-Richtung setzen; dazwischen eine Lücke (bzw. ein kurzer Dash-Streak rein visuell).
- **Trail-Burst:** von `cursor` bis `landing` sofort `range` Trail-Tiles anhängen
  (wie „range Zeichen auf einmal tippen"); `cursor` endet auf `landing`.

hud_lab demonstriert beide; das Spiel verdrahtet eine (Default **Blink**), trivial
umschaltbar über die Dispatch-Stelle.

## 4. Visuals (verifiziert gegen tachyonfx 0.25-Quelltext)

Forschungsergebnis: tachyonfx-Effekte sind Rect/Buffer-gebunden, `Motion`/`expand`/
`sweep` sind **achsen-only** (keine Diagonalen), und mehrere Effekte (`explode`,
`evolve`, `glitch`) **übermalen das Feld schwarz**. Daher:

### 4.1 Vorschau-Strahl — `draw_dash_beam` (render-time math), neben `draw_cast_ring`

- **Render-time math, fg-only** (genau die `draw_cast_ring`-Disziplin: nur `set_char` +
  `set_fg`, nie `bg` → transparent über dem scrollenden Feld).
- Look „Flowing Gradient Pulse": pro Tile `i` entlang `cursor + dir*i`
  `hue = base + i*STEP − phase*FLOW`, `lightness = 0.6 + 0.25*sin(i*k − phase*w)`,
  Glyph je Achse/Intensität. `phase` aus `aim.age`.
- **Reticle** `◎` am Landepunkt mit Sinus-Puls (ebenfalls math, kein persistenter
  EffectManager-Eintrag während des Zielens nötig).
- Begründung „warum math": der Fluss muss jeden Frame weiterlaufen und der Strahl
  re-projiziert beim Scrollen — ein zell-gebundener Effekt würde über logisch andere
  Tiles schmieren.

### 4.2 Abfeuer-Animation — Hybrid

- **Streak** (Bewegungs-Schmierer über das Feld): render-time math, fg-only, ~120 ms,
  Kopf-Helligkeit rampt zum Ziel (`QuadOut/CircOut`-Feel).
- **Landing-Pop**: tachyonfx-Effekt `dash_landing()` (neuer Konstruktor in
  `src/effects/mod.rs`) auf einem **kleinen festen Rect** am Landepunkt, z.B.
  `fx::parallel([ fx::coalesce((250, SineOut)), fx::hsl_shift_fg([60,30,40], (250, QuadOut)) ])`,
  optional `.with_pattern(RadialPattern::center())`.
- **Optionaler Origin-Ring** über `draw_cast_ring` (math). **Kein** `explode`/`evolve`/
  `glitch` über dem Feld (verifizierter Schwarz-Übermal-Trap).

### 4.3 Guard-Fix — `src/effects/mod.rs`

`is_non_overshoot` zusätzlich um `Bounce*` und `Spring` erweitern: sie overshooten
ebenfalls ≥1.0 und sind damit latentes Panic-Risiko für `expand`/`stretch` (gleicher
Subtraktions-Underflow-Mechanismus wie Back*/Elastic*).

## 5. Dynamische HUD-Steuerzeile — `src/hud/` + `src/render/mod.rs`

Die untere Steuerzeile wird zu einem Helper `controls_line(app) -> Line`:

- Normal: `Tab cast · Esc quit` (wie heute).
- Aim aktiv: `◄ ► drehen · Enter dash · Esc ab`.

Reine Funktion des App-Zustands → unit-testbar (gibt im Aim-Mode die Aim-Hints zurück).

## 6. hud_lab-Szene — `examples/hud_lab.rs`

Neue Szene (nächste freie Nummer) zum Sichten/A-B-Testen:

- Strahl-Stile (mind. „Flowing Gradient Pulse" + 1–2 Alternativen aus der Recherche).
- Beide Dash-Mechaniken (Blink vs. Trail-Burst) per Toggle.
- Abfeuer-Animation (Streak + Landing-Pop).
- 8-Richtungs-Rotation per Pfeiltasten.
- die dynamische Hint-Zeile.

Tasten-Toggles + Hilfezeile im hud_lab-Stil. Wegwerf-Sandbox, beeinflusst das Spiel nicht.

## 7. Tests

- **skill.rs:** Registry-Lookup nach Name; `rarity_weight` vorhanden; `dash` ist `Targeted`.
- **Aim:** `Aim8::rotate` durchläuft alle 8 Richtungen korrekt (CW/CCW); `delta()` stimmt.
- **Landepunkt:** `cursor + dir.delta()*range` für mehrere Richtungen.
- **Dash-Mechanik (pure):** Blink setzt `cursor = landing`; Trail-Burst hängt genau
  `range` Tiles an und `cursor` endet auf `landing`.
- **Beam-Math:** pure Funktion von `age` (analog `popup_pulse_line`) — deterministische
  Helligkeit/Hue je Tile.
- **Effekte:** `run_to_end()`-Smoke-Test für `dash_landing()` (weit über die Timer-Dauer
  prozessieren — nur so zeigt sich ein expand-Panic).
- **HUD:** `controls_line` liefert im Aim-Mode die Aim-Hints, sonst die Default-Hints.

Durchgehend `cargo build` + `cargo test` grün und **warnungsfrei**.

## 8. Betroffene Module

| Modul | Änderung |
|---|---|
| `src/game/skill.rs` | **neu**: `SkillDef`, `Activation`, `TargetingSpec`, `DirSet`, `registry()`, `skill_def()` |
| `src/game/powerup.rs` | `spawn_powerups` zieht aus Registry; `EffectTag::Dash` |
| `src/game/writing.rs` | Blink- + Trail-Burst-Methoden; ggf. `Aim8` (oder in skill.rs) |
| `src/app.rs` | `aim: Option<AimState>`, `AimState`, `fire_aim`/`cancel_aim`, `dispatch_cast`-Anbindung |
| `src/main.rs` | Aim-Input-Interception (Pfeile/Enter/Esc) |
| `src/render/mod.rs` | `draw_dash_beam`, Streak, `controls_line`-Einbindung |
| `src/effects/mod.rs` | `dash_landing()`; `is_non_overshoot` um Bounce*/Spring erweitern |
| `src/hud/` | `controls_line(app)` |
| `examples/hud_lab.rs` | neue Dash-Aim-Szene |

## 9. Scope-Grenze & Follow-ups

- **In diesem PR:** Registry + Aim-Framework + dash (`Mode::Single`) + Visuals +
  hud_lab + Guard-Fix + Tests.
- **Follow-up-Issue:** Multiplayer/Netz-Sync für dash (host-autoritativ, neue
  `ServerMsg`-Variante für Teleport/Burst) — bewusst ausgeklammert; `net-sync` Skill lesen.
- **Später:** prozedurale, `rarity_weight`-gewichtete Spawn-Generierung (Feld ist schon da).
- **#50** (Test-Powerup entfernen/ersetzen) wird durch den echten dash funktional abgelöst.
