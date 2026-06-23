//! Dynamische Quick-Notifications: schweben oben-mitte über der full-screen-Welt.
//!
//! Gewählte Signatur (validiert im `hud_lab`-Companion):
//!   * **Rein:** Panel erscheint, dazu eine horizontale `expand`-Welle aus der
//!     Mitte ([`effects::notify_panel`]); danach **sammelt sich der Text** per
//!     `coalesce` ([`effects::notify_reveal`]).
//!   * **Halten:** kurze Standzeit.
//!   * **Raus:** center-in Collapse — Panel **und** Text ziehen sich zur Mitte
//!     zusammen und geben die Welt darunter wieder frei (reine Geometrie).
//!
//! Typ-getrieben ([`NotifyKind`]): `Info` ist eine kompakte Zeile, `Event`/
//! `Major` sind 2-zeilige Karten (Titel + Detail). Gemischte Größen stapeln.
//!
//! Die Lebenszyklus-Phasen sind eine reine Funktion des Alters und damit
//! unit-testbar. Die tachyonfx-Effekte (Panel-Welle, Text-Coalesce) werden
//! pro Notification gehalten und beim Rendern mit der Frame-`elapsed`-Dauer
//! prozessiert — die Notifications selbst sind der frame-persistente State.

use crate::effects;
use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::time::Duration;
use tachyonfx::Effect;

const BUILD: Duration = Duration::from_millis(240); // Panel-Welle
const TEXT: Duration = Duration::from_millis(260); // Text sammelt sich
const HOLD: Duration = Duration::from_millis(1500); // Standzeit
const COLLAPSE: Duration = Duration::from_millis(140); // center-in Abbau

fn life() -> Duration {
    BUILD + TEXT + HOLD + COLLAPSE
}

/// Notification-Typ — steuert Größe (Stack mischt Größen) und Default-Akzent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyKind {
    /// Häufige, kleine Hinweise (Turn, Stop). Kompakt: eine Zeile.
    Info,
    /// Bemerkenswerte Ereignisse (Pickup, Combo). Karte: zwei Zeilen.
    Event,
    /// Große Momente (Merge, Boss). Karte: zwei Zeilen, kräftiger Akzent.
    Major,
}

impl NotifyKind {
    /// Höhe in Zeilen — kompakt (1) vs. Karte (2: Titel + Detail).
    pub fn height(self) -> u16 {
        match self {
            NotifyKind::Info => 1,
            NotifyKind::Event | NotifyKind::Major => 2,
        }
    }

    fn default_accent(self) -> ratatui::style::Color {
        match self {
            NotifyKind::Info => theme::ACCENT,
            NotifyKind::Event => theme::PICKUP_BASE,
            NotifyKind::Major => theme::HIGHLIGHT_BG,
        }
    }
}

/// Eine einzelne Notification. Visueller Zustand ergibt sich aus `age`; die
/// tachyonfx-Effekte werden lazy in der jeweiligen Phase erzeugt.
pub struct Notification {
    kind: NotifyKind,
    title: String,
    detail: String,
    accent: ratatui::style::Color,
    age: Duration,
    panel_fx: Option<Effect>,
    text_fx: Option<Effect>,
}

impl Notification {
    fn is_done(&self) -> bool {
        self.age >= life()
    }

    fn in_build(&self) -> bool {
        self.age < BUILD
    }

    fn in_collapse(&self) -> bool {
        self.age >= life() - COLLAPSE
    }

    /// Breiten-Faktor 0..1 des Panels beim center-in Collapse (sonst 1.0). Der
    /// Aufbau wird visuell über die `expand`-Welle gemacht, nicht über die Breite.
    fn collapse_factor(&self) -> f32 {
        if self.in_collapse() {
            let into = (self.age - (life() - COLLAPSE)).as_secs_f32();
            (1.0 - into / COLLAPSE.as_secs_f32()).max(0.0)
        } else {
            1.0
        }
    }

    fn full_width(&self, area: Rect) -> u16 {
        // Bei winzigen Terminals (< 8 Spalten) nur die Breite nehmen — `clamp(8,
        // area.width)` würde paniken (min > max). Projekt-Norm: nie paniken.
        if area.width < 8 {
            return area.width;
        }
        let t = self.title.chars().count() as u16;
        let d = self.detail.chars().count() as u16;
        let w = match self.kind {
            NotifyKind::Info => t + d + 5,
            _ => t.max(d) + 4,
        };
        w.clamp(8, area.width)
    }

    /// Zeichnet diese Notification mit oberem Rand `top` und prozessiert ihre
    /// Effekte um `elapsed`.
    fn render(&mut self, buf: &mut Buffer, top: u16, area: Rect, elapsed: Duration) {
        let h = self.kind.height();
        let full_w = self.full_width(area);
        let factor = self.collapse_factor();
        if factor <= 0.01 {
            return;
        }
        let w = ((full_w as f32 * factor).round() as u16).clamp(1, full_w);
        let x = area.left() + area.width.saturating_sub(w) / 2;
        let rect = Rect { x, y: top, width: w, height: h };

        // Panel-Hintergrund: Welt darunter verdecken + Panel-BG füllen.
        for yy in rect.top()..rect.bottom() {
            for xx in rect.left()..rect.right() {
                if let Some(cell) = buf.cell_mut((xx, yy)) {
                    cell.reset();
                    cell.set_bg(theme::PANEL_BG);
                }
            }
        }

        if self.in_build() {
            // Panel-Welle (expand) über das volle Rect; noch kein Text.
            let fx = self
                .panel_fx
                .get_or_insert_with(|| effects::notify_panel(BUILD.as_millis() as u32));
            let full_rect = Rect { x: area.left() + area.width.saturating_sub(full_w) / 2, y: top, width: full_w, height: h };
            fx.process(elapsed.into(), buf, full_rect);
            return;
        }

        if self.in_collapse() {
            return; // schrumpfendes Panel, kein Text
        }

        // Text setzen (voll), dann coalesce-Reveal drüber laufen lassen.
        self.render_text(buf, rect);
        let fx = self
            .text_fx
            .get_or_insert_with(|| effects::notify_reveal(TEXT.as_millis() as u32));
        if !fx.done() {
            fx.process(elapsed.into(), buf, rect);
        }
    }

    fn render_text(&self, buf: &mut Buffer, rect: Rect) {
        let title_style = Style::default().fg(self.accent).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD);
        let detail_style = Style::default().fg(theme::TEXT).bg(theme::PANEL_BG);
        if self.kind == NotifyKind::Info {
            let line = Line::from(vec![
                Span::styled(format!("{} ", self.title), title_style),
                Span::styled(self.detail.clone(), detail_style),
            ]);
            Paragraph::new(line).alignment(Alignment::Center).render(rect, buf);
        } else {
            Paragraph::new(Span::styled(self.title.clone(), title_style))
                .alignment(Alignment::Center)
                .render(Rect { height: 1, ..rect }, buf);
            Paragraph::new(Span::styled(self.detail.clone(), detail_style))
                .alignment(Alignment::Center)
                .render(Rect { y: rect.y + 1, height: 1, ..rect }, buf);
        }
    }
}

/// Vertikaler Stapel schwebender Notifications (oben-mitte). Mischt Größen.
#[derive(Default)]
pub struct NotificationStack {
    items: Vec<Notification>,
}

impl NotificationStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stellt eine Notification ein. `detail` darf leer sein; Akzent = Typ-Default.
    pub fn push(&mut self, kind: NotifyKind, title: impl Into<String>, detail: impl Into<String>) {
        self.items.push(Notification {
            kind,
            title: title.into(),
            detail: detail.into(),
            accent: kind.default_accent(),
            age: Duration::ZERO,
            panel_fx: None,
            text_fx: None,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Treibt alle Notifications um `elapsed` weiter, rendert sie oben-mitte und
    /// entfernt fertige. Neuste unten an die ältere gehängt.
    pub fn render(&mut self, buf: &mut Buffer, area: Rect, elapsed: Duration) {
        for n in &mut self.items {
            n.age += elapsed;
        }
        self.items.retain(|n| !n.is_done());

        let mut y = area.top().saturating_add(1);
        for n in &mut self.items {
            let h = n.kind.height();
            if y + h >= area.bottom() {
                break;
            }
            n.render(buf, y, area, elapsed);
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
        assert_eq!(NotifyKind::Event.height(), 2);
        assert_eq!(NotifyKind::Major.height(), 2);
    }

    #[test]
    fn lifecycle_phases_follow_age() {
        let mut s = NotificationStack::new();
        s.push(NotifyKind::Info, "TURNED", "Up");
        assert!(!s.is_empty());

        // Frisch: Aufbau-Phase.
        assert!(s.items[0].in_build());
        assert!(!s.items[0].in_collapse());
        assert_eq!(s.items[0].collapse_factor(), 1.0);

        // Render treibt das Alter (über einen Scratch-Buffer, darf nicht paniken).
        let area = Rect::new(0, 0, 60, 24);
        let mut buf = Buffer::empty(area);
        s.render(&mut buf, area, BUILD + TEXT + Duration::from_millis(10));
        assert!(!s.items[0].in_build());
        assert!(!s.items[0].in_collapse());

        // In den Collapse: Breite schrumpft.
        s.render(&mut buf, area, HOLD + Duration::from_millis(10));
        assert!(s.items[0].in_collapse());
        assert!(s.items[0].collapse_factor() < 1.0);

        // Nach Gesamtdauer: entfernt.
        s.render(&mut buf, area, COLLAPSE);
        assert!(s.is_empty());
    }

    #[test]
    fn mixed_kinds_stack_and_render_without_panic() {
        let mut s = NotificationStack::new();
        s.push(NotifyKind::Info, "TURNED", "Up");
        s.push(NotifyKind::Event, "PICKUP", "dash");
        s.push(NotifyKind::Major, "MERGED", "main is green");
        assert_eq!(s.items.len(), 3);
        let area = Rect::new(0, 0, 60, 24);
        let mut buf = Buffer::empty(area);
        // Über mehrere Frames bis weit ins Leben — expand-Panik-Regel inklusive.
        for _ in 0..40 {
            s.render(&mut buf, area, Duration::from_millis(50));
        }
    }

    #[test]
    fn tiny_terminal_does_not_panic() {
        // < 8 Spalten: full_width darf nicht über clamp(8, width) paniken.
        let mut s = NotificationStack::new();
        s.push(NotifyKind::Major, "MERGED", "main is green");
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        for _ in 0..20 {
            s.render(&mut buf, area, Duration::from_millis(50));
        }
    }

    #[test]
    fn text_appears_after_build() {
        let mut s = NotificationStack::new();
        s.push(NotifyKind::Event, "PICKUP", "dash");
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        // Bis in die Text-Phase (nach BUILD), aber vor Collapse.
        s.render(&mut buf, area, BUILD + Duration::from_millis(120));
        // coalesce kann einzelne Zellen noch streuen; nach genug Zeit steht der Text.
        s.render(&mut buf, area, TEXT);
        let dump: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(dump.contains("PICKUP"), "Titel sollte sichtbar sein: {dump:?}");
    }
}
