//! `examples/hud_lab.rs` — Visueller Companion (wegwerfbar) für Issue #39.
//!
//!     cargo run --example hud_lab
//!
//! Eigenständiger Build, **null Einfluss aufs Hauptspiel**. Exploriert das
//! frameless HUD/Overlay-Konzept und vergleicht **A/B**, wie die dynamischen
//! Notifications am hochwertigsten reinkommen:
//!
//!   * **Render-Modus** (Taste `m`): `manual` (Geometrie + Farb-Lerp, kein
//!     tachyonfx) · `hybrid` (Geometrie-Panel + tachyonfx-Text-Reveal) ·
//!     `full-fx` (tachyonfx `expand`-Panel mit Block-Charakter + tachyonfx-Text)
//!   * **Text-Reveal** (Taste `i`, für hybrid/full-fx): `coalesce` ·
//!     `sweep+glow` · `fade`
//!   * **Typ-getriebenes Stacking**: `n` feuert abwechselnd Info (1 Zeile) /
//!     Event / Major (3-Zeilen-Karten) — gemischte Größen stapeln zusammen.
//!   * 3 Frameless-Layouts (`1`/`2`/`3`), Inventar-Demo (`v`), Frames an/aus (`f`).
//!
//! Der Ausblend (center-in Collapse: Panel + Text ziehen zur Mitte und geben die
//! Welt frei) ist über alle Modi gleich — das ist bereits die gewählte Signatur.
//! Verglichen wird nur Panel-Aufbau + Text-Enthüllung. Was gewinnt, wird in
//! `src/hud/` live verdrahtet.

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
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
    Frame, Terminal,
};
use tachyonfx::fx::{self, ExpandDirection};
use tachyonfx::{Effect, Interpolation, Motion};

use prfh::hud::{anchor_rect, Anchor};
use prfh::theme;

// ───────────────────────────── Notification-Modell ──────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum RenderMode {
    Manual,
    Hybrid,
    FullFx,
}
impl RenderMode {
    fn label(self) -> &'static str {
        match self {
            RenderMode::Manual => "manual",
            RenderMode::Hybrid => "hybrid",
            RenderMode::FullFx => "full-fx",
        }
    }
    fn next(self) -> Self {
        match self {
            RenderMode::Manual => RenderMode::Hybrid,
            RenderMode::Hybrid => RenderMode::FullFx,
            RenderMode::FullFx => RenderMode::Manual,
        }
    }
    fn uses_fx_text(self) -> bool {
        !matches!(self, RenderMode::Manual)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Reveal {
    Coalesce,
    SweepGlow,
    Fade,
}
impl Reveal {
    fn label(self) -> &'static str {
        match self {
            Reveal::Coalesce => "coalesce",
            Reveal::SweepGlow => "sweep+glow",
            Reveal::Fade => "fade",
        }
    }
    fn next(self) -> Self {
        match self {
            Reveal::Coalesce => Reveal::SweepGlow,
            Reveal::SweepGlow => Reveal::Fade,
            Reveal::Fade => Reveal::Coalesce,
        }
    }
    fn effect(self, ms: u32) -> Effect {
        match self {
            Reveal::Coalesce => fx::coalesce((ms, Interpolation::SineOut)),
            Reveal::SweepGlow => fx::parallel(&[
                fx::sweep_in(Motion::LeftToRight, 10, 0, theme::PANEL_BG, (ms, Interpolation::SineOut)),
                fx::hsl_shift(Some([0.0, 0.0, 35.0]), None, (ms, Interpolation::SineOut)),
            ]),
            Reveal::Fade => fx::fade_from(theme::PANEL_BG, theme::PANEL_BG, (ms, Interpolation::SineOut)),
        }
    }
}

/// Cursor-Stil (der „Schreibkopf" des Spielers) — alle in Akzentfarbe.
#[derive(Clone, Copy, PartialEq)]
enum CursorStyle {
    /// Gefülltes Akzent-Kästchen mit dunklem Richtungs-Dreieck (solide, klar).
    Block,
    /// Schlankes Richtungs-Chevron in Akzent-fg, transparent (diegetisch).
    Chevron,
    /// Chevron mit sanftem Luminanz-Atmen (lebendig).
    Pulse,
    /// Heller Kopf + gedimmte Akzent-Schleppe dahinter (Bewegungsgefühl).
    Comet,
}
impl CursorStyle {
    fn label(self) -> &'static str {
        match self {
            CursorStyle::Block => "block",
            CursorStyle::Chevron => "chevron",
            CursorStyle::Pulse => "pulse",
            CursorStyle::Comet => "comet",
        }
    }
    fn next(self) -> Self {
        match self {
            CursorStyle::Block => CursorStyle::Chevron,
            CursorStyle::Chevron => CursorStyle::Pulse,
            CursorStyle::Pulse => CursorStyle::Comet,
            CursorStyle::Comet => CursorStyle::Block,
        }
    }
}

/// Richtungspfeil → gefülltes Dreieck + Schleppen-Offset (entgegen Laufrichtung).
fn dir_glyph(dir: char) -> (char, (i32, i32)) {
    match dir {
        '↑' => ('▲', (0, 1)),
        '↓' => ('▼', (0, -1)),
        '←' => ('◀', (1, 0)),
        _ => ('▶', (-1, 0)),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Info,  // 1 Zeile, häufig
    Event, // 3-Zeilen-Karte
    Major, // 3-Zeilen-Karte, kräftiger
}
impl Kind {
    fn height(self) -> u16 {
        match self {
            Kind::Info => 1,
            _ => 2, // Titel + Detail, keine leere Kopfzeile
        }
    }
}

// Phasen-Tempo (gleich über alle Modi, außer Panel-Aufbau-Technik).
const BUILD: Duration = Duration::from_millis(240);
const TEXT: Duration = Duration::from_millis(260);
const HOLD: Duration = Duration::from_millis(1500);
const COLLAPSE: Duration = Duration::from_millis(140);
fn life() -> Duration {
    BUILD + TEXT + HOLD + COLLAPSE
}

fn lerp(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (ar, ag, ab) = rgb(a);
    let (br, bg, bb) = rgb(b);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(m(ar, br), m(ag, bg), m(ab, bb))
}
fn rgb(c: Color) -> (u8, u8, u8) {
    if let Color::Rgb(r, g, b) = c {
        (r, g, b)
    } else {
        (0, 0, 0)
    }
}

struct Notif {
    kind: Kind,
    title: String,
    detail: String,
    accent: Color,
    age: Duration,
    text_fx: Option<Effect>,  // tachyonfx-Reveal (hybrid/full-fx), lazy ab Text-Phase
    panel_fx: Option<Effect>, // tachyonfx expand-Panel (nur full-fx), lazy
    fx_inited: bool,
}

impl Notif {
    fn new(kind: Kind, title: &str, detail: &str, accent: Color) -> Self {
        Self {
            kind,
            title: title.into(),
            detail: detail.into(),
            accent,
            age: Duration::ZERO,
            text_fx: None,
            panel_fx: None,
            fx_inited: false,
        }
    }

    fn done(&self) -> bool {
        self.age >= life()
    }

    /// Breiten-Faktor des Panels: center-out Aufbau, voll, center-in Collapse.
    fn width_factor(&self) -> f32 {
        let a = self.age;
        if a < BUILD {
            a.as_secs_f32() / BUILD.as_secs_f32()
        } else if a < life() - COLLAPSE {
            1.0
        } else {
            (1.0 - (a - (life() - COLLAPSE)).as_secs_f32() / COLLAPSE.as_secs_f32()).max(0.0)
        }
    }

    fn in_build(&self) -> bool {
        self.age < BUILD
    }
    fn in_collapse(&self) -> bool {
        self.age >= life() - COLLAPSE
    }
    /// Text sichtbar (nach Aufbau, vor Collapse).
    fn text_visible(&self) -> bool {
        self.age >= BUILD && !self.in_collapse()
    }
    /// Manueller Text-Alpha (nur RenderMode::Manual).
    fn text_alpha(&self) -> f32 {
        let a = self.age;
        if a < BUILD + TEXT {
            (a.saturating_sub(BUILD)).as_secs_f32() / TEXT.as_secs_f32()
        } else {
            1.0
        }
    }

    fn full_width(&self, area: Rect) -> u16 {
        let t = self.title.chars().count() as u16;
        let d = self.detail.chars().count() as u16;
        let w = match self.kind {
            Kind::Info => t + d + 5,
            _ => t.max(d) + 4,
        };
        w.clamp(8, area.width)
    }
}

// ───────────────────────────── Companion-State ──────────────────────────────

struct State {
    scene: u8,
    frames: bool,
    dir: char,
    combo: u32,
    mode: RenderMode,
    reveal: Reveal,
    cursor: CursorStyle,
    notifs: Vec<Notif>,
    seq: usize,
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
            mode: RenderMode::FullFx,
            reveal: Reveal::Coalesce,
            cursor: CursorStyle::Pulse,
            notifs: Vec::new(),
            seq: 0,
            inv_open: false,
            inv_effect: fx::coalesce((1, Interpolation::Linear)),
            inv_closing: false,
            frame: 0,
        }
    }

    fn fire(&mut self) {
        // Verschiedene Typen rotieren → man sieht gemischtes Stacking.
        const S: &[(Kind, &str, &str, Color)] = &[
            (Kind::Info, "⟹  TURNED", "Up", theme::ACCENT),
            (Kind::Event, "✦  PICKUP", "dash ins Inventar", theme::PICKUP_BASE),
            (Kind::Info, "⟹  STOP", "next char overwrites", theme::DANGER),
            (Kind::Major, "✓  MERGED", "main is green", theme::HIGHLIGHT_BG),
            (Kind::Event, "⚡  COMBO x12", "saubere Kette", theme::ACCENT),
        ];
        let (kind, title, detail, accent) = S[self.seq % S.len()];
        self.seq += 1;
        self.notifs.push(Notif::new(kind, title, detail, accent));
    }

    fn toggle_inventory(&mut self) {
        if self.inv_open && !self.inv_closing {
            self.inv_effect = fx::dissolve((400, Interpolation::SineIn));
            self.inv_closing = true;
        } else {
            self.inv_open = true;
            self.inv_closing = false;
            self.inv_effect = fx::coalesce((400, Interpolation::SineOut));
        }
    }

    fn update(&mut self, dt: Duration) {
        self.frame = self.frame.wrapping_add(1);
        for n in &mut self.notifs {
            n.age += dt;
        }
        self.notifs.retain(|n| !n.done());
        if self.inv_closing && self.inv_effect.done() {
            self.inv_open = false;
            self.inv_closing = false;
        }
    }
}

// ───────────────────────────── main / loop ──────────────────────────────────

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
                        KeyCode::Char('n') => state.fire(),
                        KeyCode::Char('m') => state.mode = state.mode.next(),
                        KeyCode::Char('i') => state.reveal = state.reveal.next(),
                        KeyCode::Char('c') => state.cursor = state.cursor.next(),
                        KeyCode::Char('v') => state.toggle_inventory(),
                        KeyCode::Char('f') => state.frames = !state.frames,
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
    draw_world_bg(f.buffer_mut(), area, state.frame);
    match state.scene {
        2 => layout_bottom_strip(f, area, state),
        3 => layout_diegetic(f, area, state),
        _ => layout_corners(f, area, state),
    }
    draw_notifications(f, area, state, dt);
    if state.inv_open {
        draw_inventory(f, area, state, dt);
    }
    draw_help(f, area, state);
}

// ───────────────────────────── Notification-Render ──────────────────────────

fn draw_notifications(f: &mut Frame, area: Rect, state: &mut State, dt: Duration) {
    let mode = state.mode;
    let reveal = state.reveal;
    let mut y = area.top().saturating_add(1);
    for n in state.notifs.iter_mut() {
        let h = n.kind.height();
        if y + h >= area.bottom() {
            break;
        }
        draw_one(f, area, n, y, mode, reveal, dt);
        y = y.saturating_add(h + 1);
    }
}

fn draw_one(
    f: &mut Frame,
    area: Rect,
    n: &mut Notif,
    top: u16,
    mode: RenderMode,
    reveal: Reveal,
    dt: Duration,
) {
    let h = n.kind.height();
    let full_w = n.full_width(area);
    let factor = n.width_factor();
    if factor <= 0.01 {
        return;
    }

    // full-fx: Panel-Aufbau über tachyonfx expand (Block-Charakter, ehrlich
    // sichtbar). Sonst manuelle center-out-Geometrie.
    let fx_panel = mode == RenderMode::FullFx && n.in_build();
    let w = if fx_panel {
        full_w // expand füllt selbst von der Mitte
    } else {
        ((full_w as f32 * factor).round() as u16).clamp(1, full_w)
    };
    let x = area.left() + area.width.saturating_sub(w) / 2;
    let rect = Rect { x, y: top, width: w, height: h };

    // Panel-Hintergrund.
    paint_panel(f.buffer_mut(), rect);

    if fx_panel {
        // expand-Effekt lazy anlegen, über das volle Rect prozessieren.
        if n.panel_fx.is_none() {
            n.panel_fx = Some(fx::expand(
                ExpandDirection::Horizontal,
                Style::default().bg(theme::PANEL_BG),
                (BUILD.as_millis() as u32, Interpolation::CircOut),
            ));
        }
        if let Some(e) = n.panel_fx.as_mut() {
            e.process(dt.into(), f.buffer_mut(), Rect { x: area.left() + area.width.saturating_sub(full_w) / 2, y: top, width: full_w, height: h });
        }
        return; // während Aufbau noch kein Text
    }

    if n.in_build() {
        return; // manueller Aufbau: nur Bg, kein Text
    }

    // ── Text ──
    if !n.text_visible() {
        return; // im Collapse: nur schrumpfendes Bg, kein Text
    }

    if mode.uses_fx_text() {
        // Vollfarbigen Text setzen, dann tachyonfx-Reveal drüber laufen lassen.
        render_text(f.buffer_mut(), rect, n, 1.0);
        if !n.fx_inited {
            n.text_fx = Some(reveal.effect(TEXT.as_millis() as u32));
            n.fx_inited = true;
        }
        if let Some(e) = n.text_fx.as_mut() {
            if !e.done() {
                e.process(dt.into(), f.buffer_mut(), rect);
            }
        }
    } else {
        // manuell: Farb-Lerp aus dem Panel-Grau.
        render_text(f.buffer_mut(), rect, n, n.text_alpha());
    }
}

fn paint_panel(buf: &mut Buffer, rect: Rect) {
    for yy in rect.top()..rect.bottom() {
        for xx in rect.left()..rect.right() {
            if let Some(cell) = buf.cell_mut((xx, yy)) {
                cell.reset();
                cell.set_bg(theme::PANEL_BG);
            }
        }
    }
}

/// Zeichnet Titel+Detail mittig. `alpha` blendet aus dem Panel-Grau (für den
/// manuellen Modus); bei 1.0 voll farbig (fx-Modi setzen so + animieren drüber).
fn render_text(buf: &mut Buffer, rect: Rect, n: &Notif, alpha: f32) {
    let title_fg = lerp(theme::PANEL_BG, n.accent, alpha);
    let detail_fg = lerp(theme::PANEL_BG, theme::TEXT, alpha);
    if n.kind == Kind::Info {
        let line = Line::from(vec![
            Span::styled(
                format!("{} ", n.title),
                Style::default().fg(title_fg).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD),
            ),
            Span::styled(n.detail.clone(), Style::default().fg(detail_fg).bg(theme::PANEL_BG)),
        ]);
        Paragraph::new(line).alignment(Alignment::Center).render(rect, buf);
    } else {
        Paragraph::new(Span::styled(
            n.title.clone(),
            Style::default().fg(title_fg).bg(theme::PANEL_BG).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .render(Rect { y: rect.y, height: 1, ..rect }, buf);
        Paragraph::new(Span::styled(
            n.detail.clone(),
            Style::default().fg(detail_fg).bg(theme::PANEL_BG),
        ))
        .alignment(Alignment::Center)
        .render(Rect { y: rect.y + 1, height: 1, ..rect }, buf);
    }
}

// ───────────────────────────── Welt + Layouts ───────────────────────────────

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
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                let g = GLYPHS[(h / 6) as usize % GLYPHS.len()];
                let v = 0x34 + ((h >> 5) % 0x18) as u8;
                cell.set_char(g).set_fg(Color::Rgb(v, v, v + 6));
            }
        }
    }
}

fn layout_corners(f: &mut Frame, area: Rect, state: &State) {
    chip(f, anchor_rect(area, Anchor::TopLeft, 9, 1), "dir", &state.dir.to_string(), state.frames);
    chip(f, anchor_rect(area, Anchor::TopRight, 12, 1), "combo", &format!("x{}", state.combo), state.frames);
    cursor_marker(f, area, state.dir, state.cursor, state.frame);
    players_strip(f, anchor_rect(area, Anchor::BottomLeft, 30, 1));
}

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
    f.render_widget(Paragraph::new(line).style(Style::default().bg(theme::PANEL_BG)), rect);
    cursor_marker(f, area, state.dir, state.cursor, state.frame);
}

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
        f.render_widget(
            Paragraph::new(Span::styled(format!("x{}", state.combo), Style::default().fg(theme::TEXT_DIM))),
            Rect { x: area.left() + area.width / 2 - 3, y: area.bottom().saturating_sub(1), width: 6, height: 1 },
        );
    }
}

fn cursor_marker(f: &mut Frame, area: Rect, dir: char, style: CursorStyle, frame: u64) {
    let cx = area.left() + area.width / 2;
    let cy = area.top() + area.height / 2;
    let (glyph, (bx, by)) = dir_glyph(dir);
    let buf = f.buffer_mut();

    // Sanftes Atmen 0.55..1.0 (nur Pulse): dimmt die Akzent-Luminanz.
    let pulse = if style == CursorStyle::Pulse {
        0.78 + 0.22 * (frame as f32 * 0.18).sin()
    } else {
        1.0
    };
    let accent_bright = lerp(theme::PANEL_BG, theme::ACCENT, pulse);
    let accent_dim = lerp(Color::Rgb(0x14, 0x10, 0x12), theme::ACCENT, 0.45);

    match style {
        CursorStyle::Block => {
            if let Some(cell) = buf.cell_mut((cx, cy)) {
                cell.set_char(glyph)
                    .set_fg(theme::HIGHLIGHT_FG)
                    .set_bg(theme::ACCENT)
                    .set_style(Style::default().add_modifier(Modifier::BOLD));
            }
        }
        CursorStyle::Chevron | CursorStyle::Pulse => {
            if let Some(cell) = buf.cell_mut((cx, cy)) {
                cell.set_char(glyph)
                    .set_fg(accent_bright)
                    .set_style(Style::default().add_modifier(Modifier::BOLD));
            }
        }
        CursorStyle::Comet => {
            // gedimmte Schleppe hinter dem Kopf (entgegen der Laufrichtung)
            let tx = cx as i32 + bx;
            let ty = cy as i32 + by;
            if tx >= area.left() as i32 && ty >= area.top() as i32 {
                if let Some(cell) = buf.cell_mut((tx as u16, ty as u16)) {
                    cell.set_char('•').set_fg(accent_dim);
                }
            }
            if let Some(cell) = buf.cell_mut((cx, cy)) {
                cell.set_char(glyph)
                    .set_fg(theme::ACCENT)
                    .set_style(Style::default().add_modifier(Modifier::BOLD));
            }
        }
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

fn players_strip(f: &mut Frame, rect: Rect) {
    let line = Line::from(vec![
        Span::styled("you", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("(du) ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled("rival ", Style::default().fg(Color::Rgb(0x8A, 0xE2, 0x34)).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line), rect);
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
        2 => "2:strip",
        3 => "3:diegetic",
        _ => "1:corners",
    };
    let reveal_dim = if state.mode.uses_fx_text() { theme::TEXT } else { theme::TEXT_DIM };
    let line = Line::from(vec![
        Span::styled(" hud_lab ", Style::default().fg(Color::Black).bg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {scene} "), Style::default().fg(theme::ACCENT)),
        Span::styled("│ n notif · ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(format!("m mode:{} ", state.mode.label()), Style::default().fg(theme::HIGHLIGHT_BG).add_modifier(Modifier::BOLD)),
        Span::styled(format!("i reveal:{} ", state.reveal.label()), Style::default().fg(reveal_dim)),
        Span::styled(format!("c cursor:{} ", state.cursor.label()), Style::default().fg(theme::ACCENT)),
        Span::styled("· 1/2/3 · v inv · f frames · q", Style::default().fg(theme::TEXT_DIM)),
    ]);
    let rect = Rect { x: area.left(), y: area.bottom().saturating_sub(1), width: area.width, height: 1 };
    f.render_widget(Paragraph::new(line).style(Style::default().bg(Color::Rgb(0x18, 0x18, 0x1C))), rect);
}
