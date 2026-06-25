# Cast-Auto-Abort + Cursor-Buchstabe — Design

**Issue:** #54 · **Datum:** 2026-06-26 · **Stacked auf:** #53 (Cast-im-Trail-Foundation)

Zwei zusammenhängende UX-Fixes an der Cast-Mechanik (#44). Beide betreffen das
Erlebnis, ein Powerup über seinen Namen zu casten bzw. einzusammeln.

## Problem

1. **Cast-Sackgasse.** Im Cast-Modus (`Tab`) füllt jeder Tastendruck den
   `cast_buffer` und schreibt steering-neutral ins Trail (`on_cast_char`,
   `app.rs`). Der einzige Auto-Exit ist ein *exakter* Inventar-Namens-Match
   (`get_exact`). Tippt man etwas, das nie ein Powerup-Name wird, bleibt man im
   Cast hängen und produziert beliebig viel Trail-Müll — Ausstieg nur manuell
   per `Tab`/`Esc`.

2. **Pfeil verdeckt den ersten Buchstaben.** Der Cursor-Marker (`▶▲◀▼`) wird in
   `draw_world` *nach* den Powerup-Wörtern gezeichnet und überdeckt damit das
   Tile, auf dem er steht. Steht der Cursor auf dem Eintritts-Tile eines Worts,
   sieht man `▶` statt `d` (von „dash") — man erkennt nicht, was zu tippen ist.
   (Während eines *aktiven* Trace ist der Eigen-Pfeil bereits unterdrückt, damit
   das Next-Tile-Highlight zählt; die Lücke ist der Moment *vor* dem Armen.)

## Teil 1 — Cast-Auto-Abort

In `App::on_cast_char(c)` **vor** dem Schreiben prüfen, ob `cast_buffer + c` noch
Präfix irgendeines Inventar-Worts ist (`Inventory::prefix_matches`, existiert):

```rust
let mut candidate = self.cast_buffer.clone();
candidate.push(c);
if self.inventory.prefix_matches(&candidate).is_empty() {
    // Abbruch: Cast verlassen, Notification, Zeichen NORMAL verarbeiten.
    self.cast_mode = false;
    self.cast_buffer.clear();
    self.cast_start_tick = None;
    self.notifications.push(NotifyKind::Info, "✗  no spell", candidate);
    self.on_char(c); // normaler Schreib-Pfad inkl. Bewegungs-Triggern
    return;
}
// sonst: bisheriges Cast-Verhalten (steering-neutral schreiben, Buffer füllen,
//        exact-match → dispatch)
```

**Eigenschaften:**

- Subsumiert den ursprünglich gewünschten „Buffer > längster Name"-Fall: ein zu
  langer Buffer ist nie Präfix eines kürzeren/gleich langen Namens ⇒ leeres
  `prefix_matches` ⇒ Abbruch. Aber es bricht *früher* ab — beim ersten
  Buchstaben, der das Matchen unmöglich macht — statt Trail zuzumüllen.
- **Leeres Inventar:** `prefix_matches` ist immer leer ⇒ der erste Cast-Char
  droppt sofort zurück in den Normalmodus. Kein Hängenbleiben.
- **Seamless:** der auslösende Char läuft genau **einmal** durch `on_char`
  (normaler Pfad), also kein Doppel-Tile, und Bewegungs-Trigger feuern wieder.
- Reihenfolge: erst Abbruch-Check; nur wenn nicht abgebrochen wird, das
  bestehende `get_exact`-Dispatch (ein exakter Name ist Präfix seiner selbst,
  also nie vom Abbruch betroffen).

## Teil 2 — Buchstabe statt Pfeil auf Wort-Tiles

**Helper** (neu, `game/powerup.rs`), spiegelt die reversed-Logik der
Render-Schleife und ist unit-testbar:

```rust
/// Der an Tile-Position `pos` dargestellte Buchstabe dieses Worts, falls `pos`
/// eines seiner Tiles ist. Reversed-aware (gleiche Abbildung wie das Rendering).
pub fn char_at_tile(&self, pos: (i32, i32)) -> Option<char> {
    let letters: Vec<char> = self.name.chars().collect();
    self.tiles().iter().position(|t| *t == pos).map(|i| {
        if self.reversed { letters[letters.len() - 1 - i] } else { letters[i] }
    })
}
```

**Render** (`render/mod.rs`, Cursor-Schleife): Sitzt der **eigene** Cursor auf
einem Powerup-Wort-Tile und ist **kein** Trace aktiv (der Pfeil wird sonst eh
unterdrückt), rendere den Buchstaben im bestehenden Cursor-Highlight-Style
(`bg=ACCENT, fg=HIGHLIGHT_FG, BOLD`) statt des Pfeils:

```rust
let glyph = if player.is_self && trace.is_none() {
    arena.entities.iter().find_map(|e| match &e.kind {
        EntityKind::PowerupWord(pw) => pw.char_at_tile(player.cursor),
    })
} else { None };
let arrow_ch = glyph.unwrap_or(arrow_ch);
```

- Gilt für jedes überlappte Wort-Tile (praktisch meist das Eintritts-Tile, da
  ein aktiver Trace den Pfeil schon unterdrückt).
- **Andere Spieler:** unverändert Pfeil (außerhalb des Scopes — der Fix dient
  dem lokalen Spieler, der lesen will, was er tippen muss).
- Der finale Look wird kurz im Spiel bzw. `examples/hud_lab.rs` gegengecheckt
  (Projekt-Norm für visuelle Arbeit), der gewählte Highlight-Style ist aber der
  bereits im Spiel verwendete Eigen-Cursor-Style.

## Tests (TDD)

**Teil 1** (`app.rs`, Cast-Tests):
- `cast_aborts_when_prefix_no_longer_matches`: Inventar `["dash"]`, Cast, `'u'` →
  `cast_mode == false`, Tile geschrieben; danach `'p'` → Richtung wird `Up`
  (auslösender + Folge-Char liefen normal, Trigger feuert).
- `cast_with_empty_inventory_drops_on_first_char`: leeres Inventar → erster
  Cast-Char setzt `cast_mode == false`.
- `cast_keeps_running_on_valid_prefix`: Inventar `["dash"]`, Cast, `'d'`,`'a'` →
  bleibt im Cast (`cast_mode == true`, `cast_buffer == "da"`).
- Bestehende Tests bleiben grün (`cast_exact_name_dispatches_and_leaves_cast_mode`
  nutzt Inventar mit dem Wort — kein Abbruch).

**Teil 2** (`powerup.rs`):
- `char_at_tile_forward`: „dash" bei (3,0) horizontal → `(3,0)→'d'`, `(6,0)→'h'`,
  Nicht-Tile → `None`.
- `char_at_tile_reversed`: reversed „dash", Eintritt (6,0) → `(6,0)→'d'`,
  `(3,0)→'h'`.

Render selbst wird nicht unit-getestet (Grid), nur der Helper; der Look per
Augenschein im Spiel/Companion.

## Out of Scope

- Andere-Spieler-Cursor über Wörtern (bleibt Pfeil).
- Verändertes Trail-/Pickup-Verhalten — nur Cast-Abbruch + Cursor-Glyph.
