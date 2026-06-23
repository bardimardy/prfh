//! `examples/hud_lab.rs` — Visueller Companion (wegwerfbar) für Issue #39.
//!
//!     cargo run --example hud_lab
//!
//! Eigenständiger Build, **null Einfluss aufs Hauptspiel**. Dient zum Explorieren
//! des frameless HUD/Overlay-Konzepts:
//!   * 3 Frameless-Layout-Vorschläge (Tasten 1/2/3)
//!   * dynamische Notifications oben-mitte mit tachyonfx evolve-in / dissolve-out
//!     (Taste n feuert; i/o zyklen In-/Out-Stil)
//!   * Overlay-Demo: Inventar-Panel poppt rein (Taste v)
//!   * Frames an/aus zum direkten Vergleich (Taste f)
//!
//! Was hier gefällt, wird in den Design-Doc eingefroren und dann erst in `src/`
//! gebaut (wie beim Powerup-Spec). Dieser Companion ist KEINE Spiel-Logik.

use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame, Terminal,
};
use tachyonfx::fx::{self, EvolveSymbolSet};
use tachyonfx::{Effect, Interpolation, Motion};

use prfh::theme;

// ───────────────────────────── Overlay-Framework-Skizze ─────────────────────
// Anker-basierte Platzierung statt hartem Layout — exakt das Muster, das ins
// echte HUD-Framework wandern soll: ein HUD-Teil kennt nur seinen Anker +
// Wunschgröße, der Layer rechnet das Rect über der full-screen-Welt aus.

#[derive(Clone, Copy)]
enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    Center,
}

fn anchor_rect(area: Rect, a: Anchor, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let cx = area.left() + (area.width.saturating_sub(w)) / 2;
    let (x, y) = match a {
        Anchor::TopLeft => (area.left(), area.top()),
        Anchor::TopCenter => (cx, area.top()),
        Anchor::TopRight => (area.right().saturating_sub(w), area.top()),
        Anchor::BottomLeft => (area.left(), area.bottom().saturating_sub(h)),
        Anchor::BottomCenter => (cx, area.bottom().saturating_sub(h)),
        Anchor::Center => (
            cx,
            area.top() + (area.height.saturating_sub(h)) / 2,
        ),
    };
    Rect { x, y, width: w, height: h }
}

// ───────────────────────────── Notification-System ──────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum InStyle {
    Evolve,
    Coalesce,
    Fade,
}
impl InStyle {
    fn label(self) -> &'static str {
        match self {
            InStyle::Evolve => "evolve",
            InStyle::Coalesce => "coalesce",
            InStyle::Fade => "fade",
        }
    }
    fn next(self) -> Self {
        match self {
            InStyle::Evolve => InStyle::Coalesce,
            InStyle::Coalesce => InStyle::Fade,
            InStyle::Fade => InStyle::Evolve,
        }
    }
    fn effect(self) -> Effect {
        match self {
            // evolve_from enthüllt am Ende den darunterliegenden (echten) Text.
            InStyle::Evolve => {
                fx::evolve_from(EvolveSymbolSet::Shaded, (650, Interpolation::SineOut))
            }
            InStyle::Coalesce => fx::coalesce((550, Interpolation::SineOut)),
            InStyle::Fade => fx::fade_from(theme::PANEL_BG, theme::PANEL_BG, (450, Interpolation::SineOut)),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum OutStyle {
    DissolveTo,
    Sweep,
    SweepThenDissolve,
    Dissolve,
}
impl OutStyle {
    fn label(self) -> &'static str {
        match self {
            OutStyle::DissolveTo => "dissolve_to",
            OutStyle::Sweep => "sweep_out",
            OutStyle::SweepThenDissolve => "sweep→dissolve",
            OutStyle::Dissolve => "dissolve",
        }
    }
    fn next(self) -> Self {
        match self {
            OutStyle::DissolveTo => OutStyle::Sweep,
            OutStyle::Sweep => OutStyle::SweepThenDissolve,
            OutStyle::SweepThenDissolve => OutStyle::Dissolve,
            OutStyle::Dissolve => OutStyle::DissolveTo,
        }
    }
    fn effect(self) -> Effect {
        let to_panel = Style::default().bg(theme::PANEL_BG);
        match self {
            OutStyle::DissolveTo => fx::dissolve_to(to_panel, (550, Interpolation::SineIn)),
            OutStyle::Sweep => {
                fx::sweep_out(Motion::LeftToRight, 8, 0, theme::PANEL_BG, (550, Interpolation::SineIn))
            }
            OutStyle::SweepThenDissolve => fx::sequence(&[
                fx::sweep_out(Motion::LeftToRight, 6, 0, theme::PANEL_BG, (320, Interpolation::SineIn)),
                fx::dissolve_to(to_panel, (320, Interpolation::SineIn)),
            ]),
            OutStyle::Dissolve => fx::dissolve((550, Interpolation::SineIn)),
        }
    }
}

enum Phase {
    In,
    Hold,
    Out,
}

/// Eine schwebende Quick-Notification: rein-animieren → kurz halten →
/// raus-animieren. Genau die Mechanik, die `App::trigger_banner` ersetzt.
struct Notif {
    title: String,
    detail: String,
    accent: Color,
    phase: Phase,
    effect: Effect,
    out_style: OutStyle,
    hold_left: Duration,
}

impl Notif {
    fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        accent: Color,
        in_style: InStyle,
        out_style: OutStyle,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            accent,
            phase: Phase::In,
            effect: in_style.effect(),
            out_style,
            hold_left: Duration::from_millis(1600),
        }
    }

    /// Phasenfortschritt. `done()` spiegelt den letzten verarbeiteten Frame —
    /// ein Frame Versatz ist unsichtbar. Gibt true zurück, wenn die Notification
    /// fertig (entfernbar) ist.
    fn update(&mut self, dt: Duration) -> bool {
        match self.phase {
            Phase::In => {
                if self.effect.done() {
                    self.phase = Phase::Hold;
                }
            }
            Phase::Hold => {
                self.hold_left = self.hold_left.saturating_sub(dt);
                if self.hold_left.is_zero() {
                    self.effect = self.out_style.effect();
                    self.phase = Phase::Out;
                }
            }
            Phase::Out => {
                if self.effect.done() {
                    return true;
                }
            }
        }
        false
    }
}

// ───────────────────────────── App-State (Companion) ────────────────────────

struct State {
    scene: u8, // 1..=3 Layout-Vorschlag
    frames: bool,
    dir: char,
    combo: u32,
    in_style: InStyle,
    out_style: OutStyle,
    notifs: Vec<Notif>,
    notif_seq: usize,
    notif_card: bool, // true = 3-Zeilen-Karte, false = 1-Zeile kompakt
    inv_open: bool,
    inv_effect: Effect,
    inv_closing: bool,
    frame: u64,
}

impl State {
    fn new() -> Self {
        Self {
            scene: 1,
            frames: false,
            dir: '→',
            combo: 7,
            in_style: InStyle::Evolve,
            out_style: OutStyle::DissolveTo,
            notifs: Vec::new(),
            notif_seq: 0,
            notif_card: true,
            inv_open: false,
            inv_effect: fx::coalesce((1, Interpolation::Linear)),
            inv_closing: false,
            frame: 0,
        }
    }

    fn fire_notif(&mut self) {
        const SAMPLES: &[(&str, &str, Color)] = &[
            ("⟹  TURNED", "Richtung: Up", theme::ACCENT),
            ("⟹  STOP", "nächstes Zeichen überschreibt", theme::DANGER),
            ("✦  PICKUP", "dash ins Inventar gewandert", theme::PICKUP_BASE),
            ("⚡  COMBO x12", "saubere Kette — weiter so", theme::HIGHLIGHT_BG),
            ("✓  MERGED", "main is green", theme::ACCENT),
        ];
        let (title, detail, accent) = SAMPLES[self.notif_seq % SAMPLES.len()];
        self.notif_seq += 1;
        self.notifs
            .push(Notif::new(title, detail, accent, self.in_style, self.out_style));
    }

    fn toggle_inventory(&mut self) {
        if self.inv_open && !self.inv_closing {
            // schließen → dissolve
            self.inv_effect = fx::dissolve((400, Interpolation::SineIn));
            self.inv_closing = true;
        } else {
            self.inv_open = true;
            self.inv_closing = false;
            self.inv_effect = self.in_style.effect();
        }
    }

    fn update(&mut self, dt: Duration) {
        self.frame = self.frame.wrapping_add(1);
        let mut i = 0;
        while i < self.notifs.len() {
            if self.notifs[i].update(dt) {
                self.notifs.remove(i);
            } else {
                i += 1;
            }
        }
        if self.inv_closing && self.inv_effect.done() {
            self.inv_open = false;
            self.inv_closing = false;
        }
    }
}

// ───────────────────────────── Rendering ────────────────────────────────────

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = State::new();
    let mut last = Instant::now();
    let mut quit = false;

    while !quit {
        let dt = last.elapsed();
        last = Instant::now();
        state.update(dt);

        terminal.draw(|f| ui(f, &mut state, dt))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => quit = true,
                        KeyCode::Char('1') => state.scene = 1,
                        KeyCode::Char('2') => state.scene = 2,
                        KeyCode::Char('3') => state.scene = 3,
                        KeyCode::Char('n') => state.fire_notif(),
                        KeyCode::Char('i') => state.in_style = state.in_style.next(),
                        KeyCode::Char('o') => state.out_style = state.out_style.next(),
                        KeyCode::Char('v') => state.toggle_inventory(),
                        KeyCode::Char('b') => state.notif_card = !state.notif_card,
                        KeyCode::Char('f') => state.frames = !state.frames,
                        KeyCode::Char('c') => state.combo += 1,
                        KeyCode::Up => state.dir = '↑',
                        KeyCode::Down => state.dir = '↓',
                        KeyCode::Left => state.dir = '←',
                        KeyCode::Right => state.dir = '→',
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn ui(f: &mut Frame, state: &mut State, dt: Duration) {
    let area = f.area();

    // 1) full-screen „Welt" als scrollender Code-Hintergrund.
    draw_world_bg(f.buffer_mut(), area, state.frame);

    // 2) HUD-Chrome je nach Layout-Vorschlag.
    match state.scene {
        2 => layout_bottom_strip(f, area, state),
        3 => layout_diegetic(f, area, state),
        _ => layout_corners(f, area, state),
    }

    // 3) Notification-Stack (oben-mitte), schwebt über der Welt.
    draw_notifications(f, area, state, dt);

    // 4) Inventar-Overlay-Demo (Center).
    if state.inv_open {
        draw_inventory(f, area, state, dt);
    }

    // 5) Companion-Steuerleiste (nur Companion, nicht Teil des Konzepts).
    draw_help(f, area, state);
}

/// Scrollender, dezenter Code-Teppich, damit man Overlays *über bewegtem Inhalt*
/// schweben sieht. Rein deterministisch aus (x, y, frame).
fn draw_world_bg(buf: &mut Buffer, area: Rect, frame: u64) {
    const GLYPHS: &[char] = &[
        'a', 'b', 'c', 'd', 'e', 'f', 'i', 'l', 'n', 'o', 'r', 's', 't', '(', ')', '{', '}', ';',
        '=', '>', '+', '-', '_', '/', '*', '.', ',',
    ];
    let scroll = frame / 2;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let hx = (x as u64).wrapping_add(scroll);
            let h = hx
                .wrapping_mul(2_654_435_761)
                .wrapping_add((y as u64).wrapping_mul(40_503));
            if !h.is_multiple_of(6) {
                continue; // dünn besät
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                let g = GLYPHS[(h / 6) as usize % GLYPHS.len()];
                let v = 0x34 + ((h >> 5) % 0x18) as u8;
                cell.set_char(g).set_fg(Color::Rgb(v, v, v + 6));
            }
        }
    }
}

// ── Layout 1: Ecken-HUD ──────────────────────────────────────────────────────
fn layout_corners(f: &mut Frame, area: Rect, state: &State) {
    chip(f, anchor_rect(area, Anchor::TopLeft, 9, 1), "dir", &state.dir.to_string(), state.frames);
    chip(
        f,
        anchor_rect(area, Anchor::TopRight, 12, 1),
        "combo",
        &format!("x{}", state.combo),
        state.frames,
    );
    // Cursor-Marker mittig (repräsentiert den Spieler in der full-screen-Welt).
    let c = anchor_rect(area, Anchor::Center, 1, 1);
    f.render_widget(
        Paragraph::new(Span::styled(
            state.dir.to_string(),
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        c,
    );
    players_strip(f, anchor_rect(area, Anchor::BottomLeft, 30, 1), state.frames);
}

// ── Layout 2: Bottom-Status-Strip ────────────────────────────────────────────
fn layout_bottom_strip(f: &mut Frame, area: Rect, state: &State) {
    let rect = anchor_rect(area, Anchor::BottomCenter, area.width.min(60), 1);
    let line = Line::from(vec![
        Span::styled(" dir ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(format!("{} ", state.dir), Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("  combo ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(format!("x{} ", state.combo), Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD)),
        Span::styled("  you", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("(du) rival ", Style::default().fg(theme::TEXT_DIM)),
    ]);
    let p = Paragraph::new(line).style(Style::default().bg(theme::PANEL_BG));
    f.render_widget(p, rect);
    let c = anchor_rect(area, Anchor::Center, 1, 1);
    f.render_widget(
        Paragraph::new(Span::styled(
            state.dir.to_string(),
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        c,
    );
}

// ── Layout 3: minimal / diegetisch ───────────────────────────────────────────
// Kein Chrome — nur ein schwebender Akzent am Cursor + winziger combo-Tick.
fn layout_diegetic(f: &mut Frame, area: Rect, state: &State) {
    let c = anchor_rect(area, Anchor::Center, 3, 1);
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(" {} ", state.dir),
            Style::default().fg(Color::Black).bg(theme::ACCENT).add_modifier(Modifier::BOLD),
        )])),
        c,
    );
    if state.combo > 1 {
        let tick = anchor_rect(area, Anchor::TopCenter, 6, 1);
        let tick = Rect { y: tick.y, ..tick };
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("x{}", state.combo),
                Style::default().fg(theme::TEXT_DIM),
            )),
            Rect { x: tick.x, y: area.bottom().saturating_sub(1), width: 6, height: 1 },
        );
    }
}

fn chip(f: &mut Frame, rect: Rect, key: &str, val: &str, frames: bool) {
    let inner = if frames {
        let b = Block::default().borders(Borders::ALL).style(Style::default().fg(theme::TEXT_DIM));
        let inner = b.inner(rect);
        f.render_widget(b, rect);
        inner
    } else {
        rect
    };
    let line = Line::from(vec![
        Span::styled(format!("{key} "), Style::default().fg(theme::TEXT_DIM)),
        Span::styled(val.to_string(), Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line), inner);
}

fn players_strip(f: &mut Frame, rect: Rect, _frames: bool) {
    let line = Line::from(vec![
        Span::styled("you", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("(du) ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled("rival ", Style::default().fg(Color::Rgb(0x8A, 0xE2, 0x34)).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line), rect);
}

/// Notification-Stack: oben-mitte, neuste oben, vertikal gestapelt. Jede
/// Notification rendert erst ihren echten Text, dann läuft ihr Effekt über
/// genau ihr Rect (per-Notification `Effect::process`).
fn draw_notifications(f: &mut Frame, area: Rect, state: &mut State, dt: Duration) {
    let card = state.notif_card;
    let h: u16 = if card { 3 } else { 1 };
    let mut y = area.top() + 1;
    for n in state.notifs.iter_mut() {
        let content_w = n.title.chars().count().max(n.detail.chars().count()) as u16;
        // Karte: Akzent-Balken (1) + Inset (1) + Inhalt + Inset (1).
        let w = if card {
            (content_w + 4).clamp(16, area.width)
        } else {
            (n.title.chars().count() as u16 + n.detail.chars().count() as u16 + 5).min(area.width)
        };
        let x = area.left() + (area.width.saturating_sub(w)) / 2;
        let rect = Rect { x, y, width: w, height: h };

        if card {
            // Solider Panel-Block (frameless) + vertikaler Akzent-Balken links.
            f.render_widget(Clear, rect);
            let bg = Paragraph::new(vec![Line::from(""); h as usize])
                .style(Style::default().bg(theme::PANEL_BG));
            f.render_widget(bg, rect);
            let bar = Rect { width: 1, ..rect };
            f.render_widget(
                Paragraph::new(vec![Line::from("▌"); h as usize]).style(Style::default().fg(n.accent)),
                bar,
            );
            let text_rect = Rect {
                x: rect.x + 2,
                y: rect.y,
                width: rect.width.saturating_sub(3),
                height: h,
            };
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    n.title.clone(),
                    Style::default().fg(n.accent).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    n.detail.clone(),
                    Style::default().fg(theme::TEXT).bg(theme::PANEL_BG),
                )),
            ];
            f.render_widget(Paragraph::new(lines).style(Style::default().bg(theme::PANEL_BG)), text_rect);
        } else {
            let p = Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", n.title),
                    Style::default().fg(n.accent).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{} ", n.detail),
                    Style::default().fg(theme::TEXT).bg(theme::PANEL_BG),
                ),
            ]));
            f.render_widget(p, rect);
        }
        n.effect.process(dt.into(), f.buffer_mut(), rect);

        y = y.saturating_add(h + 1);
        if y >= area.bottom() {
            break;
        }
    }
}

fn draw_inventory(f: &mut Frame, area: Rect, state: &mut State, dt: Duration) {
    let rect = anchor_rect(area, Anchor::Center, 34, 9);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(theme::ACCENT).bg(theme::PANEL_BG))
        .title(" inventory ");
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let rows = [
        ("dash", "schärft Richtungs-Sprint"),
        ("revert", "macht letzten Zug rückgängig"),
        ("squash", "fasst Trail zusammen"),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .map(|(name, desc)| {
            Line::from(vec![
                Span::styled(format!(" {name:<8}"), Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD)),
                Span::styled((*desc).to_string(), Style::default().fg(theme::TEXT_DIM)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines).style(Style::default().bg(theme::PANEL_BG)), inner);

    state.inv_effect.process(dt.into(), f.buffer_mut(), rect);
}

fn draw_help(f: &mut Frame, area: Rect, state: &State) {
    let scene = match state.scene {
        2 => "2:bottom-strip",
        3 => "3:diegetic",
        _ => "1:corners",
    };
    let line = Line::from(vec![
        Span::styled(" hud_lab ", Style::default().fg(Color::Black).bg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {scene} "), Style::default().fg(theme::ACCENT)),
        Span::styled("│ 1/2/3 layout · n notif · ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(format!("i in:{} ", state.in_style.label()), Style::default().fg(theme::TEXT)),
        Span::styled(format!("o out:{} ", state.out_style.label()), Style::default().fg(theme::TEXT)),
        Span::styled(
            format!("b size:{} ", if state.notif_card { "card" } else { "compact" }),
            Style::default().fg(theme::TEXT),
        ),
        Span::styled("· v inv · f frames · ←↑↓→ dir · q quit", Style::default().fg(theme::TEXT_DIM)),
    ]);
    let rect = Rect { x: area.left(), y: area.bottom().saturating_sub(1), width: area.width, height: 1 };
    f.render_widget(Paragraph::new(line).style(Style::default().bg(Color::Rgb(0x18, 0x18, 0x1C))), rect);
}
