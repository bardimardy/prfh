//! Dynamische Quick-Notifications: schweben oben-mitte über der Welt, bauen sich
//! center-out auf, halten kurz, ziehen sich center-in wieder zusammen.
//!
//! Ersetzt das alte statische `trigger_banner`. Die Animation ist **reine
//! Geometrie + Farb-Lerp** als Funktion des Alters (`age`) — kein tachyonfx,
//! damit `render` immutable bleibt (kein frame-persistenter EffectManager). Das
//! ist der gleiche Grundsatz wie beim Trail-Fade (Learning #37): positions-/
//! zeitbasierte Render-Mathematik gehört nicht in den zell-gebundenen
//! Effekt-Graphen. tachyonfx bleibt für diskrete Pickup-/Wellen-Effekte (#31).
//!
//! Die Phasen-Logik ist eine reine Funktion des Alters und damit voll
//! unit-testbar; nur das Zeichnen selbst ist es nicht.

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::time::Duration;

const BUILD: Duration = Duration::from_millis(240); // Bg wächst center-out
const TEXT: Duration = Duration::from_millis(220); // Text faded drauf
const HOLD: Duration = Duration::from_millis(1600); // Standzeit
const COLLAPSE: Duration = Duration::from_millis(140); // Bg+Text ziehen center-in

fn total() -> Duration {
    BUILD + TEXT + HOLD + COLLAPSE
}

/// Notification-Typ — steuert Größe (Stack mischt Größen) und Default-Akzent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyKind {
    /// Häufige, kleine Hinweise (Turn, Stop). Kompakt: eine Zeile.
    Info,
    /// Bemerkenswerte Ereignisse (Pickup, Combo-Meilenstein). Karte: 3 Zeilen.
    Event,
    /// Große Momente (Merge, Boss). Karte: 3 Zeilen, kräftiger Default-Akzent.
    Major,
}

impl NotifyKind {
    /// Höhe in Zeilen — kompakt (1) vs. Karte (3).
    pub fn height(self) -> u16 {
        match self {
            NotifyKind::Info => 1,
            NotifyKind::Event | NotifyKind::Major => 3,
        }
    }

    fn default_accent(self) -> Color {
        match self {
            NotifyKind::Info => theme::ACCENT,
            NotifyKind::Event => theme::PICKUP_BASE,
            NotifyKind::Major => theme::HIGHLIGHT_BG,
        }
    }
}

/// Lineare RGB-Interpolation (für das sanfte Auftauchen des Texts aus dem Panel).
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (ar, ag, ab) = rgb(a);
    let (br, bg, bb) = rgb(b);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

fn rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

/// Eine einzelne Notification. Lebt im [`NotificationStack`]; ihr visueller
/// Zustand ergibt sich rein aus `age`.
pub struct Notification {
    pub kind: NotifyKind,
    pub title: String,
    pub detail: String,
    accent: Color,
    age: Duration,
}

impl Notification {
    /// Breiten-Faktor 0..1 des Panels: center-out beim Aufbau, voll während
    /// Text+Hold, center-in beim Collapse.
    fn width_factor(&self) -> f32 {
        let a = self.age;
        if a < BUILD {
            a.as_secs_f32() / BUILD.as_secs_f32()
        } else if a < total() - COLLAPSE {
            1.0
        } else {
            let into = (a - (total() - COLLAPSE)).as_secs_f32();
            (1.0 - into / COLLAPSE.as_secs_f32()).max(0.0)
        }
    }

    /// Text-Sichtbarkeit 0..1: 0 während Aufbau und Collapse, faded dazwischen.
    fn text_alpha(&self) -> f32 {
        let a = self.age;
        if a < BUILD {
            0.0
        } else if a < BUILD + TEXT {
            (a - BUILD).as_secs_f32() / TEXT.as_secs_f32()
        } else if a < total() - COLLAPSE {
            1.0
        } else {
            0.0
        }
    }

    fn is_done(&self) -> bool {
        self.age >= total()
    }

    /// Volle Inhaltsbreite (ohne Aufbau-/Collapse-Faktor), inkl. Polster.
    fn full_width(&self, area: Rect) -> u16 {
        let title = self.title.chars().count() as u16;
        let detail = self.detail.chars().count() as u16;
        let w = match self.kind {
            NotifyKind::Info => title + detail + 5,
            _ => title.max(detail) + 4,
        };
        w.clamp(8, area.width)
    }

    /// Zeichnet diese Notification in das gegebene (volle) Rect. Außerhalb des
    /// aktuellen Breiten-Faktors wird nichts angefasst (Welt bleibt sichtbar).
    fn render(&self, buf: &mut Buffer, anchor_top: u16, area: Rect) {
        let h = self.kind.height();
        let full_w = self.full_width(area);
        let factor = self.width_factor();
        if factor <= 0.01 {
            return;
        }
        let w = ((full_w as f32 * factor).round() as u16).clamp(1, full_w);
        let x = area.left() + area.width.saturating_sub(w) / 2;
        let rect = Rect { x, y: anchor_top, width: w, height: h };

        // Panel-Hintergrund: Zellen leeren (Welt darunter verdecken) + Panel-BG.
        for yy in rect.top()..rect.bottom() {
            for xx in rect.left()..rect.right() {
                if let Some(cell) = buf.cell_mut((xx, yy)) {
                    cell.reset();
                    cell.set_bg(theme::PANEL_BG);
                }
            }
        }

        let alpha = self.text_alpha();
        if alpha <= 0.01 {
            return; // reiner Bg (Aufbau/Collapse) — noch/kein Text
        }
        let title_fg = lerp_color(theme::PANEL_BG, self.accent, alpha);
        let detail_fg = lerp_color(theme::PANEL_BG, theme::TEXT, alpha);

        if self.kind == NotifyKind::Info {
            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", self.title),
                    Style::default().fg(title_fg).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD),
                ),
                Span::styled(self.detail.clone(), Style::default().fg(detail_fg).bg(theme::PANEL_BG)),
            ]);
            Paragraph::new(line)
                .alignment(Alignment::Center)
                .render(rect, buf);
        } else {
            let title = Paragraph::new(Span::styled(
                self.title.clone(),
                Style::default().fg(title_fg).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center);
            let detail = Paragraph::new(Span::styled(
                self.detail.clone(),
                Style::default().fg(detail_fg).bg(theme::PANEL_BG),
            ))
            .alignment(Alignment::Center);
            title.render(Rect { y: rect.y + 1, height: 1, ..rect }, buf);
            detail.render(Rect { y: rect.y + 2, height: 1, ..rect }, buf);
        }
    }
}

/// Vertikaler Stapel schwebender Notifications (oben-mitte). Mischt Größen:
/// jede Notification belegt die Höhe ihres Typs.
#[derive(Default)]
pub struct NotificationStack {
    items: Vec<Notification>,
}

impl NotificationStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stellt eine Notification ein. `detail` darf leer sein. Akzent = Typ-Default.
    pub fn push(&mut self, kind: NotifyKind, title: impl Into<String>, detail: impl Into<String>) {
        self.items.push(Notification {
            kind,
            title: title.into(),
            detail: detail.into(),
            accent: kind.default_accent(),
            age: Duration::ZERO,
        });
    }

    /// Treibt alle Notifications um `dt` weiter und entfernt fertige.
    pub fn advance(&mut self, dt: Duration) {
        for n in &mut self.items {
            n.age += dt;
        }
        self.items.retain(|n| !n.is_done());
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Rendert den Stapel oben-mitte in `area`. Neuste unten (an die jüngere
    /// gehängt), beginnend eine Zeile unter dem oberen Rand.
    pub fn render(&self, buf: &mut Buffer, area: Rect) {
        let mut y = area.top().saturating_add(1);
        for n in &self.items {
            let h = n.kind.height();
            if y + h >= area.bottom() {
                break;
            }
            n.render(buf, y, area);
            y = y.saturating_add(h + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_drives_height() {
        assert_eq!(NotifyKind::Info.height(), 1);
        assert_eq!(NotifyKind::Event.height(), 3);
        assert_eq!(NotifyKind::Major.height(), 3);
    }

    #[test]
    fn lifecycle_progresses_and_completes() {
        let mut s = NotificationStack::new();
        s.push(NotifyKind::Info, "TURNED", "Up");
        assert!(!s.is_empty());

        // Direkt nach Erzeugung: Aufbau, noch kein Text.
        let n = &s.items[0];
        assert!(n.width_factor() < 1.0);
        assert_eq!(n.text_alpha(), 0.0);

        // Mitten im Leben: voll + Text sichtbar.
        s.advance(BUILD + TEXT + Duration::from_millis(10));
        let n = &s.items[0];
        assert_eq!(n.width_factor(), 1.0);
        assert!(n.text_alpha() > 0.99);

        // Im Collapse: schrumpft wieder, Text weg.
        s.advance(HOLD + Duration::from_millis(10));
        let n = &s.items[0];
        assert!(n.width_factor() < 1.0);
        assert_eq!(n.text_alpha(), 0.0);

        // Nach Gesamtdauer: entfernt.
        s.advance(COLLAPSE);
        assert!(s.is_empty());
    }

    #[test]
    fn mixed_kinds_stack_together() {
        let mut s = NotificationStack::new();
        s.push(NotifyKind::Info, "TURNED", "Up");
        s.push(NotifyKind::Event, "PICKUP", "dash");
        s.push(NotifyKind::Major, "MERGED", "main is green");
        assert_eq!(s.items.len(), 3);
        // Render gegen einen Scratch-Buffer darf nicht paniken (gemischte Höhen).
        let area = Rect::new(0, 0, 60, 24);
        let mut buf = Buffer::empty(area);
        s.advance(BUILD + TEXT);
        s.render(&mut buf, area);
    }

    #[test]
    fn text_becomes_visible_after_build() {
        let mut s = NotificationStack::new();
        s.push(NotifyKind::Event, "PICKUP", "dash");
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        s.advance(BUILD + TEXT); // voll aufgebaut + Text da
        s.render(&mut buf, area);
        let dump: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(dump.contains("PICKUP"), "Titel sollte sichtbar sein: {dump:?}");
    }
}
