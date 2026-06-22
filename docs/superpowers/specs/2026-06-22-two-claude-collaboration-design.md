# Design: Zwei-Claude-Kollaboration über GitHub

**Datum:** 2026-06-22
**Status:** Angenommen
**Kontext:** Zwei Menschen bauen gemeinsam am Spiel `prfh`, jeder mit einer eigenen
Claude-Code-Instanz. Ziel: Die beiden Instanzen kommen sich nicht in die Quere.
GitHub (Issues + PRs + `main`) ist die einzige Source of Truth.

## Voraussetzungen

- **Zwei getrennte GitHub-Accounts**, beide Collaborator an `bardimardy/prfh`.
  Das gesamte Schema (Cross-Review, Assignee-Claim) setzt zwei unterscheidbare
  Identitäten voraus. Ein geteilter Account würde es brechen.

## Kernentscheidungen

| Bereich | Entscheidung |
|---|---|
| Isolation | `main` protected + immer grün · ein Issue = ein Branch (`issue-<nr>`) = ein PR |
| Source of Truth | GitHub Issues = Arbeits-Queue · PR `Closes #<nr>` · `main` = „was fertig ist" |
| Claim | Assignee + sofort gepushter Branch + Draft-PR (mit Empty-Commit geseedet) |
| Merge-Gate | CI grün **hart erzwungen** · Cross-Review = Norm (CLAUDE.md), nicht hart geblockt · beide Admin, Solo-Merge im Notfall möglich |
| Shared Brain | `CLAUDE.md` (Playbook) · `.claude/commands/` + `skills/` (wachsen) · Issue/PR-Templates |
| Housekeeping | `Cargo.lock` eingecheckt · `settings.local.json` gitignored · merged Branches auto-delete |

## Workflow (Issue → Merge)

1. **Pick:** Freies Issue suchen — `gh issue list --search "no:assignee no:label:claimed"`.
2. **Claim (atomar genug):**
   ```bash
   gh issue edit <nr> --add-assignee @me
   me=$(gh api user -q .login)
   owner=$(gh issue view <nr> --json assignees -q '.assignees[0].login')
   [ "$owner" = "$me" ] || { gh issue edit <nr> --remove-assignee @me; echo "lost race to $owner"; exit 1; }
   ```
   „Erster Assignee gewinnt" ist der deterministische Tiebreaker, weil die Issue-API
   keinen echten Compare-and-Swap bietet.
3. **Branch + sichtbarer Lock:**
   ```bash
   git switch -c issue-<nr> main
   git commit --allow-empty -m "wip(#<nr>): claim — <title>"
   git push -u origin issue-<nr>
   gh pr create --draft --fill --base main --head issue-<nr> -b "Closes #<nr>"
   ```
   Der **gepushte Branch** macht den Claim für die andere Instanz sichtbar — der
   reine lokale Branch reicht nicht. `gh pr create --draft` braucht zwingend einen
   Commit-Unterschied; der Empty-Commit liefert ihn.
4. **Arbeiten:** TDD wo sinnvoll, `cargo build` + `cargo test` müssen grün sein.
5. **Ready:** `gh pr ready <nr>` → Draft auf „ready for review".
6. **Review:** Die andere Person/Instanz reviewed (Norm). CI muss grün sein.
7. **Merge:** Vor Merge `main` rein (Branch-Protection „require up to date" erzwingt es).
   Merge schließt das Issue via `Closes #<nr>`. Branch wird automatisch gelöscht.

## Anti-Kollisions-Regeln

- **Nie** ein Issue anfassen, das einen Assignee oder offenen (Draft-)PR hat.
- **Stale-Claim:** Ein Draft-PR ohne Fortschritt über den Claim-Commit hinaus und
  ohne Update > 48 h gilt als verlassen → reclaimbar (auf dem PR kommentieren,
  reassignen, Branch übernehmen oder PR schließen).
- **`.claude/`-Änderungen laufen durch denselben Issue → Branch → PR-Flow.** Keine
  Sonderbehandlung. Viele kleine Dateien in `commands/` und `skills/` statt eines
  wachsenden Monolithen, damit Edits selten dieselbe Datei treffen.

## Branch-Protection (`main`)

Einmalig von einem Admin zu setzen:

- Require a pull request before merging
- Require status checks to pass: der CI-Job **Check & Test (ubuntu-latest)**
- **Require branches to be up to date before merging**
- Required approvals: **0** (Cross-Review ist Norm, nicht hart erzwungen — verhindert
  den N=2-Deadlock, wenn eine Person abwesend ist)
- Admins dürfen bypassen (Notfall-Solo-Merge)
- Repo-Setting: **Automatically delete head branches**

## CI

- **Pull Request:** nur `ubuntu-latest` → schnelles grünes Gate.
- **Push auf `main`:** volle Matrix `ubuntu + macos + windows` → Plattform-Absicherung
  nach dem Merge.

## Bewusste Trade-offs

- Cross-Review ist Norm, nicht hart erzwungen. Grund: Bei genau zwei Personen würde
  „require 1 approval" den jeweils anderen vollständig blockieren, sobald einer weg
  ist (Self-Approval ist bei GitHub nicht möglich). Wir verlassen uns auf CI + Ehrensystem.
- Der Claim ist „atomar genug", nicht perfekt atomar — die Issue-API gibt nicht mehr her.
- Keine Merge-Queue (YAGNI bei dieser Größe).

## Out of Scope (vorerst)

Merge-Queue, geteilte `settings.json` mit Hooks, GitHub Project Board. Können später
ergänzt werden, wenn der Bedarf real wird.
