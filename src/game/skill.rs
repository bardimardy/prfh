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
        },
        SkillDef {
            name: "revert",
            rarity_weight: 0.6,
            effect_tag: EffectTag::Test,
            activation: Activation::Instant,
        },
        SkillDef {
            name: "warp",
            rarity_weight: 0.3,
            effect_tag: EffectTag::Test,
            activation: Activation::Instant,
        },
    ]
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
