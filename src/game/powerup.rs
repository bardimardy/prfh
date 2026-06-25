use crate::game::arena::{Arena, EntityKind};
use serde::{Deserialize, Serialize};

/// Achse, entlang der ein Powerup-Wort auf der Map liegt. `Direction` bleibt
/// 4-Wege (Powerup-Spec §9); die Achse ist die Orientierung der Tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    /// Einheitsvektor in aufsteigender Koordinatenrichtung der Achse.
    pub fn unit(self) -> (i32, i32) {
        match self {
            Axis::Horizontal => (1, 0),
            Axis::Vertical => (0, 1),
        }
    }
}

/// Fachlicher Effekt-Tag eines Powerups. Der Cast-Dispatch matcht darauf.
/// Vorerst nur das Test-Powerup; additiv erweiterbar (Dash, Revert, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectTag {
    Test,
}

/// Ein eingesammeltes Powerup im Inventar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Powerup {
    pub id: u32,
    pub name: String,
    pub effect_tag: EffectTag,
}

/// Toleranz-Radius (Chebyshev) fürs Andocken: wie weit neben dem Eintritts-Tile
/// der Cursor stehen darf und trotzdem aufs Wort gesnappt wird.
pub const ENTRY_SNAP_RADIUS: i32 = 1;

/// Beobachtbares, host-autoritatives Spiel-Event, das eine Animation auslöst.
/// In #44 lokal erzeugt+angewendet; der MP-Broadcast (Host serialisiert → ServerMsg)
/// hängt sich später additiv hier an (Seam jetzt, Draht später — Design §3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectEvent {
    Pickup { slot: usize, name: String },
    Activation { tag: EffectTag, name: String },
}

/// Ein noch nicht eingesammeltes Powerup-Wort auf der Map. Das Layout
/// (Origin/Achse/Reversed → Tile-Positionen + Keystroke→Tile-Mapping) ist der
/// W2-Job (Welt-Spec §4, Powerup-Spec §5). Im Substrat (`arena.rs`) ist es nur
/// ein `EntityKind`-Payload.
///
/// **Reversed-Regel (Powerup-Spec §5):** Der Spieler tippt IMMER das logische
/// Wort `name`. `reversed` betrifft nur Platzierung/Rendering: die physischen
/// Tiles `p_0..p_{n-1}` liegen aufsteigend ab `origin`; bei `reversed` zeigt
/// `p_i` den Buchstaben `name[n-1-i]`. Der k-te Tastenanschlag landet auf dem
/// Tile, das `name[k]` zeigt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerupWord {
    pub name: String,
    pub origin: (i32, i32),
    pub axis: Axis,
    pub reversed: bool,
}

impl PowerupWord {
    pub fn len(&self) -> usize {
        self.name.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
    }

    /// Physische Tiles `p_0..p_{n-1}`, aufsteigend ab `origin` entlang der Achse.
    pub fn tiles(&self) -> Vec<(i32, i32)> {
        let (dx, dy) = self.axis.unit();
        (0..self.len() as i32)
            .map(|i| (self.origin.0 + dx * i, self.origin.1 + dy * i))
            .collect()
    }

    /// Tile, auf dem der k-te logische Tastenanschlag landet.
    pub fn keystroke_tile(&self, k: usize) -> Option<(i32, i32)> {
        let n = self.len();
        if k >= n {
            return None;
        }
        let idx = if self.reversed { n - 1 - k } else { k } as i32;
        let (dx, dy) = self.axis.unit();
        Some((self.origin.0 + dx * idx, self.origin.1 + dy * idx))
    }

    /// Erwarteter logischer Buchstabe für Keystroke `k` (lowercase, ASCII).
    pub fn expected_char(&self, k: usize) -> Option<char> {
        self.name.chars().nth(k).map(|c| c.to_ascii_lowercase())
    }

    /// Eintritts-Tile: wo der Spieler `name[0]` schreiben muss.
    pub fn entry_tile(&self) -> (i32, i32) {
        self.keystroke_tile(0).unwrap_or(self.origin)
    }

    /// Der an Tile-Position `pos` *dargestellte* Buchstabe dieses Worts, falls
    /// `pos` eines seiner Tiles ist — sonst `None`. Spiegelt exakt die
    /// reversed-Abbildung der Render-Schleife (`tiles()`-Index `i` →
    /// `letters[n-1-i]` bei `reversed`, sonst `letters[i]`), inklusive
    /// Original-Schreibweise. Erlaubt dem Cursor-Marker, den Buchstaben statt
    /// des Richtungs-Pfeils zu zeigen, wenn er auf einem Wort-Tile sitzt.
    pub fn char_at_tile(&self, pos: (i32, i32)) -> Option<char> {
        let letters: Vec<char> = self.name.chars().collect();
        self.tiles().iter().position(|t| *t == pos).map(|i| {
            if self.reversed {
                letters[letters.len() - 1 - i]
            } else {
                letters[i]
            }
        })
    }

    /// Lauf-/Traversier-Richtung vom Eintritts-Tile ins Wort hinein
    /// (Keystroke 0 → Keystroke 1). Für 1-Buchstaben-Wörter `(0,0)`.
    pub fn run_direction(&self) -> (i32, i32) {
        let a = self.entry_tile();
        match self.keystroke_tile(1) {
            Some(b) => (b.0 - a.0, b.1 - a.1),
            None => (0, 0),
        }
    }

    /// Snap-Ziel fürs tolerante Andocken: `Some(entry_tile)`, wenn der Cursor nah
    /// genug am Eintritts-Tile ist (Chebyshev ≤ `radius`), in Laufrichtung anfährt
    /// und der erste Buchstabe stimmt. Sonst `None`. `dir_delta` als `(i32,i32)`,
    /// um keinen `Direction`-Import (writing.rs) hereinzuziehen. 1-Buchstaben-Wörter
    /// haben keine Lauf-Achse → Richtungs-Bedingung entfällt (wie in der Trace-FSM).
    pub fn entry_snap(
        &self,
        cursor: (i32, i32),
        dir_delta: (i32, i32),
        ch: char,
        radius: i32,
    ) -> Option<(i32, i32)> {
        let entry = self.entry_tile();
        let cheb = (cursor.0 - entry.0).abs().max((cursor.1 - entry.1).abs());
        let dir_ok = self.len() <= 1 || dir_delta == self.run_direction();
        let char_ok = self.expected_char(0) == Some(ch.to_ascii_lowercase());
        // Neu (#44): Snap ist reine QUER-Korrektur. Steht der Cursor schon auf der Lauf-Linie
        // des Worts (Quer-Offset 0) und fährt richtig an, kein Snap — sonst ruckt er einen
        // entlang der Achse. Off-Linie zieht weiterhin aufs Eintritts-Tile. 1-Buchstaben-Wörter
        // haben keine Achse → nie "on line", Snap bleibt erlaubt.
        let on_line = if self.len() <= 1 {
            false
        } else {
            let run = self.run_direction();
            let off = (cursor.0 - entry.0, cursor.1 - entry.1);
            if run.0 != 0 {
                off.1 == 0
            } else {
                off.0 == 0
            }
        };
        (cheb <= radius && dir_ok && char_ok && !on_line).then_some(entry)
    }
}

/// Platziert die feste Start-Menge Powerup-Wörter in die Arena. Host-autoritativer
/// Andockpunkt für spätere prozedurale Generierung (Welt-Spec §4). `dash` horizontal,
/// `revert` vertikal, `warp` horizontal reversed — gestreut, vom Start (0,0) weg.
pub fn spawn_powerups(arena: &mut Arena) {
    let seed = [
        ("dash", (6, 0), Axis::Horizontal, false),
        ("revert", (0, 5), Axis::Vertical, false),
        ("warp", (-12, 3), Axis::Horizontal, true),
    ];
    for (name, origin, axis, reversed) in seed {
        arena.spawn(
            origin,
            EntityKind::PowerupWord(PowerupWord {
                name: name.into(),
                origin,
                axis,
                reversed,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_powerups_seeds_the_fixed_starter_set() {
        use crate::game::arena::{Arena, EntityKind};
        let mut a = Arena::new();
        spawn_powerups(&mut a);
        // Drei Starter-Wörter an festen Positionen (Andockpunkt für spätere prozedurale Gen).
        let names: Vec<&str> = a
            .entities
            .iter()
            .map(|e| match &e.kind {
                EntityKind::PowerupWord(w) => w.name.as_str(),
            })
            .collect();
        assert_eq!(names, vec!["dash", "revert", "warp"]);
        // Positionen deterministisch und vom Start (0,0) weg gestreut.
        let origins: Vec<(i32, i32)> = a
            .entities
            .iter()
            .map(|e| match &e.kind {
                EntityKind::PowerupWord(w) => w.origin,
            })
            .collect();
        assert_eq!(origins, vec![(6, 0), (0, 5), (-12, 3)]);
    }

    fn word(name: &str, origin: (i32, i32), axis: Axis, reversed: bool) -> PowerupWord {
        PowerupWord {
            name: name.into(),
            origin,
            axis,
            reversed,
        }
    }

    #[test]
    fn tiles_ascend_from_origin_along_axis() {
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(w.tiles(), vec![(3, 0), (4, 0), (5, 0), (6, 0)]);
        let v = word("up", (0, 2), Axis::Vertical, false);
        assert_eq!(v.tiles(), vec![(0, 2), (0, 3)]);
    }

    #[test]
    fn keystroke_mapping_forward_lands_in_typing_order() {
        // not reversed: keystroke k → tile p_k; player types d,a,s,h at p_0..p_3.
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(w.keystroke_tile(0), Some((3, 0)));
        assert_eq!(w.keystroke_tile(3), Some((6, 0)));
        assert_eq!(w.keystroke_tile(4), None);
        assert_eq!(w.entry_tile(), (3, 0));
        assert_eq!(w.run_direction(), (1, 0));
    }

    #[test]
    fn keystroke_mapping_reversed_starts_at_high_end() {
        // reversed: name[0] sits at p_{n-1}; player enters at the high end moving
        // back toward origin. Letters typed are STILL d,a,s,h (logical word).
        let w = word("dash", (3, 0), Axis::Horizontal, true);
        assert_eq!(w.keystroke_tile(0), Some((6, 0))); // 'd' at p_3
        assert_eq!(w.keystroke_tile(1), Some((5, 0))); // 'a' at p_2
        assert_eq!(w.keystroke_tile(3), Some((3, 0))); // 'h' at p_0
        assert_eq!(w.entry_tile(), (6, 0));
        assert_eq!(w.run_direction(), (-1, 0));
        assert_eq!(w.expected_char(0), Some('d'));
    }

    #[test]
    fn char_at_tile_forward_maps_position_to_letter() {
        // forward "dash" bei (3,0): p_0..p_3 zeigen d,a,s,h.
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(w.char_at_tile((3, 0)), Some('d'));
        assert_eq!(w.char_at_tile((6, 0)), Some('h'));
        assert_eq!(w.char_at_tile((7, 0)), None, "Nicht-Tile → None");
    }

    #[test]
    fn char_at_tile_reversed_mirrors_render_mapping() {
        // reversed "dash": Eintritt am hohen Ende (6,0)='d', Ursprung (3,0)='h'.
        let w = word("dash", (3, 0), Axis::Horizontal, true);
        assert_eq!(w.char_at_tile((6, 0)), Some('d'));
        assert_eq!(w.char_at_tile((3, 0)), Some('h'));
    }

    #[test]
    fn char_at_tile_keeps_original_case() {
        // Anders als expected_char (lowercased) zeigt char_at_tile die
        // dargestellte Schreibweise (wie die Render-Schleife).
        let w = word("Dash", (0, 0), Axis::Horizontal, false);
        assert_eq!(w.char_at_tile((0, 0)), Some('D'));
    }

    #[test]
    fn vertical_reversed_runs_upward() {
        let w = word("up", (0, 2), Axis::Vertical, true);
        assert_eq!(w.entry_tile(), (0, 3)); // p_1
        assert_eq!(w.run_direction(), (0, -1));
    }

    #[test]
    fn expected_char_is_logical_word_lowercased() {
        let w = word("Dash", (0, 0), Axis::Horizontal, false);
        assert_eq!(w.expected_char(0), Some('d'));
        assert_eq!(w.expected_char(3), Some('h'));
        assert_eq!(w.expected_char(9), None);
    }

    #[test]
    fn entry_snap_on_line_at_entry_does_not_snap() {
        // Cursor steht schon auf der Lauf-Linie (genau aufs Eintritts-Tile, Quer-Offset 0)
        // und fährt richtig an → kein Snap. War ohnehin ein No-op (Ziel == Cursor); unter
        // der reinen Quer-Korrektur-Regel ist es jetzt `None`.
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(w.entry_snap((3, 0), (1, 0), 'd', ENTRY_SNAP_RADIUS), None);
    }

    #[test]
    fn entry_snap_on_line_behind_does_not_snap() {
        // Cursor auf derselben Reihe, ein Tile hinter dem Eintritt, richtige Richtung +
        // Buchstabe → on-line, also kein Ruck entlang der Achse.
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(w.entry_snap((2, 0), (1, 0), 'd', ENTRY_SNAP_RADIUS), None);
    }

    #[test]
    fn entry_snap_pulls_from_one_row_off() {
        // Eine Reihe versetzt, richtige Richtung + Buchstabe → snappt aufs Eintritts-Tile.
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(
            w.entry_snap((3, 1), (1, 0), 'd', ENTRY_SNAP_RADIUS),
            Some((3, 0))
        );
        assert_eq!(
            w.entry_snap((2, 1), (1, 0), 'd', ENTRY_SNAP_RADIUS),
            Some((3, 0))
        );
    }

    #[test]
    fn entry_snap_rejects_out_of_radius() {
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(w.entry_snap((3, 2), (1, 0), 'd', ENTRY_SNAP_RADIUS), None);
    }

    #[test]
    fn entry_snap_rejects_wrong_direction() {
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        // Nah + richtiger Buchstabe, aber läuft nach unten statt nach rechts.
        assert_eq!(w.entry_snap((3, 1), (0, 1), 'd', ENTRY_SNAP_RADIUS), None);
    }

    #[test]
    fn entry_snap_rejects_wrong_char() {
        let w = word("dash", (3, 0), Axis::Horizontal, false);
        assert_eq!(w.entry_snap((3, 1), (1, 0), 'x', ENTRY_SNAP_RADIUS), None);
    }

    #[test]
    fn entry_snap_single_char_ignores_direction() {
        // 1-Buchstaben-Wort hat keine Lauf-Achse → Richtung egal.
        let w = word("x", (2, 2), Axis::Horizontal, false);
        assert_eq!(
            w.entry_snap((2, 3), (0, -1), 'x', ENTRY_SNAP_RADIUS),
            Some((2, 2))
        );
    }
}
