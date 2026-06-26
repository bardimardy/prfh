# CLAUDE.md — Playbook für `prfh`

Dies ist die verbindliche Arbeitsanweisung für **jede Claude-Instanz**, die an diesem
Repo arbeitet. Zwei Menschen bauen hier gemeinsam mit je eigener Claude-Instanz.
**GitHub (Issues + PRs + `main`) ist die einzige Source of Truth.**

## Projekt

`prfh` ("Pull Request From Hell") ist ein Terminal-Spiel in Rust (Ratatui + Crossterm).
Aktueller Stand: die **Base-Typing-Mechanik** (Write-to-Move) als Fundament — siehe
`README.md` und `src/game/writing.rs`. Darauf wird das eigentliche Spiel gebaut.

## Build & Test

```bash
cargo build           # muss fehler- und warnungsfrei sein
cargo test            # alle Tests müssen grün sein
cargo run             # Spiel starten (PRFH_DEBUG=1 für Debug-Overlay)
```

`main` ist **immer grün**. Code, der `cargo test` bricht, wird nicht gemergt.

## Goldene Regeln

1. **Nichts wird gebaut ohne Issue.** Jede Änderung gehört zu einem GitHub-Issue.
2. **Niemals direkt auf `main` pushen.** Alles läuft über `issue-<nr>`-Branch + PR.
3. **Ein Issue = ein Branch = ein PR.**
4. **Nie ein Issue anfassen, das einen Assignee oder offenen (Draft-)PR hat** —
   sonst kollidiert ihr.

## Voraussetzung

Zwei **getrennte GitHub-Accounts**, beide Collaborator. `@me` muss euch unterscheiden.

## Workflow

### 1. Issue picken
```bash
gh issue list --search "no:assignee no:label:claimed"
```

### 2. Issue claimen (sichtbarer Lock)
```bash
gh issue edit <nr> --add-assignee @me
me=$(gh api user -q .login)
owner=$(gh issue view <nr> --json assignees -q '.assignees[0].login')
[ "$owner" = "$me" ] || { gh issue edit <nr> --remove-assignee @me; echo "Rennen verloren an $owner — anderes Issue nehmen"; exit 1; }
```
„Erster Assignee gewinnt." Läuft das durch, ist das Issue deins.

### 3. Branch + Draft-PR (der eigentliche Claim für die andere Instanz)
```bash
git switch -c issue-<nr> main
git commit --allow-empty -m "wip(#<nr>): claim — <kurztitel>"
git push -u origin issue-<nr>
gh pr create --draft --fill --base main --head issue-<nr> -b "Closes #<nr>"
```
**Wichtig:** Der Empty-Commit ist nötig, weil `gh pr create` ohne Commit-Unterschied
fehlschlägt. Der gepushte Branch macht den Claim erst remote sichtbar.

### 4. Arbeiten
- TDD wo sinnvoll. `cargo build` + `cargo test` grün halten.
- Häufig committen, regelmäßig pushen (hält den Claim „frisch").

### 5. Fertig → Review anfordern
```bash
gh pr ready <nr-des-prs>
```
Die andere Person/Instanz reviewed (Norm). CI muss grün sein.

### 6. Merge
- Vorher aktuelles `main` reinmergen (Branch-Protection erzwingt „up to date").
- Merge schließt das Issue automatisch (`Closes #<nr>`).
- Der Branch wird automatisch gelöscht.

## Stale Claims

Ein Draft-PR ohne Fortschritt über den Claim-Commit hinaus und ohne Update **> 48 h**
gilt als verlassen. Reclaim: auf dem PR kommentieren, Issue reassignen, Branch
übernehmen oder PR schließen + Issue freigeben.

## Merge- & Review-Politik

- **CI grün ist Pflicht** (hart erzwungen über Branch-Protection).
- **Cross-Review ist Norm**, aber nicht hart geblockt: Bei nur zwei Personen würde
  eine harte Approval-Pflicht den anderen blockieren, sobald einer abwesend ist.
  Beide sind Admin und dürfen im Notfall solo mergen — verantwortungsvoll nutzen.

## Dieses Repo selbst pflegen

**Beide Instanzen pflegen `.claude/` und diese `CLAUDE.md` aktiv weiter.** Entsteht ein
nützlicher Slash-Command, Skill oder eine neue Konvention → ergänzen.

- Neue Commands: `.claude/commands/`
- Neue Skills: `.claude/skills/`
- `.claude/`- und `CLAUDE.md`-Änderungen laufen durch **denselben** Issue → Branch →
  PR-Flow wie Code. Keine Sonderbehandlung.
- Viele kleine Dateien statt eines wachsenden Monolithen → weniger Merge-Konflikte.
- `.claude/settings.local.json` ist persönlich und bleibt gitignored.

### Learnings nach Implementationen festhalten (Norm)

Bringt eine Implementierung ein **nicht-offensichtliches, wiederverwendbares Learning**
zutage — ein verifizierter Panic, eine API-Falle, eine Versions-/Build-Eigenheit, ein
Test-Muster, ein Kollisions-Schnitt —, **halte es im selben PR fest**, statt es im
Gedächtnis verpuffen zu lassen:

- Domänenwissen zu einem Subsystem → fokussiertes **Skill** unter `.claude/skills/<thema>/`
  (model-invocable, mit Trigger-`description`), damit die nächste Instanz es automatisch lädt.
- Projektweite Regel/Konvention → kurzer Eintrag hier in `CLAUDE.md`.

Maßstab ist „**wenn wichtig**": Würde die nächste Instanz ohne dieses Wissen denselben
Fehler machen oder die Falle erneut suchen müssen? Dann festhalten. Reine Wald-und-Wiesen-
Implementierung ohne Überraschung → nichts dokumentieren (kein Lärm). Das ist eine
Arbeitsnorm, kein Hook: „wichtig" ist eine Urteils­frage, die kein deterministischer
Trigger treffen kann.

## Code-Konventionen

- Rust 2021, `cargo fmt`-Stil. Keine Warnungen (kein `#[allow]` zum Verstecken — toten
  Code entfernen).
- Schreib Code, der zum umgebenden Code passt (Naming, Kommentar-Dichte, Idiome).
- Die Base-Mechanik lebt in `src/game/writing.rs`; Rendering in `src/render/`.
- **`App::new()` vs. `App::new_single()`** (W3 #44): `new()` baut einen App mit
  **leerer Arena** (für Tests, die einen sauberen Welt-Zustand brauchen); `new_single()`
  seedet die Arena via `spawn_powerups` (regulärer Single-Player-Start, von `main.rs`
  benutzt). **Nicht zu einem Alias zusammenziehen** — Render-/`w2`-Tests bauen auf der
  leeren Arena auf und brechen, wenn `new()` plötzlich Powerups spawnt.
- **Netz-/Welt-Sync** (host-autoritativ) lebt in `src/net/` + `src/game/arena.rs` (Sim-Welt)
  vs. `src/game/world.rs` (Render-`WorldView` — **nicht** umbenennen). **Bevor du das
  Protokoll anfasst** (neue `ServerMsg`-Variante/-Feld, `Welcome`-Snapshot, Broadcast):
  Skill `net-sync` lesen — es kennt das Delta+Snapshot-Muster, die per-Task-grün-Disziplin
  beim Enum-Ripple, die Sim-vs-Render-Trennung und das Loopback-Test-Muster.
- **HUD/Overlays** leben in `src/hud/` — ein anker-basiertes Overlay-Framework
  (`Anchor` + `anchor_rect`) für die **frameless full-screen-UI**: die Welt füllt
  `f.area()`, HUD-Teile schweben als Overlays darüber. Neue HUD-Elemente docken an
  einem Anker an (keine Layout-OP). `src/hud/notify.rs` ist der dynamische
  `NotificationStack` (typgetrieben, gemischtes Stacking), der das alte statische
  `trigger_banner` ersetzt. `render::draw(f, &mut App, elapsed)` ist zeitgetrieben
  (`elapsed` treibt die Notifications); Notification-Phasen sind eine reine Funktion
  des Alters → unit-testbar, nur das Zeichnen nicht.
- Visuelle Effekte/Animationen laufen über `src/effects/` (tachyonfx-Wrapper) + den
  `process_effects`-Render-Hook. **Bevor du Effekte anfasst:** Skill `effects` lesen —
  es kennt die verifizierte tachyonfx-0.25-API und die HARTE Non-Overshoot-Panik-Regel
  für `expand` (nur `safe_expand` benutzen). Effekte sind nicht unit-testbar; sie werden
  per „bis zum Ende prozessiert, ohne Panik"-Smoke-Test abgesichert.
- **Visuelle/UX-Arbeit an der UI** (HUD, Overlay, Notification, Cursor, Look&Feel,
  Effekt-A/B) zuerst im **visuellen Companion** `examples/hud_lab.rs`
  (`cargo run --example hud_lab`) explorieren — isolierte, wegwerfbare Sandbox, die
  das Hauptspiel nicht beeinflusst. Skill `visual-companion` kennt Aufbau, Bedienung
  und Erweiterung. Gewählte Variante danach ins Spiel verdrahten.
- **Skills/Powerups** (#56): Der Skill-Katalog lebt in `src/game/skill.rs`
  (`SkillDef` mit `rarity_weight` + `Activation::{Instant,Targeted}`, `registry()`
  als Single Source of Truth, `Aim8` als 8-Wege-Zielvektor). `spawn_powerups`
  zieht daraus. Gezielte Skills nutzen den generischen Aim-Mode (`App.aim`,
  Pfeile drehen / Enter feuert / Esc bricht ab); der Vorschau-Strahl ist
  render-time-Math (`draw_dash_beam`, fg-only wie `draw_cast_ring`). Dash ist
  vorerst nur `Mode::Single` verdrahtet — MP-Netz-Sync ist ein Follow-up.

## Spezifikationen

Größere Features zuerst als Design-Doc unter `docs/superpowers/specs/` brainstormen,
dann implementieren. Das Kollaborations-Konzept liegt in
`docs/superpowers/specs/2026-06-22-two-claude-collaboration-design.md`.
