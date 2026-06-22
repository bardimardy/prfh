# Pull Request From Hell

> *„git blame yourself."*

**Du schreibst die Welt, durch die du läufst.** Jeder Tastendruck schreibt
ein Zeichen UND macht einen Schritt. Bestimmte Wörter wie `up`, `down`,
`left`, `right` ändern deine Laufrichtung — deine Spur bleibt sichtbar
hinter dir liegen und verblasst langsam.

Das hier ist das **Basis-Typing-Spiel**: die nackte Kern-Mechanik, auf der
das eigentliche Spiel später aufbaut.

---

## Status

🧱 **Basis / Fundament.** Spielbar, aber bewusst minimal — nur die
Schreib-und-Lauf-Mechanik. Alles Weitere wird darauf aufgesetzt.

## Die Mechanik

Ein Terminal-Programm (Rust · Ratatui · Crossterm). Du bewegst dich über
ein 2D-Spielfeld ausschließlich über die Tastatur:

- **Tippen = schreiben + laufen.** Jedes getippte Zeichen wird als Glyph
  auf das Feld geschrieben (sichtbare Spur) und schiebt den Cursor einen
  Schritt in die aktuelle Richtung.
- **Trigger-Wörter lenken den Cursor.** Sie feuern **sofort**, sobald das
  Getippte auf den Trigger endet — kein Leerzeichen, kein Enter nötig
  (z. B. `...up` dreht augenblicklich nach oben):

  | Wort | Wirkung |
  |---|---|
  | `up` / `down` / `left` / `right` | setzt die Laufrichtung |
  | `back` | kehrt die aktuelle Richtung um |
  | `stop` | pausiert — das nächste Zeichen überschreibt an Ort und Stelle |

  Hinweis: Weil Trigger auf dem **Suffix** feuern, dreht z. B. auch
  `upgrade` nach oben (das `up` darin reicht).
- **Spur & Glow.** Die Spur verblasst mit der Zeit. Ein gerade gefeuerter
  Trigger leuchtet kurz auf.
- **Leertaste ist deaktiviert.** Sie tut bewusst nichts — kein Zeichen,
  kein Schritt.

## Steuerung

| Taste | Aktion |
|---|---|
| beliebige Zeichen | schreiben & laufen / Trigger feuern |
| `Backspace` | einen Schritt zurücklaufen (Spur löschen) |
| `Esc` | beenden |

Optional: Umgebungsvariable `PRFH_DEBUG=1` setzen, um ein Debug-Overlay
(Modus, Richtung, aktuelles Wort, Cursor) einzublenden.

## Build & Run

```sh
cargo run     # Spiel starten
cargo test    # Tests ausführen
```

## Docs

- [docs/](docs/) — Design-Dokumente für das größere Spiel, das auf dieser Basis aufbauen soll.

---

*Made with verbittertem Schwarzhumor von Devs für Devs.*
