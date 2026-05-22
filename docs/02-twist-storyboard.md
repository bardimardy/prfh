# Twist-Storyboard: Die Karriere läuft rückwärts
### Pull Request From Hell — Narrative Choreographie v0.1

> *„The cleanest code I ever wrote was the code I never wrote yet."*

---

## 0. Die Grundwahrheit (Spieler weiß das NICHT)

Der Spieler erlebt das Spiel chronologisch falsch. Was sich anfühlt wie:

> *„Junior Dev kämpft sich durch toxische Firma, wird stärker, besiegt am Ende den Tech Lead."*

…ist in Wahrheit:

> *„Ausgebrannter Staff Engineer wird unter sadistischem Mentor schrittweise zum naiven Tag-1-Junior zurückgeformt — und der ‚Sieg' am Ende ist sein Einstieg in die Firma, der seine ganze Karriere ruiniert hat."*

Du **kämpfst nicht gegen Bugs** — du **erschaffst sie**, indem du Lines „uncodest".
Du **wirst nicht stärker** — du **verlierst Skills und Trauma-Narben**.
Der **Tech Lead** ist nicht dein Endgegner — er ist dein **erstes Onboarding-Gespräch**.

---

## 1. Die fünf Phasen der Enthüllung

Das Spiel teilt die Reveal-Arbeit in fünf eskalierende Phasen auf. Jede Phase hat:
- **Subtle:** Hinweise, die der Spieler übersieht oder als Stil interpretiert
- **Suspect:** Hinweise, die ein aufmerksamer Spieler bemerkt aber wegrationalisiert
- **Confirm:** Der Moment, der nicht mehr ignorierbar ist

### Phase 1 — „Lag oder Stil?" (Sprint 1)

**Spieler-Erwartung:** *Tutorial. Neues Spiel. Atmosphäre ist halt seltsam.*

| Typ | Hinweis |
|---|---|
| Subtle | Timestamps in `cat slack.log` zählen rückwärts: `[12:03] → [12:02] → [12:01]` |
| Subtle | Boot-Screen sagt „Loading career..." statt „New Game" |
| Subtle | Loading-Bar füllt sich von rechts nach links |
| Subtle | Cursor blinkt unregelmäßig (subtiler Hinweis auf RNG-Speed) |

**Spieler-Reaktion (gewünscht):** *„Witziger Stil. Devs eben."*

---

### Phase 2 — „Moment, das ist seltsam." (Sprint 2)

**Spieler-Erwartung:** *Ich werde besser, schlage Bugs.*

| Typ | Hinweis |
|---|---|
| Subtle | Bug-Counter im HUD startet bei `247` und sinkt mit jedem Kill |
| Subtle | Beim Boss-Treffer wird die HP-Bar des **Bosses größer**, nicht kleiner — UI-Animation ist umgekehrt |
| Subtle | XP heißt „Innocence" statt Experience |
| Subtle | „Level Down" Animation beim Aufsteigen |
| Suspect | Slack-Message-Counter: gestern 142, heute 89, morgen 23 |
| Suspect | NPC sagt: „Schön dich kennenzulernen" — beim zweiten Treffen sagt sie: „Bis morgen!" |

**Spieler-Reaktion (gewünscht):** *„Lustige UI-Mechanik. Die Devs trollen einen."*

---

### Phase 3 — „Hier stimmt was nicht." (Sprint 3)

**Spieler-Erwartung:** *Mid-Game. Ich bin Mid-Level Dev. Karriere läuft.*

| Typ | Hinweis |
|---|---|
| Subtle | Commit-Messages im Log lesen sich rückwärts: `"Revert: Initial commit"` |
| Suspect | Death-Screen sagt: *„You uncoded 47 lines today."* |
| Suspect | Items, die du „findest", sind eigentlich Items, die du *abgibst* — Inventar schrumpft trotz Loot |
| Suspect | Boss-Dialog: *„Glückwunsch, du hast die Probezeit nicht überlebt"* |
| Suspect | Job-Title im Profil: gestern „Senior", heute „Mid", morgen „Junior" |
| Confirm | Beim Speichern: *„Career saved at: Day -1247"* — negative Zahl |

**Spieler-Reaktion (gewünscht):** *„Ist das Absicht? Bug? Easter Egg?"*

---

### Phase 4 — „Oh nein." (Sprint 4)

**Spieler-Erwartung:** *End-Game. Letzter Boss-Approach.*

| Typ | Hinweis |
|---|---|
| Confirm | Rückblick-Cutscene zeigt komplette Slack-Nachrichten — rückwärts gelesen ergeben sie Sinn |
| Confirm | NPC-Dialog reagiert auf Dinge, die du noch nicht getan hast |
| Confirm | Inventar enthält Items mit Beschreibung: *„You'll need this — yesterday"* |
| Confirm | RNG-Speed wird **erklärt** im Lore-Item: *„Die Erinnerung verformt sich. Manche Tage rasen, manche kriechen."* |
| Confirm | Beim Betreten des finalen Raums: alle Timestamps sind `Day 1 — 09:00 — Welcome aboard!` |

**Spieler-Reaktion (gewünscht):** *„Oh. OH."*

---

### Phase 5 — Der Reveal (Final Boss „The Tech Lead")

**Setting:** Großraumbüro. Ein einzelner Schreibtisch. Ein ASCII-Mann im Bürostuhl.

```
        ┌─────────────────────────────────────────┐
        │                                         │
        │           ╔═══════════════╗             │
        │           ║   .─────.     ║             │
        │           ║  │ ◉   ◉ │    ║             │
        │           ║  │   ─   │    ║             │
        │           ║   `─────'     ║             │
        │           ║      │        ║             │
        │           ╚══════╪════════╝             │
        │              ┌───┴───┐                  │
        │              │ LGTM  │                  │
        │              └───────┘                  │
        │                                         │
        └─────────────────────────────────────────┘
```

**Boss-Dialog (statt PR-Comments):**

```
> Welcome to the team!
> I see you've reviewed all our code.
> Now let me review you.
>
> Type your name.
```

Spieler tippt seinen Namen.

```
> [PLAYERNAME], welcome aboard.
> Day 1. 09:00.
> Here's your first ticket.
>
> Don't worry. You'll get used to it.
> Everyone does.
> Eventually.
>
> [PRESS ENTER TO BEGIN YOUR CAREER]
```

Bei `Enter`: Bildschirm cleart. Es startet **derselbe Run** — aber aus der **anderen Perspektive**.

Der Spieler erlebt jetzt einen kurzen 2-Minuten-Epilog: er sieht den ersten Bug, den er macht; den ersten zynischen Slack-Kommentar; den ersten verschluckten Kritik-Punkt — und merkt: **das war der ganze Run, nur richtig herum.**

Credits rollen. Sie laufen **vorwärts**, was sich nach dem Spiel falsch anfühlt.

---

## 2. Lokaler MP: Der zweite Twist

Im Pair-Programming-Mode wird zu Run-Beginn zufällig (oder über versteckte Eingabe) **einer der beiden Spieler** markiert als „The Senior".

- Beide Spieler sehen *denselben* Bildschirm. Keine Markierung wer wer ist.
- Beide spielen kooperativ den ganzen Run.
- Im Reveal-Moment (Phase 5) sagt der Tech Lead:

```
> Welcome to the team!
> I see [PLAYER 1] has been mentoring [PLAYER 2] today.
>
> Or was it the other way around?
>
> [PLAYER X], you've been here for 12 years.
> [PLAYER Y], today is your first day.
>
> Show them what we do here.
```

Einer der beiden erfährt, dass *er* der toxische Senior war.
Der andere, dass *er* der naive Junior war.
Beide schauten sich den ganzen Run gegenseitig an — durch das selbe Terminal.

**Wenn die beiden Spieler nebeneinander sitzen, ist dieser Moment der Punkt des Spiels.**

---

## 3. Discoverability-Tuning

Risiko: Spieler verliert vorher Lust ODER bemerkt es zu früh.

**Sicherheitsnetze:**

1. **Frühe Hooks (Sprint 1):** Gameplay muss auch ohne Twist-Awareness Spaß machen — Typing-Combat trägt für sich
2. **Mid-Run Lore-Items:** Wer aufmerksam liest, kann den Twist ab Sprint 3 ahnen — soll das auch dürfen
3. **Multiple-Run-Belohnung:** Wer das Spiel ein zweites Mal startet, sieht die ersten Hinweise sofort und erlebt es als Tragödie statt Mystery
4. **Achievement: „I Knew From The Start"** — für Spieler, die unter X Min raten

---

## 4. Hinweis-Tabelle (Implementation-Reference)

Alle Hinweise als priorisierte Liste für die Implementierung:

| Prio | Hinweis | Wo? | Sprint |
|---|---|---|---|
| P0 | Bug-Counter sinkt | HUD top-right | 1 |
| P0 | Timestamps rückwärts | Slack-Log, save files | 1 |
| P0 | Death = „you uncoded N lines" | Death-Screen | 1 |
| P1 | Boss-HP füllt sich | Boss-UI | 2 |
| P1 | Slack-Counter zählt runter | NPC-Interaktion | 2 |
| P1 | „Innocence" statt XP | Profil | 1 |
| P2 | Job-Title sinkt | LinkedIn-Profil | 3 |
| P2 | NPCs reagieren auf Zukunft | Dialogue | 3 |
| P2 | Negative Day-Counter | Save | 3 |
| P3 | Commits rückwärts lesbar | Cat log | 3 |
| P3 | RNG-Speed erklärt | Lore-Item | 4 |
| P3 | Inventar schrumpft trotz Loot | Inventar-Screen | 3 |

---

## 5. Tone Maintenance

Wichtig: Der Twist ist **traurig**, nicht **lustig**. Bis Sprint 3 ist das Spiel schwarzer Humor — ab Sprint 4 darf es kippen. Der Reveal selbst soll **still** sein, nicht spektakulär. Kein „GOTCHA". Mehr: *„oh… das war traurig die ganze Zeit, und ich hab gelacht."*

---

*Document Status: v0.1 — Initial Draft*
*Dependencies: 01-game-design-doc.md*
*Next: 03-tech-architecture.md*
