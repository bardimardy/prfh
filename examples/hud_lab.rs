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
use tachyonfx::pattern::RadialPattern;
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

// ───────────────────────────── W2: Powerup-Lab ──────────────────────────────
// Szene 4 exploriert die genuin neuen W2-Visuals: ein noch nicht eingesammeltes
// Mehr-Tile-Powerup-Wort auf der Arena (Orientierung + reversed + Ghost-Styling),
// das Trace-Feedback beim räumlichen Abtippen, die Cast-Aktivierungs-Welle und
// die Cast-Buffer-Anzeige. Was gewinnt, wandert nach draw_world / src/effects/.

/// Das demonstrierte Powerup-Wort (logisches Wort, das der Spieler tippt).
const PWORD: &str = "dash";

/// Leicht blau-weißlicher Grauton einer Luminanz `v` (0..=255).
fn graytone(v: u8) -> Color {
    Color::Rgb(v, v, (v as u16 + 7).min(255) as u8)
}
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)).round() as u8
}

/// HSL→RGB. Für den Rainbow-Cast-Ring: hohe Lightness = helle, pastellige Farben.
fn hsl(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color::Rgb(to(r), to(g), to(b))
}

/// Idle-Styling eines noch nicht getracten Wort-Tiles. Alle animierten Varianten
/// sind **reine Funktionen aus (frame, index)** → scroll-immun. Pflicht, weil das
/// Wort im Spiel cursor-zentriert mitscrollt; ein tachyonfx-Zell-Effekt würde über
/// logisch andere Zeichen schmieren (Skill `effects`, Learning #37). Verschiedene
/// Stile bilden später verschiedene **Powerup-Arten** ab.
#[derive(Clone, Copy, PartialEq)]
enum WordStyle {
    /// Statisch gedämpft (heutiges draw_world) — Referenz zum Vergleich.
    Ghost,
    /// Gray→whitish Gradient-Band, das über das Wort wandert.
    Shimmer,
    /// Gleichmäßiges Luminanz-Atmen des ganzen Worts.
    Pulse,
    /// Heller Kopf scannt über das Wort.
    Scan,
    /// Einzelne Tiles funkeln pseudo-zufällig auf.
    Twinkle,
}
impl WordStyle {
    const ALL: [WordStyle; 5] = [
        WordStyle::Ghost,
        WordStyle::Shimmer,
        WordStyle::Pulse,
        WordStyle::Scan,
        WordStyle::Twinkle,
    ];
    fn label(self) -> &'static str {
        match self {
            WordStyle::Ghost => "ghost",
            WordStyle::Shimmer => "shimmer",
            WordStyle::Pulse => "pulse",
            WordStyle::Scan => "scan",
            WordStyle::Twinkle => "twinkle",
        }
    }
    fn desc(self) -> &'static str {
        match self {
            WordStyle::Ghost => "statisch (Referenz)",
            WordStyle::Shimmer => "gray→white Band",
            WordStyle::Pulse => "Atmen",
            WordStyle::Scan => "Scan-Kopf",
            WordStyle::Twinkle => "Funkeln",
        }
    }
    fn next(self) -> Self {
        match self {
            WordStyle::Ghost => WordStyle::Shimmer,
            WordStyle::Shimmer => WordStyle::Pulse,
            WordStyle::Pulse => WordStyle::Scan,
            WordStyle::Scan => WordStyle::Twinkle,
            WordStyle::Twinkle => WordStyle::Ghost,
        }
    }
    /// Stil eines einzelnen Tiles als reine Funktion von `frame` + Index `i`.
    fn style_at(self, frame: u64, i: usize, n: usize) -> Style {
        match self {
            WordStyle::Ghost => Style::default().fg(theme::TEXT_DIM),
            WordStyle::Shimmer => {
                let t = frame as f32 * 0.14 - i as f32 * 0.95;
                let l = 0.5 + 0.5 * t.sin();
                Style::default()
                    .fg(graytone(lerp_u8(0x55, 0xE6, l)))
                    .add_modifier(Modifier::BOLD)
            }
            WordStyle::Pulse => {
                let l = 0.5 + 0.5 * (frame as f32 * 0.11).sin();
                Style::default().fg(graytone(lerp_u8(0x5A, 0xCC, l)))
            }
            WordStyle::Scan => {
                let span = n as i64 + 3;
                let pos = (frame / 3) as i64 % span;
                let d = (i as i64 - pos).abs();
                let v = match d {
                    0 => 0xF0,
                    1 => 0x9A,
                    _ => 0x5E,
                };
                let mut s = Style::default().fg(graytone(v));
                if d == 0 {
                    s = s.add_modifier(Modifier::BOLD);
                }
                s
            }
            WordStyle::Twinkle => {
                let seed = (frame / 5)
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(i as u64 * 40_503);
                let v = if seed.is_multiple_of(7) { 0xE6 } else { 0x5E };
                Style::default().fg(graytone(v))
            }
        }
    }
}

/// Achse + Platzierungs-Richtung des Worts auf der Map. `reversed` betrifft nur
/// Platzierung/Rendering — getippt wird IMMER das logische Wort (Spec §5).
#[derive(Clone, Copy, PartialEq)]
enum WordAxis {
    Horizontal,
    HorizontalRev,
    Vertical,
    VerticalRev,
}
impl WordAxis {
    fn label(self) -> &'static str {
        match self {
            WordAxis::Horizontal => "horiz",
            WordAxis::HorizontalRev => "horiz-rev",
            WordAxis::Vertical => "vert",
            WordAxis::VerticalRev => "vert-rev",
        }
    }
    fn next(self) -> Self {
        match self {
            WordAxis::Horizontal => WordAxis::HorizontalRev,
            WordAxis::HorizontalRev => WordAxis::Vertical,
            WordAxis::Vertical => WordAxis::VerticalRev,
            WordAxis::VerticalRev => WordAxis::Horizontal,
        }
    }
    /// Einheitsvektor entlang der Achse (Tiles liegen `origin + i*delta`).
    fn delta(self) -> (i32, i32) {
        match self {
            WordAxis::Horizontal | WordAxis::HorizontalRev => (1, 0),
            WordAxis::Vertical | WordAxis::VerticalRev => (0, 1),
        }
    }
    fn reversed(self) -> bool {
        matches!(self, WordAxis::HorizontalRev | WordAxis::VerticalRev)
    }
}

/// Richtungs-Glyph für den Lauf-/Eintritts-Marker.
fn run_glyph(d: (i32, i32)) -> char {
    match d {
        (1, 0) => '▶',
        (-1, 0) => '◀',
        (0, 1) => '▼',
        _ => '▲',
    }
}

/// Cast-Aktivierungs-Burst — ECHTE Welle nach außen (kein wachsendes Rechteck;
/// `expand` ist nur ein Rechteck-Reveal). Vier Konzepte zum Vergleich.
#[derive(Clone, Copy, PartialEq)]
enum CastStyle {
    /// Farb-Schock: Ring aus Helligkeit/Wash rast durch die Welt-Glyphen
    /// (`hsl_shift_fg` + `lighten`, getrieben von `RadialPattern`).
    RadialShock,
    /// Glyph-Schockwelle via `evolve_into`. ⚠️ Blankt nicht-erreichte Zellen auf
    /// ' ' → verdeckt das Spielfeld (Vergleichs-Referenz, NICHT gewählt).
    GlyphRing,
    /// Explosion: Zellen streben nach außen + setzen sich wieder (`explode`).
    Explode,
    /// GEWÄHLT: transparenter Glyph-Ring, render-time gerechnet — berührt nur die
    /// Ring-Bande, Spielfeld bleibt sichtbar; scroll-immun (`draw_ring`).
    ManualRing,
}
impl CastStyle {
    fn label(self) -> &'static str {
        match self {
            CastStyle::RadialShock => "radial",
            CastStyle::GlyphRing => "glyph-ring",
            CastStyle::Explode => "explode",
            CastStyle::ManualRing => "manual-ring",
        }
    }
    fn next(self) -> Self {
        match self {
            CastStyle::RadialShock => CastStyle::GlyphRing,
            CastStyle::GlyphRing => CastStyle::Explode,
            CastStyle::Explode => CastStyle::ManualRing,
            CastStyle::ManualRing => CastStyle::RadialShock,
        }
    }
}

/// Laufende Cast-Welle: entweder ein tachyonfx-Effekt oder der manuelle Ring.
enum Wave {
    Fx(Effect),
    Ring { age: Duration },
}

/// Baut die Welle für den gewählten Cast-Stil. tachyonfx-Bursts laufen <0.5 s →
/// Scroll-Smear vernachlässigbar (Skill `effects`). `expand` wird bewusst gemieden.
fn cast_wave(style: CastStyle) -> Wave {
    match style {
        CastStyle::RadialShock => Wave::Fx(fx::parallel(&[
            fx::hsl_shift_fg([0.0, 40.0, 30.0], (450, Interpolation::QuadOut))
                .with_pattern(RadialPattern::center().with_transition_width(3.0)),
            fx::lighten(None, Some(0.5), (450, Interpolation::QuadOut))
                .with_pattern(RadialPattern::center().with_transition_width(4.0))
                .reversed(),
        ])),
        // Gewählt (W2-Cast): kleiner, dünner Ring, smooth dezeleriert. Die dotty
        // `Circles`-Symbole + schmale `transition_width` halten ihn delikat statt
        // dense; `prolong_end` lässt ihn am Ende sanft ausklingen statt snappen.
        CastStyle::GlyphRing => Wave::Fx(fx::prolong_end(
            (120, Interpolation::SineOut),
            fx::evolve_into(fx::EvolveSymbolSet::Circles, (560, Interpolation::SineOut))
                .with_pattern(RadialPattern::center().with_transition_width(1.4)),
        )),
        CastStyle::Explode => Wave::Fx(fx::sequence(&[
            fx::parallel(&[
                fx::explode(12.0, 2.0, (400, Interpolation::CubicOut)),
                fx::fade_to_fg(theme::PANEL_BG, (400, Interpolation::CubicOut)),
            ]),
            fx::coalesce((150, Interpolation::SineOut)),
        ])),
        CastStyle::ManualRing => Wave::Ring {
            age: Duration::ZERO,
        },
    }
}

/// Transparenter Rainbow-Glyph-Ring (die GEWÄHLTE Cast-Signatur): nur die
/// expandierende Ring-Bande wird gezeichnet — **alle anderen Zellen bleiben
/// unberührt**, das Spielfeld bleibt sichtbar. Reine render-time-Mathematik
/// (dieselbe `sqrt(dx² + 4·dy²)`-Distanz wie `RadialPattern` intern, 2:1-
/// Zellaspekt kompensiert) → über scrollendem Inhalt smear-frei (Skill `effects`,
/// Learning #37). Anders als `evolve_into`, das nicht erreichte Zellen auf ' '
/// blankt. Look: Regenbogen nach Winkel (leicht rotierend), helle/pastellige
/// Farben (hohe Lightness), luftig — dünne Bande + Stipple + leichte Glyphen.
const RING_DUR: f32 = 0.38;
fn draw_ring(buf: &mut Buffer, cx: i32, cy: i32, age: Duration, area: Rect) {
    const MAXR: f32 = 17.0;
    const BAND: f32 = 1.5; // dünn → weniger dense
    let p = (age.as_secs_f32() / RING_DUR).clamp(0.0, 1.0);
    let r = (1.0 - (1.0 - p) * (1.0 - p)) * MAXR; // QuadOut: schnell raus, sanft aus
    let life = 1.0 - p; // Ring blendet zum Ende hin aus
    for y in area.top() as i32..area.bottom() as i32 {
        for x in area.left() as i32..area.right() as i32 {
            let dxf = (x - cx) as f32;
            let dy = (y - cy) as f32 * 2.0;
            let d = (dxf * dxf + dy * dy).sqrt();
            let off = (d - r).abs();
            if off > BAND {
                continue; // transparent: nur die Ring-Bande berühren
            }
            let intensity = (1.0 - off / BAND) * life;
            if intensity < 0.12 {
                continue;
            }
            // Stipple: ~40 % der Bande auslassen → luftiger, nicht dense.
            let hsh = (x as u64)
                .wrapping_mul(2_654_435_761)
                .wrapping_add((y as u64).wrapping_mul(40_503));
            if hsh % 5 < 2 {
                continue;
            }
            // Regenbogen nach Winkel um die Mitte, mit leichter Rotation übers Alter.
            let hue = dy.atan2(dxf).to_degrees() + 360.0 + p * 50.0;
            // Hell/pastellig: hohe Lightness, moderate Sättigung; Kern minimal heller.
            let col = hsl(hue, 0.55, 0.74 + 0.12 * intensity);
            let ch = if intensity > 0.66 { '•' } else { '·' }; // leicht, kein ●
            // Nur fg + Glyph setzen — der Zell-Hintergrund (Spielfeld) bleibt.
            if let Some(cell) = buf.cell_mut((x as u16, y as u16)) {
                cell.set_char(ch).set_fg(col);
            }
        }
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
    // W2-Powerup-Lab (Szene 4)
    word_style: WordStyle,
    word_axis: WordAxis,
    traced: usize, // 0..=PWORD.len(): wie viele Tiles bereits getract sind
    cast_on: bool, // Cast-Buffer-Indikator sichtbar
    cast_style: CastStyle,
    wave: Option<Wave>,
    replay: Duration, // Auto-Replay-Pause seit der letzten Welle (nur Szene 4)
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
            word_style: WordStyle::Shimmer,
            word_axis: WordAxis::Horizontal,
            traced: 0,
            cast_on: false,
            cast_style: CastStyle::ManualRing,
            wave: None,
            replay: Duration::ZERO,
        }
    }

    /// Trace einen Buchstaben weiter (wrappt am Wortende zurück auf 0).
    fn advance_trace(&mut self) {
        let n = PWORD.chars().count();
        self.traced = if self.traced >= n { 0 } else { self.traced + 1 };
    }

    fn fire_wave(&mut self) {
        self.wave = Some(cast_wave(self.cast_style));
        self.replay = Duration::ZERO;
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
        // Manueller Ring altert hier (render-time gerechnet); tachyonfx-Wellen
        // räumt layout_powerup nach dem Prozessieren ab.
        let mut clear = false;
        if let Some(Wave::Ring { age }) = &mut self.wave {
            *age += dt;
            if age.as_secs_f32() > RING_DUR {
                clear = true;
            }
        }
        if clear {
            self.wave = None;
        }
        // Sanfter Auto-Replay in der finalen Cast-Szene: ~1.3 s Pause zwischen
        // Wellen, damit man den Look ohne Tastendruck im Loop sieht.
        if self.scene == 4 {
            if self.wave.is_none() {
                self.replay += dt;
                if self.replay >= Duration::from_millis(850) {
                    self.fire_wave();
                }
            }
        } else {
            self.replay = Duration::ZERO;
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
                        KeyCode::Char('4') => state.scene = 4,
                        KeyCode::Char('5') => state.scene = 5,
                        KeyCode::Char('p') => state.word_style = state.word_style.next(),
                        KeyCode::Char('o') => state.word_axis = state.word_axis.next(),
                        KeyCode::Char('t') => state.advance_trace(),
                        KeyCode::Char('w') => state.fire_wave(),
                        KeyCode::Char('s') => state.cast_style = state.cast_style.next(),
                        KeyCode::Char('b') => state.cast_on = !state.cast_on,
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
        4 => layout_powerup(f, area, state, dt),
        5 => layout_gallery(f, area, state),
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
        // ⚠️ Companion ruft fx::expand direkt (safe_expand ist crate-privat). Die
        // Kurve MUSS Non-Overshoot bleiben (CircOut/QuadOut/SineOut/CubicOut) —
        // Back*/Elastic* paniken am Timer-Ende (siehe Skill `effects`). Im Spiel
        // läuft das über effects::notify_panel/safe_expand.
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

/// Szene 4 — W2-Powerup-Visuals: Wort auf der Map (Orientierung/reversed/Ghost),
/// Trace-Highlight, Eintritts-Marker, Cast-Welle, Cast-Buffer.
fn layout_powerup(f: &mut Frame, area: Rect, state: &mut State, dt: Duration) {
    let chars: Vec<char> = PWORD.chars().collect();
    let n = chars.len();
    let (dx, dy) = state.word_axis.delta();
    let reversed = state.word_axis.reversed();

    let cx = (area.left() + area.width / 2) as i32;
    let cy = (area.top() + area.height / 2) as i32;
    // Wort um die Mitte zentrieren (origin = Tile mit kleinstem Achsen-Wert).
    let ox = cx - dx * (n as i32) / 2;
    let oy = cy - dy * (n as i32) / 2;

    let in_bounds = |px: i32, py: i32| {
        px >= area.left() as i32
            && py >= area.top() as i32
            && px < area.right() as i32
            && py < area.bottom() as i32
    };

    let buf = f.buffer_mut();
    let traced_style = Style::default()
        .fg(theme::HIGHLIGHT_FG)
        .bg(theme::HIGHLIGHT_BG)
        .add_modifier(Modifier::BOLD);

    for i in 0..n {
        let px = ox + dx * i as i32;
        let py = oy + dy * i as i32;
        if !in_bounds(px, py) {
            continue;
        }
        // reversed: Tile i trägt den logischen Buchstaben name[n-1-i].
        let ch = if reversed { chars[n - 1 - i] } else { chars[i] };
        // Highlight folgt der Tipp-Reihenfolge: name[0] zuerst. Bei reversed liegt
        // name[0] am hohen Achsen-Ende → die Trace-Tiles wandern von dort herein.
        let traced = if reversed {
            i >= n - state.traced
        } else {
            i < state.traced
        };
        // Nächstes erwartetes Tile = wo der nächste Buchstabe hin muss
        // (in Tipp-Reihenfolge das Tile direkt hinter dem getracten Block).
        let is_next = if reversed {
            state.traced < n && i == n - 1 - state.traced
        } else {
            state.traced < n && i == state.traced
        };
        let next_style = Style::default()
            .fg(theme::HIGHLIGHT_FG)
            .bg(theme::ACCENT)
            .add_modifier(Modifier::BOLD);
        let style = if is_next {
            next_style
        } else if traced {
            traced_style
        } else {
            state.word_style.style_at(state.frame, i, n)
        };
        if let Some(cell) = buf.cell_mut((px as u16, py as u16)) {
            cell.set_char(ch).set_style(style);
        }
    }

    // Eintritts-Marker: zeigt, wo der Spieler ins Wort läuft (name[0]-Ende).
    // run_dir = Laufrichtung ins Wort; Marker sitzt eine Zelle davor.
    let run_dir = if reversed { (-dx, -dy) } else { (dx, dy) };
    let entry = if reversed {
        (ox + dx * (n as i32 - 1), oy + dy * (n as i32 - 1))
    } else {
        (ox, oy)
    };
    let mx = entry.0 - run_dir.0;
    let my = entry.1 - run_dir.1;
    if in_bounds(mx, my) {
        if let Some(cell) = buf.cell_mut((mx as u16, my as u16)) {
            cell.set_char(run_glyph(run_dir))
                .set_fg(theme::HIGHLIGHT_FG)
                .set_bg(theme::ACCENT)
                .set_style(Style::default().add_modifier(Modifier::BOLD));
        }
    }

    // Cast-Welle. tachyonfx-Effekt über eine zentrierte Region prozessieren;
    // der manuelle Ring wird radial um die Mitte gezeichnet.
    match state.wave.as_mut() {
        Some(Wave::Fx(e)) => {
            // Kompaktes, zentriertes Burst-Feld — kleiner = weniger dense, fokussiert.
            let ww = area.width.min(30);
            let wh = area.height.min(11);
            let wr = Rect {
                x: area.left() + area.width.saturating_sub(ww) / 2,
                y: area.top() + area.height.saturating_sub(wh) / 2,
                width: ww,
                height: wh,
            };
            e.process(dt.into(), f.buffer_mut(), wr);
        }
        Some(Wave::Ring { age }) => {
            let age = *age;
            draw_ring(f.buffer_mut(), cx, cy, age, area);
        }
        None => {}
    }
    // tachyonfx-Welle abräumen, wenn fertig (den Ring räumt update()).
    if matches!(&state.wave, Some(Wave::Fx(e)) if e.done()) {
        state.wave = None;
    }

    if state.cast_on {
        draw_cast_buffer(f, area);
    }
}

/// Cast-Buffer-Indikator: gematchter Prefix im Pink-Kasten, Rest gedämpft —
/// dieselbe Highlight-Signatur wie das Typing-Highlight (Spec §2/§7).
fn draw_cast_buffer(f: &mut Frame, area: Rect) {
    let typed = "das"; // gematchter Prefix von "dash"
    let rest = "h";
    let w: u16 = 26;
    let rect = Rect {
        x: area.left() + area.width.saturating_sub(w) / 2,
        y: area.bottom().saturating_sub(3),
        width: w,
        height: 1,
    };
    let line = Line::from(vec![
        Span::styled(
            " cast ▸ ",
            Style::default()
                .fg(theme::ACCENT)
                .bg(theme::PANEL_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            typed,
            Style::default()
                .fg(theme::HIGHLIGHT_FG)
                .bg(theme::HIGHLIGHT_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(rest, Style::default().fg(theme::TEXT_DIM).bg(theme::PANEL_BG)),
        Span::styled(" ", Style::default().bg(theme::PANEL_BG)),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::PANEL_BG)),
        rect,
    );
}

/// Szene 5 — Galerie: alle animierten Idle-Styles gleichzeitig & live nebeneinander,
/// damit man sie direkt vergleicht (verschiedene Stile = spätere Powerup-Arten).
fn layout_gallery(f: &mut Frame, area: Rect, state: &State) {
    let frame = state.frame;
    let chars: Vec<char> = PWORD.chars().collect();
    let n = chars.len();
    let x0 = area.left() + (area.width / 2).saturating_sub(14);
    let rows = WordStyle::ALL.len() as u16;
    let start_y = area.top() + (area.height / 2).saturating_sub(rows);

    // Titel.
    f.render_widget(
        Paragraph::new(Span::styled(
            "powerup idle-styles — live, nebeneinander",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Rect {
            x: x0,
            y: start_y.saturating_sub(2),
            width: area.width.saturating_sub(x0),
            height: 1,
        },
    );

    for (row, style) in WordStyle::ALL.iter().enumerate() {
        let y = start_y + row as u16 * 2;
        if y + 1 >= area.bottom() {
            break;
        }
        {
            let buf = f.buffer_mut();
            for (i, ch) in chars.iter().enumerate() {
                let st = style.style_at(frame, i, n);
                if let Some(cell) = buf.cell_mut((x0 + i as u16, y)) {
                    cell.set_char(*ch).set_style(st);
                }
            }
        }
        let lx = x0 + n as u16 + 3;
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{:<9}", style.label()),
                    Style::default().fg(theme::TEXT),
                ),
                Span::styled(style.desc(), Style::default().fg(theme::TEXT_DIM)),
            ])),
            Rect {
                x: lx,
                y,
                width: area.width.saturating_sub(lx),
                height: 1,
            },
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
    let rect = Rect {
        x: area.left(),
        y: area.bottom().saturating_sub(1),
        width: area.width,
        height: 1,
    };
    let bg = Style::default().bg(Color::Rgb(0x18, 0x18, 0x1C));

    let tag = |s: &str| {
        Span::styled(
            format!(" {s} "),
            Style::default()
                .fg(Color::Black)
                .bg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    };

    // Szene 4 hat eigene W2-Schalter — eigene Hilfszeile.
    if state.scene == 4 {
        let n = PWORD.chars().count();
        let line = Line::from(vec![
            tag("hud_lab"),
            Span::styled(" 4:powerup ", Style::default().fg(theme::ACCENT)),
            Span::styled("│ ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled(
                format!("p style:{} ", state.word_style.label()),
                Style::default()
                    .fg(theme::HIGHLIGHT_BG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("o axis:{} ", state.word_axis.label()),
                Style::default().fg(theme::ACCENT),
            ),
            Span::styled(
                format!("t trace:{}/{} ", state.traced, n),
                Style::default().fg(theme::TEXT),
            ),
            Span::styled(
                format!("s cast:{} ", state.cast_style.label()),
                Style::default()
                    .fg(theme::PICKUP_BASE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("w fire ", Style::default().fg(theme::TEXT)),
            Span::styled(
                format!("b buf:{} ", if state.cast_on { "on" } else { "off" }),
                Style::default().fg(theme::TEXT_DIM),
            ),
            Span::styled("· 5 gallery · q", Style::default().fg(theme::TEXT_DIM)),
        ]);
        f.render_widget(Paragraph::new(line).style(bg), rect);
        return;
    }

    if state.scene == 5 {
        let line = Line::from(vec![
            tag("hud_lab"),
            Span::styled(" 5:gallery ", Style::default().fg(theme::ACCENT)),
            Span::styled(
                "│ alle Idle-Styles live nebeneinander ",
                Style::default().fg(theme::TEXT_DIM),
            ),
            Span::styled(
                "· 4 powerup-szene · 1/2/3 · q",
                Style::default().fg(theme::TEXT_DIM),
            ),
        ]);
        f.render_widget(Paragraph::new(line).style(bg), rect);
        return;
    }

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
        Span::styled("· 1/2/3/4 · v inv · f frames · q", Style::default().fg(theme::TEXT_DIM)),
    ]);
    f.render_widget(Paragraph::new(line).style(bg), rect);
}
