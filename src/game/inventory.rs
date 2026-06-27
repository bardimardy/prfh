use crate::game::powerup::Powerup;

/// Ein Inventar-Eintrag: ein Powerup plus `count` (`×N`). Stackbare Skills
/// (`SkillDef.stackable`) erhöhen den Count eines bestehenden Eintrags statt eine
/// neue Zeile anzulegen; der Count treibt z. B. Länge/Speed des Dash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvEntry {
    pub powerup: Powerup,
    pub count: u32,
}

/// Eingesammelte Powerups. Cast matcht per Prefix (Overlay-Highlight) bzw.
/// exaktem Namen (Aktivierung).
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub items: Vec<InvEntry>,
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fügt ein Powerup hinzu und liefert den Slot-Index des betroffenen Eintrags.
    /// Stackbar (`SkillDef.stackable`) + gleicher Name → Count des bestehenden
    /// Eintrags erhöhen; sonst eine neue Zeile (Count 1).
    pub fn add(&mut self, p: Powerup) -> usize {
        let stackable = crate::game::skill::skill_def(&p.name).is_some_and(|d| d.stackable);
        if stackable {
            if let Some(slot) = self
                .items
                .iter()
                .position(|e| e.powerup.name.eq_ignore_ascii_case(&p.name))
            {
                self.items[slot].count += 1;
                return slot;
            }
        }
        self.items.push(InvEntry {
            powerup: p,
            count: 1,
        });
        self.items.len() - 1
    }

    /// Entfernt den Eintrag mit exaktem Namen (case-insensitiv) ganz und gibt ihn
    /// zurück — der Cast verbraucht den **ganzen Stack** auf einmal (consume-all).
    pub fn consume(&mut self, name: &str) -> Option<InvEntry> {
        let pos = self
            .items
            .iter()
            .position(|e| e.powerup.name.eq_ignore_ascii_case(name))?;
        Some(self.items.remove(pos))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Einträge, deren Name mit `buffer` beginnt (case-insensitiv). Leerer
    /// Buffer matcht nichts (Overlay poppt erst beim Tippen).
    pub fn prefix_matches(&self, buffer: &str) -> Vec<&InvEntry> {
        if buffer.is_empty() {
            return Vec::new();
        }
        let b = buffer.to_ascii_lowercase();
        self.items
            .iter()
            .filter(|e| e.powerup.name.to_ascii_lowercase().starts_with(&b))
            .collect()
    }

    /// Exakter Name (case-insensitiv) → der zu aktivierende Eintrag.
    pub fn get_exact(&self, name: &str) -> Option<&InvEntry> {
        let n = name.to_ascii_lowercase();
        self.items
            .iter()
            .find(|e| e.powerup.name.to_ascii_lowercase() == n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::powerup::{EffectTag, Powerup};

    fn pw(name: &str, tag: EffectTag) -> Powerup {
        Powerup {
            id: 0,
            name: name.into(),
            effect_tag: tag,
        }
    }

    /// Baut ein Inventar aus Test-Powerups (EffectTag::Test → nicht-stackbar,
    /// da nicht in der Registry; jeder Name eine eigene Zeile).
    fn inv(names: &[&str]) -> Inventory {
        let mut i = Inventory::new();
        for n in names {
            i.add(pw(n, EffectTag::Test));
        }
        i
    }

    #[test]
    fn empty_buffer_matches_nothing() {
        let i = inv(&["dash", "revert"]);
        assert!(i.prefix_matches("").is_empty());
    }

    #[test]
    fn prefix_matches_case_insensitive() {
        let i = inv(&["dash", "revert", "squash"]);
        let names: Vec<&str> = i
            .prefix_matches("s")
            .iter()
            .map(|e| e.powerup.name.as_str())
            .collect();
        assert_eq!(names, vec!["squash"]);
        assert_eq!(i.prefix_matches("RE").len(), 1);
        assert_eq!(i.prefix_matches("zzz").len(), 0);
    }

    #[test]
    fn get_exact_is_case_insensitive() {
        let i = inv(&["dash"]);
        assert_eq!(
            i.get_exact("DASH").map(|e| e.powerup.name.as_str()),
            Some("dash")
        );
        assert!(i.get_exact("das").is_none());
    }

    #[test]
    fn stackable_same_name_increments_count_single_entry() {
        // "dash" ist in der Registry stackbar → drei Pickups = EIN Eintrag ×3.
        let mut i = Inventory::new();
        i.add(pw("dash", EffectTag::Dash));
        i.add(pw("dash", EffectTag::Dash));
        i.add(pw("dash", EffectTag::Dash));
        assert_eq!(i.len(), 1, "stackbar → eine Zeile");
        assert_eq!(i.get_exact("dash").unwrap().count, 3);
    }

    #[test]
    fn non_stackable_same_name_creates_separate_entries() {
        // EffectTag::Test-Namen sind nicht in der Registry → nicht-stackbar.
        let i = inv(&["potion", "potion"]);
        assert_eq!(i.len(), 2, "nicht-stackbar → zwei getrennte Zeilen");
        assert_eq!(i.items[0].count, 1);
        assert_eq!(i.items[1].count, 1);
    }

    #[test]
    fn add_returns_slot_index_of_affected_entry() {
        let mut i = Inventory::new();
        assert_eq!(i.add(pw("revert", EffectTag::Test)), 0);
        assert_eq!(i.add(pw("dash", EffectTag::Dash)), 1);
        // Erneutes dash stackt auf den bestehenden Eintrag (Slot 1), kein neuer.
        assert_eq!(i.add(pw("dash", EffectTag::Dash)), 1);
        assert_eq!(i.len(), 2);
    }

    #[test]
    fn consume_removes_whole_entry_and_returns_it() {
        let mut i = Inventory::new();
        i.add(pw("dash", EffectTag::Dash));
        i.add(pw("dash", EffectTag::Dash)); // ×2
        let taken = i.consume("dash").expect("dash entry consumed");
        assert_eq!(taken.count, 2, "ganzer Stack auf einmal");
        assert_eq!(taken.powerup.name, "dash");
        assert!(i.is_empty(), "Eintrag entfernt (consume-all)");
        assert!(i.consume("dash").is_none(), "zweites consume findet nichts");
    }
}
