//! Skill-Katalog: zentrale Beschreibung aller Powerups/Skills (Single Source of
//! Truth) + der generische 8-Wege-Zielvektor `Aim8`. `spawn_powerups` und der
//! Cast-Dispatch ziehen hieraus. `rarity_weight` ist als Property schon da —
//! prozedurale, gewichtete Welt-Generierung verdrahtet sie später.

use crate::game::powerup::EffectTag;
use crate::game::writing::Direction;

/// Welche Richtungen ein Targeting erlaubt. `dash` nutzt `Eight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirSet {
    Four,
    Eight,
}

/// Parameter eines gezielten Skills: Richtungs-Granularität + feste Reichweite
/// (in Tiles). Additiv erweiterbar (z.B. später regelbare Range / AoE-Radius).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetingSpec {
    pub dirs: DirSet,
    pub range: u16,
}

/// Wie ein Skill ausgelöst wird. `Instant` feuert sofort beim Cast; `Targeted`
/// öffnet den generischen Aim-Mode (Vorschau-Strahl, drehen, Enter feuert).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Activation {
    Instant,
    Targeted(TargetingSpec),
}

/// Statische Beschreibung eines Skills im Katalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillDef {
    pub name: &'static str,
    pub rarity_weight: f32,
    pub effect_tag: EffectTag,
    pub activation: Activation,
    /// Ob sich mehrere Pickups dieses Skills zu EINEM Inventar-Eintrag mit Count
    /// (`×N`) stapeln (`dash`) — oder jedes Pickup eine eigene Zeile bekommt.
    pub stackable: bool,
}

/// Der Katalog. Heute: `dash` (gezielt, 8 Richtungen, feste Distanz 6),
/// `revert`/`warp` als Instant-Platzhalter (echte Effekte später). Reihenfolge
/// = Seed-Reihenfolge von `spawn_powerups`.
pub fn registry() -> &'static [SkillDef] {
    &[
        SkillDef {
            name: "dash",
            rarity_weight: 1.0,
            effect_tag: EffectTag::Dash,
            activation: Activation::Targeted(TargetingSpec {
                dirs: DirSet::Eight,
                range: 6,
            }),
            stackable: true,
        },
        SkillDef {
            name: "revert",
            rarity_weight: 0.6,
            effect_tag: EffectTag::Test,
            activation: Activation::Instant,
            stackable: false,
        },
        SkillDef {
            name: "warp",
            rarity_weight: 0.3,
            effect_tag: EffectTag::Test,
            activation: Activation::Instant,
            stackable: false,
        },
    ]
}

/// Glyph-Alphabet des Dash-Strahls/Trails: code-artige Zeichen, in die sich der
/// Trail nach dem Settle „einfriert" (und aus denen der Shuffle zufällig zieht).
pub const DASH_GLYPHS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789{}[]<>/=+*&^%$#";

/// Deterministischer „Zufalls"-Glyph aus einem Seed (kein rng nötig → stabil pro
/// Frame-Bucket, reproduzierbar). Geteilt von Trail-Burst (feste Buchstaben),
/// Settle-Shuffle und Vorschau-Strahl.
pub fn rand_glyph(seed: u64) -> char {
    let h = seed
        .wrapping_mul(2_654_435_761)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    DASH_GLYPHS[(h % DASH_GLYPHS.len() as u64) as usize] as char
}

/// Aspekt-normierte Schritt-Anzahl (Tiles) des Dash für eine Richtung: vertikale
/// Zellen zählen ~2× (2:1-Zellaspekt, wie der Cast-Ring), damit ALLE 8 Richtungen
/// visuell gleich weit reichen. Die Reichweite skaliert mit dem Stack-Count
/// (`stack`) — der `×N`-Identifier treibt die Länge des Dash.
pub fn dash_steps(dir: (i32, i32), stack: u32) -> i32 {
    let beam_reach = 14.0 + 10.0 * stack as f32; // länger pro Stack
    let step_len = (((dir.0 * dir.0) + (2 * dir.1) * (2 * dir.1)) as f32)
        .sqrt()
        .max(1.0);
    (beam_reach / step_len).round().max(1.0) as i32
}

/// Speed-Faktor aus dem Stack-Count: mehr Stacks → schnellerer Dash (kürzere
/// Settle-Zeit + frühere Aktivierung). Der Cursor springt ohnehin sofort; dieser
/// Faktor bestimmt, wie schnell die Abschluss-Sequenz einrastet. Bewusst hohes
/// Basis-Tempo (#58-Balance), damit der Dash als snappy Burst klar schneller wirkt
/// als normales Schreiben — ein echter Vorteil statt langsamer „Materialisierung".
pub fn dash_speed(stack: u32) -> f32 {
    2.5 + 0.6 * (stack.saturating_sub(1)) as f32
}

// Dash-Abschluss-Timeline (Sekunden ab Cast), Stack-Speed-skaliert:
/// Zeitpunkt, zu dem das erste (base-nahe) Tile aufhört zu shuffeln und einrastet.
pub const DASH_SETTLE_BASE: f32 = 0.28;
/// Versatz pro Tile — die Tiles setzen sich gestaffelt base→tip.
pub const DASH_SETTLE_STAGGER: f32 = 0.05;

/// Settle-Zeitpunkt (s ab Cast) für das `i`-te Dash-Tile (i=1 base … i=steps tip),
/// durch den Stack-`speed` beschleunigt. `dash_settle_at(steps, speed)` ist der
/// **Skill-Abschluss** → Auslöser der Aktivierung-am-Ziel. Reine Funktion →
/// scroll-immun + unit-testbar (geteilt von App-Advance und Settle-Render).
pub fn dash_settle_at(i: i32, speed: f32) -> f32 {
    (DASH_SETTLE_BASE + i as f32 * DASH_SETTLE_STAGGER) / speed.max(0.001)
}

/// Skill per Name (case-insensitiv) nachschlagen.
pub fn skill_def(name: &str) -> Option<&'static SkillDef> {
    registry()
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(name))
}

/// 8-Wege-Zielvektor des Aim-Modes. `Direction` (writing.rs) bleibt 4-Wege für
/// Write-to-Move; `Aim8` ist nur fürs Zielen. Reihenfolge im Kreis (im
/// Uhrzeigersinn ab Norden) für `rotate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aim8 {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Aim8 {
    /// Einheitsvektor in Tile-Koordinaten (y wächst nach unten).
    pub fn delta(self) -> (i32, i32) {
        match self {
            Aim8::N => (0, -1),
            Aim8::NE => (1, -1),
            Aim8::E => (1, 0),
            Aim8::SE => (1, 1),
            Aim8::S => (0, 1),
            Aim8::SW => (-1, 1),
            Aim8::W => (-1, 0),
            Aim8::NW => (-1, -1),
        }
    }

    /// Um 45° drehen: `cw` = im Uhrzeigersinn (N→NE→E…), sonst gegen.
    pub fn rotate(self, cw: bool) -> Aim8 {
        const RING: [Aim8; 8] = [
            Aim8::N,
            Aim8::NE,
            Aim8::E,
            Aim8::SE,
            Aim8::S,
            Aim8::SW,
            Aim8::W,
            Aim8::NW,
        ];
        let i = RING.iter().position(|&d| d == self).unwrap_or(0);
        let n = if cw { i + 1 } else { i + 7 } % 8;
        RING[n]
    }

    /// Nächstes Kardinal für die Write-to-Move-Richtung nach dem Dash.
    /// Diagonalen bevorzugen die Horizontale (Default-Lauf ist horizontal).
    pub fn nearest_cardinal(self) -> Direction {
        match self {
            Aim8::N => Direction::Up,
            Aim8::S => Direction::Down,
            Aim8::E | Aim8::NE | Aim8::SE => Direction::Right,
            Aim8::W | Aim8::NW | Aim8::SW => Direction::Left,
        }
    }

    /// Start-Zielrichtung aus der aktuellen Lauf-Richtung ableiten.
    pub fn from_direction(d: Direction) -> Aim8 {
        match d {
            Direction::Up => Aim8::N,
            Direction::Down => Aim8::S,
            Direction::Left => Aim8::W,
            Direction::Right => Aim8::E,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_the_three_starter_skills_in_order() {
        let names: Vec<&str> = registry().iter().map(|d| d.name).collect();
        assert_eq!(names, vec!["dash", "revert", "warp"]);
    }

    #[test]
    fn every_skill_has_a_positive_rarity_weight() {
        assert!(registry().iter().all(|d| d.rarity_weight > 0.0));
    }

    #[test]
    fn dash_is_stackable_others_are_not() {
        assert!(skill_def("dash").unwrap().stackable, "dash stacks (×N)");
        assert!(!skill_def("revert").unwrap().stackable);
        assert!(!skill_def("warp").unwrap().stackable);
    }

    #[test]
    fn dash_steps_scale_with_stack() {
        // Mehr Stack → längerer Dash (monoton wachsend, gleiche Richtung).
        let s1 = dash_steps((1, 0), 1);
        let s3 = dash_steps((1, 0), 3);
        assert!(s3 > s1, "×3 reicht weiter als ×1 ({s3} > {s1})");
    }

    #[test]
    fn dash_steps_are_aspect_normalized_equal_reach() {
        // 2:1-Zellaspekt: vertikale Schritte zählen doppelt → vertikal ~halb so
        // viele Tiles wie horizontal, damit der Strahl visuell gleich weit reicht.
        let horiz = dash_steps((1, 0), 1);
        let vert = dash_steps((0, 1), 1);
        assert!(vert < horiz, "vertikal weniger Tiles als horizontal");
        // Grob doppelt so viele horizontale wie vertikale Tiles.
        assert!((horiz as f32 / vert as f32 - 2.0).abs() < 0.4);
    }

    #[test]
    fn dash_speed_is_snappy_and_grows_with_stack() {
        // Hohes Basis-Tempo (#58-Balance): der Dash rastet deutlich schneller ein
        // als normales Schreiben es einholen könnte, und skaliert mit dem Stack.
        assert!(dash_speed(1) >= 2.0, "Basis-Tempo snappy genug");
        assert!(dash_speed(3) > dash_speed(1));
    }

    #[test]
    fn dash_settle_staggers_base_before_tip() {
        // Späteres (tip-näheres) Tile rastet später ein als ein früheres (base).
        assert!(dash_settle_at(5, 1.0) > dash_settle_at(1, 1.0));
    }

    #[test]
    fn dash_settle_is_faster_at_higher_speed() {
        // Mehr Stack-Speed → früherer Abschluss (kürzere Settle-Zeit).
        assert!(dash_settle_at(6, 2.0) < dash_settle_at(6, 1.0));
    }

    #[test]
    fn rand_glyph_is_deterministic_in_charset() {
        assert_eq!(rand_glyph(42), rand_glyph(42), "selber Seed → selber Glyph");
        let g = rand_glyph(7);
        assert!(DASH_GLYPHS.contains(&(g as u8)), "Glyph aus dem Charset");
    }

    #[test]
    fn dash_is_targeted_eight_way_fixed_range() {
        let d = skill_def("dash").expect("dash registered");
        assert_eq!(d.effect_tag, EffectTag::Dash);
        match d.activation {
            Activation::Targeted(spec) => {
                assert_eq!(spec.dirs, DirSet::Eight);
                assert_eq!(spec.range, 6);
            }
            _ => panic!("dash must be Targeted"),
        }
    }

    #[test]
    fn skill_def_is_case_insensitive() {
        assert_eq!(skill_def("DASH").map(|d| d.name), Some("dash"));
        assert!(skill_def("nope").is_none());
    }

    #[test]
    fn aim8_delta_matches_compass() {
        assert_eq!(Aim8::N.delta(), (0, -1));
        assert_eq!(Aim8::E.delta(), (1, 0));
        assert_eq!(Aim8::SW.delta(), (-1, 1));
    }

    #[test]
    fn aim8_rotate_cycles_both_ways() {
        assert_eq!(Aim8::N.rotate(true), Aim8::NE);
        assert_eq!(Aim8::N.rotate(false), Aim8::NW);
        // Acht Schritte im Uhrzeigersinn = zurück am Start.
        let mut d = Aim8::N;
        for _ in 0..8 {
            d = d.rotate(true);
        }
        assert_eq!(d, Aim8::N);
    }

    #[test]
    fn aim8_nearest_cardinal_favors_horizontal_on_diagonals() {
        assert_eq!(Aim8::N.nearest_cardinal(), Direction::Up);
        assert_eq!(Aim8::NE.nearest_cardinal(), Direction::Right);
        assert_eq!(Aim8::SW.nearest_cardinal(), Direction::Left);
    }

    #[test]
    fn aim8_from_direction_roundtrips_cardinals() {
        assert_eq!(Aim8::from_direction(Direction::Up), Aim8::N);
        assert_eq!(Aim8::from_direction(Direction::Right), Aim8::E);
    }
}
