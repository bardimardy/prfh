//! Dark-Mode-Palette für `prfh` — Single Source of Truth für alle Farben.
//!
//! Render-berührende Issues sollen Farben hier referenzieren statt eigene
//! Hex-Werte zu hardcoden. Werte eingefroren in Spec §2.

use ratatui::style::Color;

/// Blau — HUD/Overlay-Text, Überschriften, Akzente.
pub const ACCENT: Color = Color::Rgb(0x5A, 0xA9, 0xFF);
/// Pink — Highlighting (getippter Prefix).
pub const HIGHLIGHT_BG: Color = Color::Rgb(0xFF, 0x49, 0xA0);
/// Dunkler Text auf dem Pink-Kasten.
pub const HIGHLIGHT_FG: Color = Color::Rgb(0x14, 0x10, 0x12);
/// Panel-/Overlay-Füllung.
pub const PANEL_BG: Color = Color::Rgb(0x26, 0x26, 0x2B);
/// Pinkes `×N`-Stack-Badge im Inventar (Count ≥ 2).
pub const STACK_BADGE: Color = Color::Rgb(0xFF, 0x5C, 0xA8);
/// Gesättigte Basis für den Pickup-Regenbogen.
pub const PICKUP_BASE: Color = Color::Rgb(0xFF, 0x40, 0x80);
/// Lesbarer Body-Text.
pub const TEXT: Color = Color::Rgb(0xC8, 0xCC, 0xD4);
/// Gedämpfter Text, Borders.
pub const TEXT_DIM: Color = Color::Rgb(0x6A, 0x6E, 0x78);
/// Warn-/Fehlerakzent.
pub const DANGER: Color = Color::Rgb(0xE5, 0x4B, 0x4B);
/// Heller Flash beim Pickup-Landen (pop-pulse). Bewusster Look-Zusatz über §2;
/// warmes Off-White statt Reinweiß, damit es in die Dark-Palette passt.
pub const PICKUP_FLASH: Color = Color::Rgb(0xFF, 0xF4, 0xE6);

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn palette_matches_spec_hex() {
        // Werte aus Spec §2 — Single source of truth, gegen Tippfehler abgesichert.
        assert_eq!(ACCENT, Color::Rgb(0x5A, 0xA9, 0xFF));
        assert_eq!(HIGHLIGHT_BG, Color::Rgb(0xFF, 0x49, 0xA0));
        assert_eq!(HIGHLIGHT_FG, Color::Rgb(0x14, 0x10, 0x12));
        assert_eq!(PANEL_BG, Color::Rgb(0x26, 0x26, 0x2B));
        assert_eq!(PICKUP_BASE, Color::Rgb(0xFF, 0x40, 0x80));
        assert_eq!(TEXT, Color::Rgb(0xC8, 0xCC, 0xD4));
        assert_eq!(TEXT_DIM, Color::Rgb(0x6A, 0x6E, 0x78));
        assert_eq!(DANGER, Color::Rgb(0xE5, 0x4B, 0x4B));
    }
}
