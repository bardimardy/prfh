# Write-to-Move: Die Kern-Mechanik
### Pull Request From Hell — v0.2 Pivot

> *„You don't navigate the world. You write it."*

---

## 1. Das Prinzip

**Jeder Tastendruck ist ein Zeichen UND ein Schritt.**

Du schreibst kontinuierlich. Was du schreibst, erscheint **als Spur** in der Welt. Während du schreibst, bewegt sich dein Charakter in die aktuelle **Schreibrichtung**. Die Spur ist deine Vergangenheit, der nächste Buchstabe deine Bewegung.

```
$ I started this job f
                     ↑
                  (du bist hier, Richtung →)
```

Wenn du `up`, `down`, `left`, `right` schreibst, ändert sich die Richtung **am Wort-Ende**.

```
$ I started this job feeling hopeful but soon up
                                              ↑
                                          (Richtung wechselt jetzt ↑)
```

Dein nächster Buchstabe geht nach oben. Deine Sätze biegen ab. Die Welt füllt sich mit deiner Prosa.

---

## 2. Warum diese Mechanik?

- **Schreiben = Sein.** Du *bist* die Wörter, die du tippst — und die du tippen musst.
- **Memoir-Feeling.** Jeder Run ist ein Tagebucheintrag, der buchstäblich zur Karte wird.
- **Twist-Verstärker.** Wenn die Zeit rückwärts läuft, wird deine *Schrift* rückwärts gelesen. Der Reveal ist sichtbar in deiner eigenen Spur.
- **Skill-Floor + Ceiling.** Auf der Basis tippt jeder, fortgeschritten plant man Sätze als Pfade.
- **Diegetisch perfekt:** Devs tippen den ganzen Tag. Das ist die Tätigkeit, *literally*.

---

## 3. Direction-Trigger Regeln

### 3.1 Trigger-Wörter (englisch, MVP)

| Wort | Richtung |
|---|---|
| `up` | ↑ |
| `down` | ↓ |
| `left` | ← |
| `right` | → |
| `back` | aktuelle Richtung umkehren |
| `stop` | Pause (nächster Buchstabe in selber Position überschreibt) |

**Diagonale (v0.2b):** `upright`, `upleft`, `downright`, `downleft`

### 3.2 Trigger-Regel

**Standard-Modus:** Trigger nur an **Wort-Grenzen** (Leerzeichen davor + danach).

- `go up and away` → Richtung wechselt nach `up`+space ✓
- `upgrade my skills` → kein Trigger (kein Wort-Boundary nach `up`) ✓
- `way up` → Trigger ✓ (Newline / EOF zählt auch als Boundary)

**Sloppy-Modus (Accessibility-Option):** Trigger auf jedem Substring-Match — `upgrade` würde sofort nach `p` hochziehen.

### 3.3 Was passiert mit dem Trigger-Wort selbst?

Das Trigger-Wort *schreibt sich noch in der vorherigen Richtung*. Erst der **nächste** Buchstabe (nach Space oder Punkt) bewegt sich in die neue Richtung. So:

```
fell asleep at desk up_
                       
                       i (next char, geht nach oben)
                       _
                       
```

---

## 4. Schreib-Mechanik Details

### 4.1 Zeichen-Kategorien

| Zeichen | Wirkung |
|---|---|
| `a-z`, `A-Z`, `0-9` | Bewegt + schreibt |
| Space | Bewegt + schreibt Leer-Tile (Boundary für Trigger) |
| `.` `,` `!` `?` | Bewegt + schreibt + Satz-Ende-Boundary |
| `Backspace` | **Schreitet rückwärts** und löscht den letzten Buchstaben |
| `Enter` | Newline — springt zum nächsten Zeilenanfang (links unten), Richtung resetet zu → |
| Sonderzeichen `(){}[]<>` | Bewegt + schreibt, hat in Combat besondere Effekte |

### 4.2 Geschwindigkeit

- **Basis:** 1 Tile pro Keystroke
- **Combo:** Schnelles fehlerfreies Tippen erhöht „Flow"-Multiplier — höhere Multiplier = mehr Damage in Combat
- **Vertippen:** Setzt Combo zurück (nicht das Geschriebene — du musst weiter)
- **`Backspace`-Penalty:** Kostet einen „Doubt"-Punkt. Zu viel Doubt = Burnout-Risiko.

### 4.3 Welt-Boundaries

- Die Welt ist ein 2D-Grid (z.B. 200×60 Tiles pro Raum/„Page")
- Trifft du auf eine **Wand**, prallst du ab — Richtung kehrt sich um, du tippst quasi rückwärts gegen deine eigene Spur
- Trifft du auf deine **eigene Spur** → du überschreibst sie (Redraft-Mechanik, kostet Doubt)
- Verlässt du das Grid am Rand → Übergang in nächsten Raum (= „Page Break")

---

## 5. Combat-Anpassung

### 5.1 Enemies sind Wörter im Weg

Feinde erscheinen als **getypte Wörter im Raum**, oft mit Markierungs-Farbe (rot). Sie blockieren Tiles.

Zwei Wege:
1. **Through-Type:** Tippe das Wort des Enemies *als nächstes Wort* in deinem Text → Enemy stirbt, dein Text geht durch ihn hindurch
2. **Around-Type:** Schreib einen Bogen drumherum (use `up`/`down`-Trigger)

**Through-Type gibt mehr Schaden + Combo, ist aber riskant** (vertippen = du blockst dich selbst).

### 5.2 Boss-Fights

Boss-Räume sind **Schreibwerkstätten**. Der Boss spawnt Wörter in Echtzeit aus verschiedenen Richtungen auf dich zu. Du musst:
- Sie **durchtippen** (=Damage)
- Oder **abbiegen** (=Survival)

Boss-Phasen wechseln Wort-Spawn-Patterns:
- *The Nitpicker:* kurze Wörter, viele auf einmal, alle Richtungen
- *The Architect:* ein einziger 300-Zeichen-Satz, läuft direkt auf dich zu — du musst ihn als Ganzes durchschreiben oder umschiffen

### 5.3 Damage = Wortqualität

Damage skaliert mit:
- Wortlänge (länger = mehr)
- Wort-Schwierigkeit (Sonderzeichen, Camelcase: `getUserById`, `_PRIVATE_KEY`)
- Combo-Multiplier
- „Eloquence"-Bonus: Wenn dein Satz grammatikalisch und thematisch passt, Bonus-Damage

---

## 6. Items als Wörter

Items in der Welt sind **Wörter im Text-Layer**. Du sammelst sie, indem du **durch sie hindurch tippst** (Through-Type wie Enemies, aber kein Damage).

Beispiele:
- `coffee` (item) auf dem Boden → du schreibst „coffee" durch → Item im Inventar
- `tailwind.css` als längeres Item → braucht präzises Tippen
- Manche Items haben **Curse-Strings** — Pop-up vor dem Aufsammeln: „Pick up `.env.production`? Confirm by typing `yes`."

---

## 7. Shell-Modus (Sub-Mechanik)

Shell-Commands existieren weiterhin, aber als **separater Modus**:

- **In-World:** Write-to-Move ist aktiv. Du bewegst dich, schreibst, kämpfst.
- **Shell-Mode aktivieren:** Drücke `Tab` oder schreib `:` am Zeilenanfang
- **In Shell:** `ls`, `cd`, `cat`, `grep`, `git stash` funktionieren wie vorher (siehe alte docs/01 §4.1)
- **Zweck:** Inventar prüfen, Lore lesen, Notausgang nehmen, Hidden-Rooms entdecken via `grep`

**Diegetik:** Shell = du hörst auf zu „erleben" und schaust kurz in dein Terminal. Tab-Switch fühlt sich an wie Window-Switch.

---

## 8. Beispiel-Szene

**Sprint 1 — Erster Bug-Encounter:**

```
┌─ /work/repo/src/auth.ts ───────────────────────────────────┐
│                                                            │
│  My first day started with merge conflicts up              │
│                                          .                 │
│                                          i                 │
│                                          d                 │
│                                          n                 │
│  ┌─────────┐                             '                 │
│  │  BUG42  │ ←─ enemy                    t                 │
│  │ undef   │                             know what i was   │
│  └─────────┘                                               │
│                                                            │
└────────────────────────────────────────────────────────────┘
   $ status: writing... combo x3   doubt: 1   day: 4380
```

Spieler tippt: `My first day started with merge conflicts up.` → Richtung wechselt ↑. Dann `i didn't know what i was` → Bug42 ignoriert. Oder: Spieler hätte direkt nach links biegen können (`left`) und in den Bug hineintippen.

---

## 9. Twist-Verstärkung durch die Mechanik

Diese Mechanik **eskaliert den Twist**:

- **Sprint 1:** Du schreibst von links nach rechts (normal)
- **Sprint 3:** Du bemerkst, dass deine alten Texte im Hintergrund **gelöscht werden** während du neuen Text schreibst — als würde jemand hinter dir radieren
- **Sprint 5:** Die Erkenntnis: Deine Texte werden nicht gelöscht. **Du schreibst sie längst rückwärts.** Was du als „neuen Text" wahrnimmst, ist die Korrektur deiner zukünftigen Selbst, die diese Worte wieder löscht.
- **Final:** Das letzte Level startet mit einer **fertigen, vollständigen Karriere-Memoir** am Bildschirm. Mit jedem Buchstaben, den du tippst, **verschwindet** ein Wort daraus — bis am Ende eine leere Seite und der Cursor an Position (0, 0) — Tag 1.

---

## 10. Accessibility

- **Sloppy-Mode:** Trigger auf jedem Substring (für Spieler mit präziser Tippung schwer)
- **Auto-Direction:** Du tippst nur, Spiel wählt Richtung automatisch (Easy-Mode)
- **No-Doubt-Mode:** Backspace kostet nichts
- **Larger Tiles:** Jeder Buchstabe ist 2×2 statt 1×1 (Sehhilfe)
- **Direction-Indicator:** Großer Pfeil sichtbar bei aktueller Richtung (Default an, kann für Hardcore-Mode aus)

---

## 11. Skill-Expression

Pro-Tier-Spieler werden:
- **Pfade vordenken:** Sätze schreiben, deren Wörter strategisch bei Direction-Triggers landen
- **Through-Type-Routen:** Mehrere Enemies in einer Bewegung killen
- **Eloquence farmen:** Thematisch passende Sätze für Bonus-Damage
- **No-Backspace-Runs:** Zero-Doubt-Achievements

Casual-Spieler:
- Schreiben drauflos, biegen mit `up`/`down`-Wörtern ab, vermeiden Feinde

---

## 12. Offene Fragen

- **Newline-Verhalten:** Resetet Enter wirklich zur linken Seite? Oder springt zur nächsten freien Zeile relativ zur Position?
- **Sentence-Eloquence-Scoring:** Wie misst man „thematisch passend"? Whitelist? Sentiment-Analyse-Lite?
- **Lokalisierung:** Deutsche Trigger-Wörter (`oben`, `unten`, `links`, `rechts`)? Oder bleiben Trigger global English?
- **Multi-Player im neuen Modell:** Driver und Navigator könnten *abwechselnd Buchstaben tippen* — co-authored sentence? Spannend, aber konfliktreich.

---

*Document Status: v0.1 — Initial Pivot Draft*
*Supersedes: docs/01-game-design-doc.md §4.1 + §4.2 (Movement & Combat)*
*Next: GDD und Tech-Architektur anpassen*
