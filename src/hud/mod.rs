//! HUD-/Overlay-Framework: frameless full-screen-Welt mit schwebenden HUD-Teilen.
//!
//! Kernidee: anker-basierte Platzierung statt hartem Layout. Ein HUD-Teil kennt
//! nur seinen [`Anchor`] + Wunschgröße; [`anchor_rect`] rechnet daraus das Rect
//! über der Welt aus. Neue Overlays andocken = ein weiteres `anchor_rect` +
//! Render, keine Layout-Operation. Bewusst schlank (YAGNI) — wächst mit echten
//! Consumern (Inventar-Panel etc., #31).

pub mod notify;

use ratatui::layout::Rect;

/// Ankerpunkt eines HUD-Teils relativ zur (full-screen-)Welt-Fläche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Center,
}

/// Legt ein `w×h`-Rect am gewünschten Anker innerhalb `area` ab. Größe wird auf
/// `area` geklemmt; Positionen saturieren (nie negativ / außerhalb).
pub fn anchor_rect(area: Rect, anchor: Anchor, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let cx = area.left() + area.width.saturating_sub(w) / 2;
    let cy = area.top() + area.height.saturating_sub(h) / 2;
    let (x, y) = match anchor {
        Anchor::TopLeft => (area.left(), area.top()),
        Anchor::TopCenter => (cx, area.top()),
        Anchor::TopRight => (area.right().saturating_sub(w), area.top()),
        Anchor::BottomLeft => (area.left(), area.bottom().saturating_sub(h)),
        Anchor::BottomCenter => (cx, area.bottom().saturating_sub(h)),
        Anchor::BottomRight => (area.right().saturating_sub(w), area.bottom().saturating_sub(h)),
        Anchor::Center => (cx, cy),
    };
    Rect { x, y, width: w, height: h }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_land_in_corners_and_center() {
        let a = Rect::new(0, 0, 100, 40);
        assert_eq!(anchor_rect(a, Anchor::TopLeft, 10, 2), Rect::new(0, 0, 10, 2));
        assert_eq!(anchor_rect(a, Anchor::TopRight, 10, 2), Rect::new(90, 0, 10, 2));
        assert_eq!(anchor_rect(a, Anchor::BottomLeft, 10, 2), Rect::new(0, 38, 10, 2));
        assert_eq!(anchor_rect(a, Anchor::BottomRight, 10, 2), Rect::new(90, 38, 10, 2));
        assert_eq!(anchor_rect(a, Anchor::Center, 10, 2), Rect::new(45, 19, 10, 2));
        assert_eq!(anchor_rect(a, Anchor::TopCenter, 10, 2), Rect::new(45, 0, 10, 2));
    }

    #[test]
    fn oversized_is_clamped_not_panicking() {
        let a = Rect::new(0, 0, 8, 3);
        let r = anchor_rect(a, Anchor::TopRight, 200, 200);
        assert_eq!(r, Rect::new(0, 0, 8, 3));
    }
}
