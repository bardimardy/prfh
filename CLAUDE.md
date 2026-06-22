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

## Code-Konventionen

- Rust 2021, `cargo fmt`-Stil. Keine Warnungen (kein `#[allow]` zum Verstecken — toten
  Code entfernen).
- Schreib Code, der zum umgebenden Code passt (Naming, Kommentar-Dichte, Idiome).
- Die Base-Mechanik lebt in `src/game/writing.rs`; Rendering in `src/render/`.

## Spezifikationen

Größere Features zuerst als Design-Doc unter `docs/superpowers/specs/` brainstormen,
dann implementieren. Das Kollaborations-Konzept liegt in
`docs/superpowers/specs/2026-06-22-two-claude-collaboration-design.md`.
