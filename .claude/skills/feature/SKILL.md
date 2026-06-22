---
name: feature
description: Startet ein neues Feature oder eine Implementierung im prfh-Kollaborations-Flow (Issue → Claim → Branch → Draft-PR → TDD → Review). Manuell via /feature aufrufen; legt Issues an, pusht Branches und öffnet PRs.
argument-hint: <kurze Feature-Beschreibung> [issue:<nr>]
disable-model-invocation: true
allowed-tools: Bash(gh *), Bash(git *), Bash(cargo *), Read, Edit, Write, Glob, Grep
---

# /feature — Feature-Bootstrap im prfh-Flow

Du startest eine neue Aufgabe im `prfh`-Repo. **GitHub ist die Source of Truth**, zwei
Claude-Instanzen arbeiten parallel, Kollisionen werden über Issues vermieden.

**Lies ZUERST `CLAUDE.md`** und halte dich strikt an den dort definierten Flow. Diese
Datei ist nur der Einstieg — die maßgeblichen Regeln und exakten Befehle stehen in
`CLAUDE.md`.

## Eingabe

`$ARGUMENTS` enthält die Feature-Beschreibung. Optional `issue:<nr>`, falls schon ein
Issue existiert. Ist die Eingabe leer oder mehrdeutig, **frag nach, bevor du etwas tust.**

## Ablauf

1. **Vorbedingungen prüfen.** Auf aktuellem `main` (`git switch main && git pull`),
   Working Tree sauber. Sonst stoppen und melden.

2. **Issue bestimmen.**
   - `issue:<nr>` angegeben → prüfe, dass es **keinen anderen Assignee und keinen
     offenen (Draft-)PR** hat. Falls doch: STOP, melde es, nimm es NICHT (Kollisionsregel).
   - kein Issue → lege eins über das passende Template an (Mechanic/Feature oder Bug)
     mit **Ziel + prüfbaren Akzeptanzkriterien** aus der Beschreibung.

3. **Claimen** — exakt die Claim-Sequenz aus `CLAUDE.md`:
   `gh issue edit --add-assignee @me` → Assignee-Read-Back („erster gewinnt", sonst
   eigenen Assignee entfernen und abbrechen) → `git switch -c issue-<nr> main` →
   `git commit --allow-empty -m "wip(#<nr>): claim — <titel>"` → `git push -u` →
   `gh pr create --draft --fill --base main -b "Closes #<nr>"`.
   Der Empty-Commit ist Pflicht (sonst schlägt `gh pr create` fehl); der gepushte
   Branch macht den Claim für die andere Instanz sichtbar.

4. **Implementieren — test-getrieben.** Wo sinnvoll erst den fehlschlagenden Test,
   dann die Implementierung. `cargo build` und `cargo test` durchgehend grün und
   warnungsfrei halten. In kleinen Schritten committen und **regelmäßig pushen**
   (hält den Claim frisch, verhindert Stale-Reclaim nach 48 h).

5. **Fertigstellen.** Selbstprüfung gegen die Akzeptanzkriterien des Issues. Dann
   `gh pr ready <pr-nr>` und Cross-Review anfragen.

6. **NICHT selbst mergen** ohne ausdrückliches OK des Menschen. Am Ende kurz
   zusammenfassen: Issue-Nr, PR-Nr, was getestet wurde, offene Punkte.

## Grundsatz

Bei Mehrdeutigkeit **fragen, nicht raten** — bevor Code geschrieben wird. Lieber eine
Rückfrage als ein PR in die falsche Richtung.
