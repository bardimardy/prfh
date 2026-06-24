use crate::game::powerup::Powerup;

/// Eingesammelte Powerups. Cast matcht per Prefix (Overlay-Highlight) bzw.
/// exaktem Namen (Aktivierung).
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub items: Vec<Powerup>,
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, p: Powerup) {
        self.items.push(p);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Powerups, deren Name mit `buffer` beginnt (case-insensitiv). Leerer
    /// Buffer matcht nichts (Overlay poppt erst beim Tippen).
    pub fn prefix_matches(&self, buffer: &str) -> Vec<&Powerup> {
        if buffer.is_empty() {
            return Vec::new();
        }
        let b = buffer.to_ascii_lowercase();
        self.items
            .iter()
            .filter(|p| p.name.to_ascii_lowercase().starts_with(&b))
            .collect()
    }

    /// Exakter Name (case-insensitiv) → das zu aktivierende Powerup.
    pub fn get_exact(&self, name: &str) -> Option<&Powerup> {
        let n = name.to_ascii_lowercase();
        self.items.iter().find(|p| p.name.to_ascii_lowercase() == n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::powerup::{EffectTag, Powerup};

    fn inv(names: &[&str]) -> Inventory {
        let mut i = Inventory::new();
        for (k, n) in names.iter().enumerate() {
            i.add(Powerup {
                id: k as u32,
                name: (*n).into(),
                effect_tag: EffectTag::Test,
            });
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
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, vec!["squash"]);
        assert_eq!(i.prefix_matches("RE").len(), 1);
        assert_eq!(i.prefix_matches("zzz").len(), 0);
    }

    #[test]
    fn get_exact_is_case_insensitive() {
        let i = inv(&["dash"]);
        assert_eq!(i.get_exact("DASH").map(|p| p.name.as_str()), Some("dash"));
        assert!(i.get_exact("das").is_none());
    }
}
