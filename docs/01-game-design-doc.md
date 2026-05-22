# PULL REQUEST FROM HELL
### Game Design Document v0.1

> *„git blame yourself."*

---

## 1. Vision Statement

Ein sadistisches Terminal-Horror-Roguelike über verbitterte Softwareentwickler.
Du tippst Shell-Commands, um durch prozedurale Ticket-Räume zu navigieren, kämpfst
gegen ASCII-Code-Reviewer-Bosse mit eskalierenden Nitpick-PR-Comments und entkommst
einer Karriere, die rückwärts läuft — ohne es zunächst zu merken.

**Pitch in einem Satz:** *Du schreibst die Welt, durch die du läufst — und merkst zu spät, dass du sie längst rückwärts schreibst.*

**Tagline:** *Typing of the Dead × Braid × Severance, gespielt in deinem Terminal.*

---

## 2. Core Pillars

1. **Tippen ist Überleben.** Jede Bewegung, jeder Angriff, jede Ability ist ein Keystroke. Geschwindigkeit + Genauigkeit = Skill-Expression.
2. **Die Firma ist der Endboss.** Die Bedrohung sind keine Monster — es sind PRs, Reviews, Meetings, Sprint-Pressure.
3. **Die Zeit ist nicht auf deiner Seite. Sie ist gegen dich. Und sie läuft falsch.** RNG-Speed-Shifts + verstecktes Time-Reversal als narrative Pointe.
4. **Schwarzer Humor über echten Schmerz.** Jeder Dev erkennt sich wieder. Wenn es nicht weh tut, ist es nicht lustig.

---

## 3. Core Gameplay Loop

```
┌────────────────────────────────────────────────────┐
│  1. Stand-up (Run-Start, Loadout wählen)           │
│  2. Sprint-Tickets clearen (prozedurale Grids)     │
│  3. Code-Review-Boss bekämpfen                     │
│  4. Retro (Items/Upgrades wählen)                  │
│  5. Nächster Sprint (schwieriger)                  │
│  6. Death (= „Burnout") → Karriere-Meta-Progress   │
└────────────────────────────────────────────────────┘
```

**Run-Länge:** 20–40 Min (5–7 Sprints, finaler Boss = „Tech Lead")

---

## 4. Mechaniken

### 4.1 Bewegung — „Write-to-Move" (Kernmechanik)

**Jeder Tastendruck schreibt ein Zeichen UND ist ein Schritt.** Du bewegst dich, indem du schreibst. Deine Spur bleibt sichtbar. Die Richtung wechselt durch eingebaute Trigger-Wörter (`up`, `down`, `left`, `right`, `back`, `stop`) an Wort-Grenzen.

**Vollständige Spezifikation:** siehe `docs/05-write-to-move.md`.

**Kurzfassung:**
- Default-Richtung: → (rechts)
- Trigger-Wörter ändern Richtung am Wort-Ende
- Backspace = ein Schritt rückwärts + Buchstaben-Löschen, kostet „Doubt"
- Welt = 2D-Grid pro Raum/„Page", Wände prallen ab, Ränder = Page Break

**Shell-Mode als Sub-Mechanik:** Per `Tab` öffnest du einen Shell-Prompt (`ls`, `cd`, `cat`, `grep`, `git stash`) für Inventar/Lore/Notausgang — siehe `docs/05` §7.

### 4.2 Combat — Through-Type oder Around-Type

Feinde sind **Wörter im Raum** (Bug-Strings wie `undef`, `NPE`, `merge_conflict`). Sie blockieren Tiles.

- **Through-Type:** Tippe das Wort des Enemies als nächstes Wort in deinem Text → Enemy stirbt, dein Text geht durch ihn hindurch → max. Damage + Combo
- **Around-Type:** Schreib einen Bogen drumherum mit `up`/`down`-Trigger → kein Damage, aber überlebbar

**Damage skaliert** mit Wortlänge, Komplexität (Camelcase, Sonderzeichen), Combo-Multiplier und „Eloquence" (thematisch passende Sätze = Bonus).

**Combo-System:** Konsekutive fehlerfreie Through-Types = Multiplier (`x2`, `x4`, „PR APPROVED" bei `x10`).

**Penalty:** Vertippen resetet Combo. Backspace kostet Doubt (zu viel = Burnout-Risiko).

### 4.3 Bosse (ASCII Code-Reviewer)

Jeder Boss ist ein archetypischer toxischer Reviewer. Boss-Fight = Tippe Nitpick-Comments weg, bevor sie auf deinem PR landen.

| # | Name | Mechanik |
|---|---|---|
| 1 | **The Nitpicker** | Wirft kurze, aber endlose Style-Comments (`missing semicolon`, `prefer const`) |
| 2 | **The Architect** | Lange philosophische PR-Walls of Text — Multi-Satz-Tipp-Marathons |
| 3 | **The Ghost Reviewer** | Comments erscheinen verzögert + unsichtbar (musst `grep` triggern) |
| 4 | **The Bikeshedder** | Verschiebt Diskussion auf Trivialitäten — Wörter ändern sich live beim Tippen |
| 5 | **The Senior Who Wrote It In 2014** | „I have context you don't" — Comments enthalten Insider-Variablen, die nirgendwo definiert sind |
| 6 | **The Tech Lead** *(Final)* | Kombiniert alle Vorgänger. Reveal-Moment. |

### 4.4 RNG-Speed (Pacing-Chaos)

Alle 30–120s wechselt das Spiel die globale Tick-Rate zufällig:

- **Surge** (1.5×–2×): Wörter scrollen schneller, Combat hektischer
- **Stall** (0.5×): Alles in Slow-Motion, atmosphärisch unheimlich
- **Normal** (1×)

Wird in der UI **nicht** angezeigt — Spieler soll denken: „Lag? Mein WLAN?"
Subtiles Audio-Tinnitus + flackernder Cursor als einzige Hinweise.

### 4.5 Der Twist: Time Reversal

**Das gesamte Spiel läuft rückwärts.** Der Spieler erlebt es chronologisch falsch.

**Versteckte Hinweise (eskalierend pro Sprint):**

| Sprint | Hinweis |
|---|---|
| 1 | Timestamps in Logs zählen rückwärts (`12:03 → 12:02 → 12:01`) |
| 2 | Bug-Counter im HUD sinkt statt zu steigen, wenn du „kämpfst" |
| 3 | Boss-Health-Bars *füllen* sich beim Treffen statt zu leeren |
| 4 | NPC-Slack-Messages ergeben rückwärts gelesen mehr Sinn |
| 5 | Death-Recap: „You **uncoded** 247 lines today" |
| 6 | Letzter Boss enthüllt: du bist nicht der Held der Story, du erschaffst die Bugs |

**Finale Enthüllung:** Run = Karriere rückwärts. Letztes Level = **Tag 1 im Job.**
Der „Death/Burnout" = dein Onboarding. Du wirst nicht besser — du wirst naiver.

### 4.6 Items / Loot

Items sind Code-Artefakte. Inventar = `package.json`.

| Item | Effekt | Curse? |
|---|---|---|
| `tailwind.css` | +Speed | — |
| `jquery.js` | +Damage | -Speed (Legacy-Pinalty) |
| `node_modules/` | +Inventar-Slots | startet langsamer (Lade-Zeit) |
| `.env.production` | +Crit-Chance | 1% Chance: instant-Death (Leak) |
| `Stack Overflow Tab` | Heal | nur 1× pro Run |
| `Coffee` | Jetpack-Refill | Über-Konsum = Heart-Attack |
| `Linter Config` | Auto-correct 1 Typo/min | nervt mit Pop-ups |
| `Mechanical Keyboard` | Combo-Multiplier +0.5 | Geräusch erhöht Aggro-Range |

### 4.7 Dev-„Champions" (Loadouts)

Inspiriert von LoL-Champion-Auswahl, abgespeckt. 3 Start-Klassen:

| Klasse | Stärke | Schwäche | Vibe |
|---|---|---|---|
| **Backend Stoiker** | High HP, Slow Type, Lange Wörter OK | Schwach gegen UI-Bugs | „Es kompiliert." |
| **Frontend Mage** | Fast Type, hohe Crit | Glass Cannon | „Es sieht hübsch aus." |
| **DevOps Schatten** | Hohe Mobility (Jetpack +50%) | Mid Damage | „Es läuft. Irgendwo." |

---

## 5. Local Multiplayer: „Pair Programming Mode"

Asymmetrische 2-Spieler-Coop, geteiltes Terminal (Split-Layout via Ratatui).

| Rolle | Eingabe | Aufgabe |
|---|---|---|
| **Driver** | Linke Tastatur-Hälfte | Navigation (`cd`, Movement, `grep` für Hidden Rooms) |
| **Navigator** | Rechte Tastatur-Hälfte | Combat-Typing, Boss-Mechaniken, Ability-Trigger |

- **Shared:** HP, Coffeine-Pool, Items, Combo-Multiplier.
- **Tension:** Beide müssen synchron tippen, um Bosse zu besiegen — Kommunikation ist Pflicht.
- **Twist-Erweiterung im MP:** Finaler Reveal enthüllt, dass *einer der beiden* (zufällig gewählt zu Beginn) eigentlich der Senior-Reviewer war, der den anderen ge-mobbt hat. Schweigend. Den ganzen Run lang. 🔪

**Alternative MP-Modi (Stretch):**
- *Code Duel:* PvP-Splitscreen, schnellster Tipper gewinnt
- *Sprint Race:* Asynchron, höchste Combo gewinnt

---

## 6. Meta-Progression: Die Karriere

Zwischen Runs öffnet sich das **„LinkedIn-Profil"** — eine perverse Persistenz-Schicht.

- Skills levelt = Buffs für nächsten Run
- „Endorsements" = unlockbare Items
- Job-Titel steigen (Junior → Mid → Senior → Staff → Burnout)
- **Aber:** Da die Zeit rückwärts läuft, *sinkt* dein Titel mit jedem Run (Spieler merkt es spät)

---

## 7. Tech Stack

- **Engine:** Rust + Ratatui + Crossterm
- **Event-Loop:** `rat-salsa` für Timer / RNG-Speed-Ticks
- **Roguelike-Helpers:** `bracket-lib` (FOV, Pathfinding, RNG)
- **ASCII-Assets:** chafa für Bild→ASCII Frame-Sequences, manuelles Cleanup
- **Persistence:** lokale JSON für Meta-Progress
- **MP:** Single-Process, zwei Input-Handler (Tastatur-Range-Split oder zweites USB-Keyboard via `evdev`)

---

## 8. Aesthetik & Tone

- **Farbpalette:** Terminal-Standard (16 Farben), aber bewusste Akzente — Bug-Rot, PR-Grün, Comment-Gelb, Warning-Orange
- **Typografie:** Monospace, `figlet` für Boss-Titel, Damage-Numbers floaten als ASCII-Sprites
- **Sound:** *Optional, falls möglich* — Mechanical-Keyboard-Clicks, Slack-Notification-Tinnitus, leiser CRT-Hum
- **Tone:** *Office Space* trifft *Severance*. Niemals albern — der Humor liegt im Wiedererkennen.

---

## 9. Scope & Milestones

### MVP (Sprint 1)
- 1 Level, prozedurale 5×5 Grid-Generation
- 3 Enemy-Typen, Basic-Typing-Combat
- 1 Boss (The Nitpicker)
- Solo-Modus, keine Meta-Progression

### v0.2
- 3 Levels, alle 6 Bosse
- Items + Loadouts
- RNG-Speed-Mechanik
- Time-Reversal-Hinweise

### v0.3
- Lokaler MP (Pair Programming)
- Meta-Progression („LinkedIn-Profil")
- Twist-Finale komplett

### v1.0
- Polishing, Balancing, Sound, Tutorial („Onboarding")
- Release auf itch.io / crates.io

---

## 10. Risiken & Open Questions

- **Typing-Latenz:** Crossterm-Input muss <16ms reagieren — testen mit Mechanical-KB
- **Lokaler MP-Input:** Wie elegant trennt man zwei Tastaturen in einem Terminal? evdev/IOKit prüfen
- **Time-Reversal-Discoverability:** Wann ist der Reveal *zu spät* (Spieler verliert vorher Lust) vs. *zu offensichtlich* (kein Aha)?
- **Boss-Längen-Balance:** Endlos-Typing-Sessions sind anstrengend — Pacing pro Boss tunen
- **ASCII-Animation-Performance:** 30+ FPS bei großen Bossen → Diff-Rendering vs. Full-Redraw?

---

## 11. Inspirationen / Referenzen

- **Mechanik:** Typing of the Dead, Epistory, Z-Type, Hades, Rogue, Brogue CE
- **Twist:** Braid, Outer Wilds, Soma, Undertale
- **Tone:** Severance, Office Space, Silicon Valley, The Stanley Parable
- **Tech:** terminal-phase, pokete, sshtron, Dwarf Fortress

---

*Document Status: v0.1 — Initial Draft*
*Next: Twist-Choreographie (`02-twist-storyboard.md`) & Tech-Skeleton (`03-tech-architecture.md`)*
