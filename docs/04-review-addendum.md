# Review-Addendum & Resolutions
### Pull Request From Hell — v0.1

Antworten auf die Top-5-Action-Items aus dem GDD-Review + zusätzliche Spezifikationen für Tutorial, Accessibility, Lokalisierung, Audio, Monetarisierung und Playtesting.

---

## 1. Tutorial: „Day 0 — Onboarding"

**Diegetisches Tutorial-Sprint, das gleichzeitig den Twist seedet.**

Beim ersten Spielstart erlebt der Spieler **„Day 0"** — ein abgespeckter Mini-Sprint, der als sein erster Arbeitstag inszeniert ist. Aber: Im Run-Kontext bedeutet „Day 0" **das Ende der Karriere**, nicht den Anfang. Erst beim finalen Reveal merkt der Spieler, dass dieses „Onboarding" eigentlich sein Abschied war.

**Tutorial-Beats:**

| Beat | Lernziel | Diegetik |
|---|---|---|
| Boot | Spiel-Aesthetik einführen | „Loading career…" |
| Slack-DM | Tippen einüben | „HR: Welcome! Reply with `ok` to start." |
| First `cd` | Shell-Movement | Office-Plan, navigiere zum Schreibtisch |
| First `ls` | Inspect-Pattern | Sieh die Tools auf deinem Desk |
| First Bug | Combat-Typing | „Test-Suite is red. Type to fix." |
| First `sudo` | Ability-System | Erste Coffeine-Refill |
| First NPC | Dialog-System | Ein Senior begrüßt dich — *zynisch* |
| Day 0 Boss | „Onboarding-Form" | 5-Punkte-Form zum Wegtippen — Tone: bürokratisch |
| Day 0 End | Tutorial-Closure | „Welcome to the team. See you tomorrow." — Bildschirm bricht ab |

**Tutorial-Sprint ist 5–8 Min lang**, kein Death-möglich, alle Hinweise als Pop-up-Text in einem Slack-Window-Pendant.

**Twist-Seed:** Day 0 wird beim Reveal als **Day 4380** (12 Jahre rückwärts) entlarvt.

---

## 2. Lokaler MP: Architektur-Entscheidung

**Verworfen:** `evdev`-basierte Tastatur-Trennung (Linux-only).
**Verworfen:** Tastatur-Range-Split (physisch unbequem, Crash-Gefahr für Hand-Position).

**Gewählt:** **Multi-Instance via Unix-Socket** (Linux/macOS) + **Named Pipe** (Windows).

```
┌─────────────────┐         ┌─────────────────┐
│ Player 1        │         │ Player 2        │
│ Terminal A      │         │ Terminal B      │
│ ──────────────  │  IPC    │ ──────────────  │
│ Driver-Role     │◄───────►│ Navigator-Role  │
│ (cd, grep, mv)  │         │ (combat, sudo)  │
└─────────────────┘         └─────────────────┘
        ▲                            ▲
        └────────── Shared State ────┘
            (Authoritative on Host)
```

**Use-Cases:**
- **Couch-Coop:** Zwei tmux-Splits auf einem Monitor — Spieler sitzen nebeneinander
- **Same-Room-LAN:** Zwei Laptops, ein Spiel — über `--coop-tcp 0.0.0.0:7777`
- **Remote-Pair-Programming:** Cloud-Coop später denkbar

**MVP-Reihenfolge:** Solo → Solo-mit-Replay → Local-Coop (UDS) → LAN-Coop (TCP).

---

## 3. Twist vs. Difficulty-Curve — Plot-Hole-Auflösung

**Problem:** Wenn Karriere rückwärts läuft (Staff → Junior), wieso wird das *Spiel* schwieriger?

**Auflösung (narrativ + mechanisch):**

> *„Difficulty ist nicht die Welt — Difficulty ist deine Erinnerung an die Welt."*

- Was der Spieler als „Boss wird stärker" erlebt, ist eigentlich: **deine Naivität wächst gegen eine Welt, die immer roher wird.**
- Du verlierst keine objektiven Skills — du verlierst **Coping-Mechanismen, zynische Distanz, Workarounds**.
- Der Junior-Player hatte keine Defense gegen toxisches Verhalten. Der Staff hatte sie aufgebaut. Rückwärts gespielt: Defense bröckelt ab.

**Mechanische Umsetzung:**
- „Stärker werden" im klassischen Sinne = Combo-System wird *kürzer* belohnt, Patience-Buffer schrumpft
- Items, die du „verlierst" (rückwärts: aufgibst), waren deine Bewältigungs-Tools
- Items, die du „findest", sind Naivitäten (z.B. „Belief in Code-Review-Goodwill")
- Boss-Worte werden komplexer, weil **du** sie weniger gut parst, nicht weil sie objektiv schwerer sind

**Tagline am Ende:** *„You didn't get worse. You just remembered less."*

---

## 4. Accessibility-Spezifikation

**Eigenständiges Kapitel — vorher fehlend.**

### 4.1 Word-Difficulty-Modi

| Mode | Wort-Pool | Penalty |
|---|---|---|
| **Easy** | 3–6 Buchstaben, alphabetisch, kein Sonderzeichen | Typo = -10% Combo |
| **Normal** | 4–12 Buchstaben, gängige Dev-Wörter | Typo = Combo-Reset |
| **Hard** *(Default)* | Camelcase, regex, snake_case, Stack-Traces | Typo = Combo-Reset + Speed-Lag |
| **Sadistic** | Unicode, Pfade, Hex-Strings, multilingual | Typo = damage zurück |

### 4.2 Motorik-Optionen

- **Hold-Typing-Mode:** Halten statt einzeln tippen (für motorische Einschränkungen)
- **Word-Completion:** Erstes Zeichen reicht zum Targeting (Z-Type-Style)
- **Auto-Pause:** Bei Inaktivität >2s pausiert das Spiel
- **No-Penalty-Mode:** Vertippen kostet nichts (für stress-freies Erleben)

### 4.3 Visuelle Optionen

- **Color-Blind-Palette:** Protanopia/Deuteranopia/Tritanopia-Presets
- **High-Contrast-Mode:** Schwarz-Weiß mit minimalen Akzentfarben
- **Font-Override:** OS-Default überschreiben (für Dyslexie-freundliche Fonts in Terminal-Emulator empfehlen)
- **Animation-Reduce:** RNG-Speed-Surges deaktivierbar (verliert Twist-Hint, dafür planbar)
- **Cursor-Stability:** Flackernder Cursor abschaltbar

### 4.4 Cognitive Load

- **Tutorial-Re-Replay:** „Day 0" jederzeit aus dem Menü
- **Glossary-Command:** Im Spiel `:help <topic>` für Mechanik-Erklärungen
- **Twist-Awareness-Toggle:** Modus für Wiederholungsspieler — UI macht Reverse-Mechaniken sichtbar

### 4.5 Localization

- MVP: English only
- v1.0+: Deutsch (QWERTZ-Wortlisten), zusätzlich Französisch, Spanisch
- Tipping-Wortlisten sind keyboard-layout-aware
- Lore-Texte vollständig lokalisierbar (i18n via `fluent-rs`)

---

## 5. Scope-Split: v0.2 in zwei Releases

Original v0.2 war überladen. Neu:

### v0.2a: „Die ganze Firma" (3 Wochen)
- 3 Levels mit Etagen-Multi-Floor-Layout
- Alle 6 Bosse implementiert (vereinfacht)
- Items + 3 Loadouts (Champions)
- **Kein** RNG-Speed, **keine** Reverse-Hints — pure Roguelike-Erfahrung

### v0.2b: „Die Wahrheit" (2 Wochen)
- RNG-Speed-Mechanik aktiv
- Twist-Hinweise Phase 1–3 implementiert
- Final-Boss-Phase 4–5 als Standalone

### v0.3: „Pair Programming"
- Coop via UDS
- Traitor-Twist im MP
- Twist-Phase 5 fertig

### v1.0: „Production"
- Polish, Sound, Accessibility-Full-Pass
- Tutorial-Refinement
- Lokalisierung Deutsch
- Release auf itch.io + crates.io

---

## 6. Zusätzliche Spezifikationen

### 6.1 Audio-Konzept (vorher unterspezifiziert)

Sound ist **Träger des Twists**, nicht Beiwerk.

- **Engine:** `rodio` (Rust, pure-Rust-Audio, optional Feature)
- **Stil:** Konkret, low-fi, diegetisch
- **Layers:**
  - **Mechanical Keyboard Layer:** Jeder Keystroke ein Click — verstärkt Tippen
  - **Tinnitus-Hum:** Bei RNG-Speed-Surge leiser High-Pitch-Tone
  - **Slack-Ping:** Notification-Sounds für NPC-Dialog
  - **CRT-Hum:** Atmosphärisch im Hintergrund (sehr leise)
  - **Reverse-Cue:** Subtile rückwärts-gespielte Sounds bei wichtigen Twist-Hinweisen
- **Audio-Off:** Vollständig spielbar (Accessibility-Anforderung)

### 6.2 Telemetrie & Playtesting

- **Opt-in-Telemetry:** Run-Dauer, Death-Cause, Twist-Discovery-Sprint, Word-Difficulty
- **Local-Only-Default:** Daten landen erst remote, wenn Spieler explizit zustimmt
- **Playtesting-Plan:**
  - Solo-Dev-Phase (Self-Test): 1–2 Runs/Tag
  - Closed-Alpha (5 Tester ab v0.2a): Discord-Channel, wöchentliche Builds
  - Open-Beta (v0.3+): itch.io „in development"-Release
  - Twist-Discovery-Survey: Anonym, „In welchem Sprint hast du es bemerkt?"

### 6.3 Monetarisierung

- **Pricing-Modell:** Pay-What-You-Want auf itch.io (Min: 0€, Suggested: 5€)
- **Open-Source:** Code unter MPL-2.0 oder AGPL-3.0 (TBD), Assets unter CC-BY-NC
- **Kein DLC/IAP**, **kein Online-Account**
- **Source-Code-Editions:** Käufer bekommen optional ein „Senior-Edition"-Bundle mit Postmortem-Doc + Source-Annotations

### 6.4 Save-System (Detail)

- **Auto-Save:** Nach jedem Sprint
- **Manual-Save:** Nicht erlaubt während Run (kein Save-Scumming)
- **Multi-Slot:** 3 Karriere-Slots (= 3 verschiedene Devs)
- **Cloud-Sync:** Nicht im MVP. Stretch via Steam (falls Release dorthin).

### 6.5 Combat-Paradigm-Klärung

Aus Review: Ist `rm <enemy>` der Trigger oder das Wort über dem Enemy?

**Auflösung:**
- **Out-of-Combat:** Shell-Modus aktiv, `rm <enemy>` initiiert Combat
- **In-Combat:** Combat-Modus aktiv, Shell-Prompt deaktiviert, Wörter erscheinen direkt
- **Übergang:** Sichtbarer Mode-Switch im UI („SHELL" / „COMBAT" Label) — keine Mode-Verwirrung
- **Boss-Combat:** Direkter Eintritt, kein `rm <boss>` nötig (Boss-Räume sind diegetisch unausweichlich)

### 6.6 RNG-Speed Fairness-Locks

Aus Review: Surge bei Architect-Boss = unfair.

**Locks:**
- **Boss-Phase „Long-Form":** RNG-Modifier gecapped auf ≤1.0
- **Boss-Phase „Short-Form":** RNG-Modifier voll aktiv
- **Tutorial-Sprint:** Komplett kein RNG-Speed
- **Death-Recap:** RNG-Speed deaktiviert (Lesbarkeit)

### 6.7 Item-Spec: `.env.production` 1% Death

Aus Review: 1% pro was?

**Klärung:** 1% **pro Combat-Initiation** mit dem Item im Inventar. Pop-up vor Run-Start: *„This file should not be in version control. Are you sure?"* — explizite Curse-Choice.

---

## 7. Offene Punkte (zur späteren Klärung)

- **Bikeshedder-Boss:** Live-ändernde Wörter — alternative Mechanik vs. RSI-Risiko?
- **The Architect:** Checkpoint-System pro Absatz definieren
- **Achievement-System:** Steam vs. itch vs. eigen?
- **Modding-Support:** Custom-Boss-Scripts via RON? — Reizvoll aber Scope-Risiko
- **Trailer/Marketing:** Wann produzieren?

---

## 8. Updated Top-Level Action Items

1. ✅ Tutorial-Konzept definiert (Day 0 — Onboarding)
2. ✅ MP-Architektur entschieden (UDS/Named Pipe + TCP-Stretch)
3. ✅ Twist-Plot-Hole aufgelöst (Erinnerung vs. Welt)
4. ✅ Accessibility-Sektion geschrieben
5. ✅ v0.2 in zwei Releases gesplittet
6. 🔲 Prototyp-Skeleton bauen (Ratatui-Boilerplate, Shell-Parser, Render-Loop)
7. 🔲 Boss-Pattern-DSL designen (RON-Schema)
8. 🔲 Erste 50 Combat-Wörter kuratieren + Wortlisten-Generator
9. 🔲 Day-0-Tutorial-Skript schreiben (Dialog + Beat-Sequenz)
10. 🔲 ASCII-Boss „The Nitpicker" zeichnen (3 Frames)

---

*Document Status: v0.1 — Initial Draft*
*Dependencies: 01-game-design-doc.md, 02-twist-storyboard.md, 03-tech-architecture.md*
*Next: 05-prototype-plan.md*
