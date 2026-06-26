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
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
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
                fx::sweep_in(
                    Motion::LeftToRight,
                    10,
                    0,
                    theme::PANEL_BG,
                    (ms, Interpolation::SineOut),
                ),
                fx::hsl_shift(Some([0.0, 0.0, 35.0]), None, (ms, Interpolation::SineOut)),
            ]),
            Reveal::Fade => fx::fade_from(
                theme::PANEL_BG,
                theme::PANEL_BG,
                (ms, Interpolation::SineOut),
            ),
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

/// Zeichen-Vorrat für die „random chars"-Optik (Preview + Shuffle). Bewusst
/// code-artig (Buchstaben, Ziffern, Symbole), damit der Strahl wie ein
/// flackernder Code-Schweif liest.
const DASH_GLYPHS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789{}[]<>/=+*&^%$#";

/// Deterministischer „Zufalls"-Glyph aus einem Seed (kein rng nötig → stabil
/// pro Frame-Bucket, reproduzierbar).
fn rand_glyph(seed: u64) -> char {
    let h = seed
        .wrapping_mul(2_654_435_761)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    DASH_GLYPHS[(h % DASH_GLYPHS.len() as u64) as usize] as char
}

/// Aspekt-normierte Schritt-Anzahl des Dash-Strahls für eine Richtung: vertikale
/// Zellen zählen ~2× (2:1-Zellaspekt, wie `draw_ring`), damit ALLE 8 Richtungen
/// visuell gleich weit reichen. `beam_reach` ist die Ziel-Sichtweite in
/// Breiten-Einheiten — bewusst groß für einen satten Strahl.
fn dash_steps(dir: (i32, i32)) -> i32 {
    let beam_reach = 16.0;
    let step_len = (((dir.0 * dir.0) + (2 * dir.1) * (2 * dir.1)) as f32)
        .sqrt()
        .max(1.0);
    (beam_reach / step_len).round().max(1.0) as i32
}

// Dash-Abschluss-Timeline (Sekunden, ab Bestätigung):
const DASH_EXTEND: f32 = 0.12; // Kopf schießt base→tip (Trail-Erweiterung)
const DASH_SETTLE_BASE: f32 = 0.28; // erstes Tile hört auf zu shuffeln + setzt sich
const DASH_SETTLE_STAGGER: f32 = 0.05; // Versatz pro Tile (setzt sich base→tip)

/// Laufende Dash-Abschluss-Sequenz (Szene 7): der eigene Trail wurde um `steps`
/// Tiles mit festen Ziel-Buchstaben (`letters`) erweitert; diese shuffeln noch
/// und setzen sich gestaffelt. Die Cast-Welle am Ziel (`wave`) startet ERST nach
/// dem Settle — der Skill ist erst dann abgeschlossen.
struct DashFire {
    age: Duration,
    dir: (i32, i32),
    steps: i32,
    letters: Vec<char>,     // feste Ziel-Buchstaben, Index 0 = Schritt 1
    nonce: u64,             // variiert die Buchstaben/Shuffle pro Abschuss
    wave: Option<Duration>, // Cast-Welle am Ziel; None bis Settle fertig
}

impl DashFire {
    fn settled_at(&self) -> f32 {
        DASH_SETTLE_BASE + self.steps as f32 * DASH_SETTLE_STAGGER
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
    text_fx: Option<Effect>, // tachyonfx-Reveal (hybrid/full-fx), lazy ab Text-Phase
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

// ───────────────────────── #44 Inventar-Explorationen ───────────────────────
// Drei genuin neue W3-Looks (Szene 6, Taste `6`): Pickup → neue Inventar-Zeile,
// Shadow-Autocomplete-Highlight im Cast und Panel-Position/§8-Stil.

const PICKUP_DUR: f32 = 0.60;

/// A/B des Pickup-Looks: eine NEUE Inventar-Zeile kommt rein, durchläuft einen
/// vollen 360°-Hue-Sweep über gesättigter Basis und setzt sich auf `TEXT`-Grau
/// (~550 ms, One-Shot). Render-time gerechnet (voller Sweep + präziser Settle,
/// dieselbe Technik wie der gewählte Cast-Ring) → scroll-immun; im Spiel wandert
/// das als benannter `effects`-Konstruktor bzw. Render-Helper.
#[derive(Clone, Copy, PartialEq)]
enum PickupStyle {
    /// Slide von rechts + Regenbogen, dann Grau (Spec §2 wörtlich).
    SlideRainbow,
    /// wie Slide + kurzer Akzent-Glow (bg-Flash), wenn die Zeile landet.
    SlideRainbowGlow,
    /// sanfter: kein Slide, nur Fade-In + Regenbogen.
    FadeRainbow,
    /// Name erscheint Buchstabe für Buchstabe (Schreibmaschine) in Regenbogen.
    Typewriter,
    /// Buchstaben sind anfangs Zufalls-Glyphen und settlen staffelweise (Coalesce).
    Scatter,
    /// Greller Weiß/Akzent-Flash am Start, klingt zu Grau ab (Pop).
    PopFlash,
    /// ACCENT-Balken wischt von links weg und gibt den Namen frei.
    BarWipe,
    /// Zwei Hue-Pulse über `PICKUP_BASE`, dann Grau.
    DoublePulse,
    /// GEWÄHLT: Pop-Flash + Double-Pulse kombiniert — greller Pop beim Landen,
    /// dann zwei Hue-Pulse, die auf Body-Grau ausklingen.
    PopPulse,
}
impl PickupStyle {
    fn label(self) -> &'static str {
        match self {
            PickupStyle::SlideRainbow => "slide+rainbow",
            PickupStyle::SlideRainbowGlow => "slide+glow",
            PickupStyle::FadeRainbow => "fade+rainbow",
            PickupStyle::Typewriter => "typewriter",
            PickupStyle::Scatter => "scatter",
            PickupStyle::PopFlash => "pop-flash",
            PickupStyle::BarWipe => "bar-wipe",
            PickupStyle::DoublePulse => "double-pulse",
            PickupStyle::PopPulse => "pop-pulse",
        }
    }
    fn next(self) -> Self {
        match self {
            PickupStyle::SlideRainbow => PickupStyle::SlideRainbowGlow,
            PickupStyle::SlideRainbowGlow => PickupStyle::FadeRainbow,
            PickupStyle::FadeRainbow => PickupStyle::Typewriter,
            PickupStyle::Typewriter => PickupStyle::Scatter,
            PickupStyle::Scatter => PickupStyle::PopFlash,
            PickupStyle::PopFlash => PickupStyle::BarWipe,
            PickupStyle::BarWipe => PickupStyle::DoublePulse,
            PickupStyle::DoublePulse => PickupStyle::PopPulse,
            PickupStyle::PopPulse => PickupStyle::SlideRainbow,
        }
    }
}

/// A/B der Inventar-Fenster-Optik (§8-Showcase + 5 weitere Treatments). Alle
/// top-right verankert, alle nach unten wachsend.
#[derive(Clone, Copy, PartialEq)]
enum InvSkin {
    /// (a) Vollrahmen + " POWERUPS "-Titel (§8-Showcase).
    BorderedBox,
    /// (b) randlos mit dickem ACCENT-Balken an der linken Kante.
    LeftBar,
    /// (c) Titel als ACCENT-Pille/Badge mit Live-Count "POWERUPS · N".
    PillBadge,
    /// (d) gerundete Ecken (`BorderType::Rounded`).
    Rounded,
    /// (e) minimal: randlos, ACCENT-Underline-Header, Zeilen durch dim-Rule getrennt.
    Minimal,
    /// (f) kompakt: nur Name, schmaler, " PWR "-Titel.
    Compact,
}
impl InvSkin {
    fn label(self) -> &'static str {
        match self {
            InvSkin::BorderedBox => "bordered",
            InvSkin::LeftBar => "left-bar",
            InvSkin::PillBadge => "pill-badge",
            InvSkin::Rounded => "rounded",
            InvSkin::Minimal => "minimal",
            InvSkin::Compact => "compact",
        }
    }
    fn next(self) -> Self {
        match self {
            InvSkin::BorderedBox => InvSkin::LeftBar,
            InvSkin::LeftBar => InvSkin::PillBadge,
            InvSkin::PillBadge => InvSkin::Rounded,
            InvSkin::Rounded => InvSkin::Minimal,
            InvSkin::Minimal => InvSkin::Compact,
            InvSkin::Compact => InvSkin::BorderedBox,
        }
    }
}

/// A/B des Shadow-Autocomplete-Highlights (Cast-Modus, Spec §7/§8): der getippte
/// Prefix steckt im Pink-Kasten (`HIGHLIGHT_BG/FG`), der Rest bleibt lesbar.
#[derive(Clone, Copy, PartialEq)]
enum ShadowStyle {
    /// (a) nur der Prefix-Kasten auf der gematchten Zeile.
    BoxOnly,
    /// (b) zusätzlich die nicht-matchenden Zeilen dimmen (`TEXT_DIM`).
    BoxDim,
}
impl ShadowStyle {
    fn label(self) -> &'static str {
        match self {
            ShadowStyle::BoxOnly => "box",
            ShadowStyle::BoxDim => "box+dim",
        }
    }
    fn next(self) -> Self {
        match self {
            ShadowStyle::BoxOnly => ShadowStyle::BoxDim,
            ShadowStyle::BoxDim => ShadowStyle::BoxOnly,
        }
    }
}

/// A/B der Inventar-Panel-Position (§8 lässt sie offen).
#[derive(Clone, Copy, PartialEq)]
enum InvAnchor {
    Center,
    TopRight,
    BottomRight,
}
impl InvAnchor {
    fn label(self) -> &'static str {
        match self {
            InvAnchor::Center => "center",
            InvAnchor::TopRight => "top-right",
            InvAnchor::BottomRight => "bottom-right",
        }
    }
    fn anchor(self) -> Anchor {
        match self {
            InvAnchor::Center => Anchor::Center,
            InvAnchor::TopRight => Anchor::TopRight,
            InvAnchor::BottomRight => Anchor::BottomRight,
        }
    }
    fn next(self) -> Self {
        match self {
            InvAnchor::Center => InvAnchor::TopRight,
            InvAnchor::TopRight => InvAnchor::BottomRight,
            InvAnchor::BottomRight => InvAnchor::Center,
        }
    }
}

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
    // #44 Inventar-Explorationen (Szene 6)
    pickup_style: PickupStyle,
    pickup_anim: Option<Duration>, // Alter der laufenden Pickup-Zeile (None = ruht)
    inv_rows: Vec<usize>,          // Pool-Indizes der eingesammelten Powerups
    inv_skin: InvSkin,
    shadow_style: ShadowStyle,
    shadow_len: usize, // getippte Prefix-Länge gegen "dash" (0..=4)
    inv_anchor: InvAnchor,
    // Dash-Aim-Szene (Szene 7)
    dash_dir: prfh::game::skill::Aim8,
    dash_age: Duration,
    dash_burst: bool,            // false = Blink, true = Trail-Burst
    dash_beam_style: u8,         // 0..=4, A/B der Strahl-Stile
    dash_fire: Option<DashFire>, // läuft die Abschluss-Sequenz?
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
            pickup_style: PickupStyle::PopPulse,
            pickup_anim: None,
            inv_rows: vec![0, 1], // ~2 Start-Zeilen
            inv_skin: InvSkin::Rounded,
            shadow_style: ShadowStyle::BoxDim,
            shadow_len: 0,
            inv_anchor: InvAnchor::TopRight,
            dash_dir: prfh::game::skill::Aim8::E,
            dash_age: Duration::ZERO,
            dash_burst: false,
            dash_beam_style: 0,
            dash_fire: None,
        }
    }

    /// Dash bestätigen (Szene 7): erweitert den eigenen Trail um `steps` Tiles mit
    /// festen, zufälligen Buchstaben und startet die Abschluss-Sequenz
    /// (Shuffle→Settle→Cast am Ziel). `nonce` aus dem aktuellen `dash_age`, damit
    /// jeder Abschuss andere Buchstaben zieht.
    fn fire_dash(&mut self) {
        if self.dash_fire.is_some() {
            return;
        }
        let dir = self.dash_dir.delta();
        let steps = dash_steps(dir);
        let nonce = self.dash_age.as_nanos() as u64 | 1;
        let letters = (1..=steps)
            .map(|i| rand_glyph((i as u64).wrapping_mul(0x9E37_79B9) ^ nonce))
            .collect();
        self.dash_fire = Some(DashFire {
            age: Duration::ZERO,
            dir,
            steps,
            letters,
            nonce,
            wave: None,
        });
    }

    /// Szene 6 betreten: Panel auf, sauberer Demo-Zustand (~2 Zeilen).
    fn enter_inventory_scene(&mut self) {
        self.inv_open = true;
        self.inv_closing = false;
        self.inv_effect = fx::coalesce((400, Interpolation::SineOut));
        self.shadow_len = 0;
        self.inv_rows = vec![0, 1];
        self.pickup_anim = None;
    }

    /// Pickup auslösen: nächstes Pool-Powerup ans Inventar anhängen, die neue
    /// Zeile animiert rein, Panel wächst nach unten.
    fn fire_pickup(&mut self) {
        self.inv_open = true;
        self.inv_closing = false;
        self.inv_rows.push(self.inv_rows.len() % INV_POOL.len());
        self.pickup_anim = Some(Duration::ZERO);
    }

    /// Inventar leeren — zum Vergleich leer vs. voll.
    fn clear_inventory(&mut self) {
        self.inv_rows.clear();
        self.pickup_anim = None;
    }

    /// Cast-Prefix gegen "dash" weitersteppen (0..=4, wrappt zurück auf 0).
    fn advance_shadow(&mut self) {
        self.shadow_len = if self.shadow_len >= 4 {
            0
        } else {
            self.shadow_len + 1
        };
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
            (
                Kind::Event,
                "✦  PICKUP",
                "dash ins Inventar",
                theme::PICKUP_BASE,
            ),
            (Kind::Info, "⟹  STOP", "next char overwrites", theme::DANGER),
            (
                Kind::Major,
                "✓  MERGED",
                "main is green",
                theme::HIGHLIGHT_BG,
            ),
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
        // Pickup-Zeile altert; nach `PICKUP_DUR` ruht sie als normale Grau-Zeile.
        if let Some(age) = &mut self.pickup_anim {
            *age += dt;
            if age.as_secs_f32() > PICKUP_DUR {
                self.pickup_anim = None;
            }
        }
        // Dash-Aim-Szene: Preview-Uhr + Abschluss-Sequenz altern.
        self.dash_age += dt;
        if let Some(df) = self.dash_fire.as_mut() {
            df.age += dt;
            // Cast-Welle am Ziel ERST nach dem Settle starten (Skill-Abschluss).
            if df.wave.is_none() && df.age.as_secs_f32() >= df.settled_at() {
                df.wave = Some(Duration::ZERO);
            }
            if let Some(w) = df.wave.as_mut() {
                *w += dt;
                if w.as_secs_f32() > RING_DUR {
                    self.dash_fire = None; // Sequenz fertig → zurück zur Preview
                }
            }
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
                        KeyCode::Char('6') => {
                            state.scene = 6;
                            state.enter_inventory_scene();
                        }
                        KeyCode::Char('7') => state.scene = 7,
                        KeyCode::Char('g') => state.fire_pickup(),
                        KeyCode::Char('x') => state.clear_inventory(),
                        KeyCode::Char('j') => state.pickup_style = state.pickup_style.next(),
                        KeyCode::Char('u') => state.inv_skin = state.inv_skin.next(),
                        KeyCode::Char('h') => state.advance_shadow(),
                        KeyCode::Char('l') => state.shadow_style = state.shadow_style.next(),
                        KeyCode::Char('k') => state.inv_anchor = state.inv_anchor.next(),
                        KeyCode::Char('p') => state.word_style = state.word_style.next(),
                        KeyCode::Char('o') => state.word_axis = state.word_axis.next(),
                        KeyCode::Char('t') => state.advance_trace(),
                        KeyCode::Char('w') => state.fire_wave(),
                        KeyCode::Char('s') => {
                            if state.scene == 7 {
                                state.dash_beam_style = (state.dash_beam_style + 1) % 5;
                            } else {
                                state.cast_style = state.cast_style.next();
                            }
                        }
                        KeyCode::Char('b') => {
                            if state.scene == 7 {
                                state.dash_burst = !state.dash_burst;
                            } else {
                                state.cast_on = !state.cast_on;
                            }
                        }
                        KeyCode::Char('n') => state.fire(),
                        KeyCode::Char('m') => state.mode = state.mode.next(),
                        KeyCode::Char('i') => state.reveal = state.reveal.next(),
                        KeyCode::Char('c') => state.cursor = state.cursor.next(),
                        KeyCode::Char('v') => state.toggle_inventory(),
                        KeyCode::Char('f') => state.frames = !state.frames,
                        KeyCode::Up => state.dir = '↑',
                        KeyCode::Down => state.dir = '↓',
                        KeyCode::Left => {
                            if state.scene == 7 {
                                state.dash_dir = state.dash_dir.rotate(false);
                            } else {
                                state.dir = '←';
                            }
                        }
                        KeyCode::Right => {
                            if state.scene == 7 {
                                state.dash_dir = state.dash_dir.rotate(true);
                            } else {
                                state.dir = '→';
                            }
                        }
                        KeyCode::Enter if state.scene == 7 => state.fire_dash(),
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
        6 => {} // Inventar-Lab: nur Welt-BG + Overlay + Hilfe (Panel via inv_open)
        7 => layout_dash_aim(f, area, state),
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
    let rect = Rect {
        x,
        y: top,
        width: w,
        height: h,
    };

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
            e.process(
                dt.into(),
                f.buffer_mut(),
                Rect {
                    x: area.left() + area.width.saturating_sub(full_w) / 2,
                    y: top,
                    width: full_w,
                    height: h,
                },
            );
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
                Style::default()
                    .fg(title_fg)
                    .bg(theme::PANEL_BG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                n.detail.clone(),
                Style::default().fg(detail_fg).bg(theme::PANEL_BG),
            ),
        ]);
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .render(rect, buf);
    } else {
        Paragraph::new(Span::styled(
            n.title.clone(),
            Style::default()
                .fg(title_fg)
                .bg(theme::PANEL_BG)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .render(
            Rect {
                y: rect.y,
                height: 1,
                ..rect
            },
            buf,
        );
        Paragraph::new(Span::styled(
            n.detail.clone(),
            Style::default().fg(detail_fg).bg(theme::PANEL_BG),
        ))
        .alignment(Alignment::Center)
        .render(
            Rect {
                y: rect.y + 1,
                height: 1,
                ..rect
            },
            buf,
        );
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
    chip(
        f,
        anchor_rect(area, Anchor::TopLeft, 9, 1),
        "dir",
        &state.dir.to_string(),
        state.frames,
    );
    chip(
        f,
        anchor_rect(area, Anchor::TopRight, 12, 1),
        "combo",
        &format!("x{}", state.combo),
        state.frames,
    );
    cursor_marker(f, area, state.dir, state.cursor, state.frame);
    players_strip(f, anchor_rect(area, Anchor::BottomLeft, 30, 1));
}

fn layout_bottom_strip(f: &mut Frame, area: Rect, state: &State) {
    let rect = anchor_rect(area, Anchor::BottomCenter, area.width.min(60), 1);
    let line = Line::from(vec![
        Span::styled(" dir ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("{} ", state.dir),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  combo ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("x{} ", state.combo),
            Style::default()
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  you",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("(du) rival ", Style::default().fg(theme::TEXT_DIM)),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::PANEL_BG)),
        rect,
    );
    cursor_marker(f, area, state.dir, state.cursor, state.frame);
}

fn layout_diegetic(f: &mut Frame, area: Rect, state: &State) {
    let c = anchor_rect(area, Anchor::Center, 3, 1);
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(" {} ", state.dir),
            Style::default()
                .fg(Color::Black)
                .bg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )])),
        c,
    );
    if state.combo > 1 {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("x{}", state.combo),
                Style::default().fg(theme::TEXT_DIM),
            )),
            Rect {
                x: area.left() + area.width / 2 - 3,
                y: area.bottom().saturating_sub(1),
                width: 6,
                height: 1,
            },
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
        Span::styled(
            rest,
            Style::default().fg(theme::TEXT_DIM).bg(theme::PANEL_BG),
        ),
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
        let b = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(theme::TEXT_DIM));
        let inner = b.inner(rect);
        f.render_widget(b, rect);
        inner
    } else {
        rect
    };
    let line = Line::from(vec![
        Span::styled(format!("{key} "), Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            val.to_string(),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), inner);
}

fn players_strip(f: &mut Frame, rect: Rect) {
    let line = Line::from(vec![
        Span::styled(
            "you",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("(du) ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            "rival ",
            Style::default()
                .fg(Color::Rgb(0x8A, 0xE2, 0x34))
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), rect);
}

// Demo-Pool, aus dem `g` (grab) der Reihe nach Powerups einsammelt.
const INV_POOL: &[(&str, &str)] = &[
    ("dash", "schärft Richtungs-Sprint"),
    ("revert", "macht letzten Zug rückgängig"),
    ("squash", "fasst Trail zusammen"),
    ("warp", "teleportiert ein Feld"),
    ("purge", "löscht fremde Spur"),
    ("bolt", "beschleunigt kurz"),
    ("snap", "rastet aufs Raster"),
    ("forge", "setzt einen Block"),
];

fn rgb_of(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

/// Linearer RGB-Lerp zwischen zwei Farben (`t` ∈ 0..1).
fn blend(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (ar, ag, ab) = rgb_of(a);
    let (br, bg, bb) = rgb_of(b);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(l(ar, br), l(ag, bg), l(ab, bb))
}

/// Voller 360°-Hue-Sweep über gesättigter Basis, der zum Ende auf `TEXT`-Grau
/// entsättigt (render-time → präziser Settle, vgl. Cast-Ring).
fn rainbow_fg(p: f32) -> Color {
    let hue = p * 360.0;
    let sat = (1.0 - p) * 0.85;
    let light = 0.62 + 0.10 * (1.0 - p);
    blend(hsl(hue, sat, light), theme::TEXT, p)
}

fn blank_line() -> Line<'static> {
    Line::from(Span::styled(" ", Style::default().bg(theme::PANEL_BG)))
}

fn rule_line(w: u16) -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(w as usize),
        Style::default().fg(theme::TEXT_DIM).bg(theme::PANEL_BG),
    ))
}

fn push_desc(spans: &mut Vec<Span<'static>>, desc: &str, fg: Color, bg: Color, show: bool) {
    if show {
        spans.push(Span::styled(
            desc.to_string(),
            Style::default().fg(fg).bg(bg),
        ));
    }
}

/// Eine ruhende Inventar-Zeile — mit Shadow-Autocomplete-Highlight, falls der
/// getippte Prefix diese Zeile matcht (sonst ggf. gedimmt bei `BoxDim`).
fn inv_row(
    name: &str,
    desc: &str,
    state: &State,
    shadow_active: bool,
    typed: &str,
    show_desc: bool,
) -> Line<'static> {
    if shadow_active && !typed.is_empty() && name.starts_with(typed) {
        // Highlight ist eine REINE Stiländerung über exakt dieselben Zeichen:
        // Leerzeichen + prefix + (rest+pad). prefix und rest+pad ergeben zusammen
        // immer 8 Spalten (= `{name:<8}`), also ist das Namensfeld mit/ohne Box
        // identisch breit — kein Spaltenversatz, `desc` startet gleich. (Der
        // frühere Bug: ein extra Trailing-Space hier machte die Zeile 1 Spalte breiter.)
        let len = typed.chars().count().min(name.chars().count());
        let prefix: String = name.chars().take(len).collect();
        let rest: String = name.chars().skip(len).collect();
        let pad = " ".repeat(8usize.saturating_sub(name.chars().count()));
        let mut spans = vec![
            Span::styled(" ", Style::default().bg(theme::PANEL_BG)),
            Span::styled(
                prefix,
                Style::default()
                    .fg(theme::HIGHLIGHT_FG)
                    .bg(theme::HIGHLIGHT_BG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{rest}{pad}"),
                Style::default().fg(theme::TEXT).bg(theme::PANEL_BG),
            ),
        ];
        push_desc(
            &mut spans,
            desc,
            theme::TEXT_DIM,
            theme::PANEL_BG,
            show_desc,
        );
        return Line::from(spans);
    }
    let dim = shadow_active && matches!(state.shadow_style, ShadowStyle::BoxDim);
    let name_fg = if dim { theme::TEXT_DIM } else { theme::TEXT };
    let modi = if dim {
        Modifier::empty()
    } else {
        Modifier::BOLD
    };
    let mut spans = vec![Span::styled(
        format!(" {name:<8}"),
        Style::default()
            .fg(name_fg)
            .bg(theme::PANEL_BG)
            .add_modifier(modi),
    )];
    push_desc(
        &mut spans,
        desc,
        theme::TEXT_DIM,
        theme::PANEL_BG,
        show_desc,
    );
    Line::from(spans)
}

/// Die gerade eingesammelte Zeile, animiert nach gewähltem `PickupStyle`. Alle
/// Varianten sind One-Shots (~`PICKUP_DUR`) auf dem statischen Zeilen-Rect —
/// render-time gerechnet, damit der Look präzise auf Body-Grau landet.
fn animated_pickup_line(
    style: PickupStyle,
    age: Duration,
    name: &str,
    desc: &str,
    show_desc: bool,
) -> Line<'static> {
    let p = (age.as_secs_f32() / PICKUP_DUR).clamp(0.0, 1.0);
    let fg = rainbow_fg(p);
    let panel = theme::PANEL_BG;
    let mut spans: Vec<Span<'static>> = Vec::new();
    match style {
        PickupStyle::SlideRainbow => {
            let pad = " ".repeat(((1.0 - p) * 10.0).round() as usize);
            spans.push(Span::styled(
                format!("{pad} {name:<8}"),
                Style::default()
                    .fg(fg)
                    .bg(panel)
                    .add_modifier(Modifier::BOLD),
            ));
            push_desc(&mut spans, desc, fg, panel, show_desc);
        }
        PickupStyle::SlideRainbowGlow => {
            let pad = " ".repeat(((1.0 - p) * 10.0).round() as usize);
            let g = (1.0 - ((p - 0.8).abs() / 0.2)).clamp(0.0, 1.0);
            let bg = if g > 0.02 {
                blend(panel, theme::ACCENT, g * 0.8)
            } else {
                panel
            };
            spans.push(Span::styled(
                format!("{pad} {name:<8}"),
                Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            ));
            push_desc(&mut spans, desc, fg, bg, show_desc);
        }
        PickupStyle::FadeRainbow => {
            let f = blend(panel, fg, p.max(0.15));
            spans.push(Span::styled(
                format!(" {name:<8}"),
                Style::default()
                    .fg(f)
                    .bg(panel)
                    .add_modifier(Modifier::BOLD),
            ));
            push_desc(&mut spans, desc, f, panel, show_desc);
        }
        PickupStyle::Typewriter => {
            let n = name.chars().count();
            let k = ((p * n as f32).ceil() as usize).min(n);
            let shown: String = name.chars().take(k).collect();
            spans.push(Span::styled(
                format!(" {shown}"),
                Style::default()
                    .fg(fg)
                    .bg(panel)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                " ".repeat(8usize.saturating_sub(k)),
                Style::default().bg(panel),
            ));
            push_desc(&mut spans, desc, theme::TEXT_DIM, panel, show_desc);
        }
        PickupStyle::Scatter => {
            // Pseudo-Coalesce: jeder Buchstabe ist anfangs ein Zufalls-Glyph und
            // settelt staffelweise (linke Buchstaben zuerst).
            const GLYPHS: [char; 7] = ['#', '*', '%', '░', '▒', '·', '+'];
            let cnt = name.chars().count().max(1);
            let frame = (p * 12.0) as u64;
            let s: String = name
                .chars()
                .enumerate()
                .map(|(i, ch)| {
                    let settled = p > 0.25 + 0.55 * (i as f32 / cnt as f32);
                    if settled {
                        ch
                    } else {
                        let h = (i as u64).wrapping_mul(2_654_435_761) ^ frame.wrapping_mul(40_503);
                        GLYPHS[(h % 7) as usize]
                    }
                })
                .collect();
            spans.push(Span::styled(
                format!(" {s:<8}"),
                Style::default()
                    .fg(fg)
                    .bg(panel)
                    .add_modifier(Modifier::BOLD),
            ));
            push_desc(&mut spans, desc, fg, panel, show_desc);
        }
        PickupStyle::PopFlash => {
            // Greller Weiß/Akzent-Flash am Start, klingt zu TEXT/Panel ab.
            let flash = (1.0 - p).powi(2);
            let f = blend(theme::TEXT, Color::Rgb(255, 255, 255), flash);
            let bg = blend(panel, theme::ACCENT, flash * 0.7);
            spans.push(Span::styled(
                format!(" {name:<8}"),
                Style::default().fg(f).bg(bg).add_modifier(Modifier::BOLD),
            ));
            push_desc(
                &mut spans,
                desc,
                blend(theme::TEXT_DIM, theme::TEXT, flash),
                bg,
                show_desc,
            );
        }
        PickupStyle::BarWipe => {
            // ACCENT-Balken wischt von links weg und gibt den Namen frei.
            let total = 8usize;
            let revealed = ((p * total as f32).round() as usize).min(total);
            let name_pad = format!("{name:<8}");
            let shown: String = name_pad.chars().take(revealed).collect();
            let bar = "█".repeat(total - revealed);
            spans.push(Span::styled(
                format!(" {shown}"),
                Style::default()
                    .fg(theme::TEXT)
                    .bg(panel)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                bar,
                Style::default().fg(theme::ACCENT).bg(panel),
            ));
            push_desc(&mut spans, desc, theme::TEXT_DIM, panel, show_desc);
        }
        PickupStyle::DoublePulse => {
            // Zwei Hue-Pulse über `PICKUP_BASE`, dann ruhig auf Grau.
            let pulse =
                ((1.0 - p) * (0.5 + 0.5 * (std::f32::consts::PI * 4.0 * p).sin())).clamp(0.0, 1.0);
            let f = blend(theme::TEXT, theme::PICKUP_BASE, pulse);
            spans.push(Span::styled(
                format!(" {name:<8}"),
                Style::default()
                    .fg(f)
                    .bg(panel)
                    .add_modifier(Modifier::BOLD),
            ));
            push_desc(&mut spans, desc, theme::TEXT_DIM, panel, show_desc);
        }
        PickupStyle::PopPulse => {
            // GEWÄHLT (Kombination): erst ein kurzer, greller Pop-Flash beim Landen
            // (weiß über Akzent-bg, in ~30 % abgeklungen), dann die zwei Hue-Pulse
            // über `PICKUP_BASE`, die präzise auf Body-Grau ausklingen.
            let flash = (1.0 - p / 0.30).clamp(0.0, 1.0).powi(2);
            let pulse =
                ((1.0 - p) * (0.5 + 0.5 * (std::f32::consts::PI * 4.0 * p).sin())).clamp(0.0, 1.0);
            let base = blend(theme::TEXT, theme::PICKUP_BASE, pulse);
            let f = blend(base, Color::Rgb(255, 255, 255), flash);
            let bg = blend(panel, theme::ACCENT, flash * 0.7);
            spans.push(Span::styled(
                format!(" {name:<8}"),
                Style::default().fg(f).bg(bg).add_modifier(Modifier::BOLD),
            ));
            push_desc(
                &mut spans,
                desc,
                blend(theme::TEXT_DIM, theme::TEXT, flash),
                bg,
                show_desc,
            );
        }
    }
    Line::from(spans)
}

/// Item-Zeilen des Inventars; die zuletzt hinzugefügte Zeile animiert, solange
/// `pickup_anim` läuft.
fn item_lines(state: &State, show_desc: bool) -> Vec<Line<'static>> {
    let shadow_active = state.shadow_len > 0;
    let typed: String = "dash".chars().take(state.shadow_len).collect();
    let last = state.inv_rows.len().saturating_sub(1);
    state
        .inv_rows
        .iter()
        .enumerate()
        .map(|(i, &pi)| {
            let (name, desc) = INV_POOL[pi];
            match state.pickup_anim {
                Some(age) if i == last => {
                    animated_pickup_line(state.pickup_style, age, name, desc, show_desc)
                }
                _ => inv_row(name, desc, state, shadow_active, &typed, show_desc),
            }
        })
        .collect()
}

/// Inventar-Overlay (Szene 6): top-right verankert, wächst nach unten, mit dem
/// gewählten `InvSkin` (§8-Showcase + 5 weitere Treatments).
fn draw_inventory(f: &mut Frame, area: Rect, state: &mut State, dt: Duration) {
    let skin = state.inv_skin;
    let show_desc = !matches!(skin, InvSkin::Compact);
    let width: u16 = if matches!(skin, InvSkin::Compact) {
        24
    } else {
        42
    };
    let inner_w = width.saturating_sub(2);
    let bordered = matches!(
        skin,
        InvSkin::BorderedBox | InvSkin::Rounded | InvSkin::Compact
    );
    let row_rule = matches!(skin, InvSkin::Minimal);

    // Inhalt aufbauen (Header je Skin + Item-Zeilen + §8-Atemzeilen).
    let mut content: Vec<Line> = Vec::new();
    if bordered {
        content.push(blank_line()); // §8: 1 BG-Zeile über dem (Border-)Header
    } else {
        match skin {
            InvSkin::PillBadge => {
                content.push(blank_line());
                content.push(Line::from(Span::styled(
                    format!(" POWERUPS · {} ", state.inv_rows.len()),
                    Style::default()
                        .fg(theme::HIGHLIGHT_FG)
                        .bg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                )));
                content.push(blank_line());
            }
            InvSkin::LeftBar => {
                content.push(blank_line());
                content.push(Line::from(Span::styled(
                    "  POWERUPS",
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                )));
                content.push(blank_line());
            }
            InvSkin::Minimal => {
                content.push(Line::from(Span::styled(
                    "  POWERUPS",
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                )));
                content.push(rule_line(inner_w));
            }
            _ => unreachable!(),
        }
    }

    let items = item_lines(state, show_desc);
    if items.is_empty() {
        content.push(Line::from(Span::styled(
            "  — leer —",
            Style::default().fg(theme::TEXT_DIM).bg(theme::PANEL_BG),
        )));
    }
    for (i, line) in items.into_iter().enumerate() {
        if row_rule && i > 0 {
            content.push(rule_line(inner_w));
        }
        content.push(line);
    }
    if bordered {
        content.push(blank_line()); // §8: 1 BG-Zeile unter den Zeilen
    }

    let h = (content.len() as u16 + if bordered { 2 } else { 0 }).max(3);
    let rect = anchor_rect(area, state.inv_anchor.anchor(), width, h);
    f.render_widget(Clear, rect);

    let para_rect = if bordered {
        let bt = if matches!(skin, InvSkin::Rounded) {
            BorderType::Rounded
        } else {
            BorderType::Plain
        };
        let title = if matches!(skin, InvSkin::Compact) {
            " PWR "
        } else {
            " POWERUPS "
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(bt)
            .border_style(Style::default().fg(theme::TEXT_DIM))
            .style(Style::default().bg(theme::PANEL_BG))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        inner
    } else {
        // Randlos: Hintergrund selbst füllen.
        f.render_widget(
            Block::default().style(Style::default().bg(theme::PANEL_BG)),
            rect,
        );
        if matches!(skin, InvSkin::LeftBar) {
            for y in rect.top()..rect.bottom() {
                if let Some(c) = f.buffer_mut().cell_mut((rect.left(), y)) {
                    c.set_char('█')
                        .set_fg(theme::ACCENT)
                        .set_bg(theme::PANEL_BG);
                }
            }
            Rect {
                x: rect.left() + 2,
                y: rect.top(),
                width: rect.width.saturating_sub(2),
                height: rect.height,
            }
        } else {
            rect
        }
    };

    f.render_widget(
        Paragraph::new(content).style(Style::default().bg(theme::PANEL_BG)),
        para_rect,
    );
    state.inv_effect.process(dt.into(), f.buffer_mut(), rect);
}

/// Szene 7 — Dash-Aim: 8-Richtungs-Rotation, beide Dash-Mechaniken (Blink vs.
/// Trail-Burst) und 3 Vorschau-Strahl-Stile im A/B-Vergleich. Die Kamera ist
/// FEST — so ist der Streak über das Feld als Demo sichtbar.
fn layout_dash_aim(f: &mut Frame, area: Rect, state: &State) {
    use ratatui::style::Color;
    let buf = f.buffer_mut();
    let center = (
        area.left() as i32 + area.width as i32 / 3,
        area.top() as i32 + area.height as i32 / 2,
    );
    let (dx, dy) = state.dash_dir.delta();
    // Aspekt-normierte Schritt-Anzahl (alle 8 Richtungen gleich weit, s. dash_steps).
    let steps = dash_steps((dx, dy));
    let t = state.dash_age.as_secs_f32();
    let in_bounds = |x: i32, y: i32| {
        x >= area.left() as i32
            && x < area.right() as i32
            && y >= area.top() as i32
            && y < area.bottom() as i32
    };

    // Vorschau-Strahl (5 Stile per `s`).
    if state.dash_fire.is_none() {
        for i in 1..=steps {
            let x = center.0 + dx * i;
            let y = center.1 + dy * i;
            if !in_bounds(x, y) {
                continue;
            }
            // Fortschritt 0..1 entlang des Strahls — richtungs-unabhängig, da `steps`
            // pro Richtung normiert ist.
            let f = i as f32 / steps as f32;
            let pulse = 0.5 + 0.5 * (f * 4.0 - t * 6.0).sin();
            let last = i == steps;
            let (ch, col) = match state.dash_beam_style {
                0 => {
                    // Flowing Gradient Pulse
                    let hue = 200.0 + f * 50.0 + t * 60.0;
                    let l = 0.45 + 0.35 * pulse;
                    (
                        if last {
                            '◎'
                        } else if dy == 0 {
                            '─'
                        } else if dx == 0 {
                            '│'
                        } else {
                            '·'
                        },
                        hsl(hue, 0.55, if last { 0.8 } else { l }),
                    )
                }
                1 => {
                    // Charging Sweep: heller Kopf wandert
                    let head = ((t * 8.0) as i32 % steps) + 1;
                    let bright = if i == head { 1.0 } else { 0.25 };
                    ('=', hsl(190.0, 0.5, 0.35 + 0.5 * bright))
                }
                2 => {
                    // Shimmer/Laser: stabil + Funkeln
                    let hsh = (x as u64)
                        .wrapping_mul(2_654_435_761)
                        .wrapping_add(state.dash_age.as_millis() as u64);
                    let spark = (hsh % 7) < 1;
                    (
                        if last { '◎' } else { '─' },
                        hsl(330.0, 0.5, if spark { 0.9 } else { 0.5 }),
                    )
                }
                3 => {
                    // Volle, unterschiedlich shaded Blöcke: ein nach außen fließender
                    // Gradient (leicht animiert). Block-Glyph nach Wellen-Intensität,
                    // Farbe als Hue-Verlauf entlang des Strahls.
                    let wave = 0.5 + 0.5 * (f * 5.0 - t * 4.0).sin();
                    let block = if wave > 0.72 {
                        '█'
                    } else if wave > 0.48 {
                        '▓'
                    } else if wave > 0.24 {
                        '▒'
                    } else {
                        '░'
                    };
                    let hue = 265.0 + f * 70.0 + t * 25.0;
                    (
                        if last { '█' } else { block },
                        hsl(hue, 0.6, 0.35 + 0.45 * wave),
                    )
                }
                _ => {
                    // Random Chars: flackernder Code-Schweif aus zufälligen Zeichen +
                    // dezenter Farb-Animation → liest sich klar als unverbindliche
                    // Vorschau (genau die Optik, in die sich der gesetzte Trail
                    // nachher „einfriert"). Langsamer Shuffle-Bucket.
                    let bucket = (t * 12.0) as u64;
                    let glyph = rand_glyph((i as u64).wrapping_mul(2_654_435_761) ^ bucket);
                    let hue = 250.0 + f * 30.0 + t * 18.0;
                    (glyph, hsl(hue, 0.32, 0.52 + 0.14 * pulse))
                }
            };
            if let Some(cell) = buf.cell_mut((x as u16, y as u16)) {
                cell.set_char(ch).set_fg(col);
            }
        }
    }

    // Abschluss-Sequenz: der eigene Trail ist um `steps` Tiles mit festen
    // Buchstaben erweitert. Pro Tile (base→tip, gestaffelt):
    //   1) Extend: Kopf schießt von der Mitte nach außen (DASH_EXTEND).
    //   2) Shuffle: zufällige Zeichen flackern, bis `settle_at(i)` erreicht ist;
    //      der Shuffle verlangsamt sich zum Settle hin.
    //   3) Settle: Glyph rastet auf den festen Ziel-Buchstaben ein → ganz normaler
    //      Trail-Bestandteil (heller, dezent abklingender Look).
    // Die Cast-Welle am ZIEL kommt erst danach (in update gesetzt) — der Skill ist
    // erst mit dem Settle abgeschlossen.
    if let Some(df) = &state.dash_fire {
        let tf = df.age.as_secs_f32();
        let head = (tf / DASH_EXTEND * df.steps as f32).floor() as i32; // sichtbarer Kopf
        for i in 1..=df.steps {
            if i > head {
                continue; // noch nicht erweitert
            }
            let x = center.0 + df.dir.0 * i;
            let y = center.1 + df.dir.1 * i;
            if !in_bounds(x, y) {
                continue;
            }
            let settle_at = DASH_SETTLE_BASE + i as f32 * DASH_SETTLE_STAGGER;
            let settled = tf >= settle_at;
            let (ch, col) = if settled {
                // Gesetzter Trail-Buchstabe: hell, fast neutral — Teil des Trails.
                (df.letters[(i - 1) as usize], hsl(210.0, 0.10, 0.86))
            } else {
                // Shuffle: je näher am Settle, desto langsamer der Bucket-Wechsel.
                let remaining = (settle_at - tf).max(0.0);
                let speed = (10.0 + 60.0 * remaining).min(70.0);
                let bucket = (tf * speed) as u64;
                let seed = (i as u64).wrapping_mul(2_654_435_761) ^ bucket ^ df.nonce;
                let hue = 255.0 + i as f32 * 5.0 + tf * 40.0;
                (rand_glyph(seed), hsl(hue, 0.42, 0.62))
            };
            if let Some(cell) = buf.cell_mut((x as u16, y as u16)) {
                cell.set_char(ch).set_fg(col);
            }
        }

        // Cast-Welle am Ziel — erst nach dem Settle (Skill-Abschluss = Aktivierung).
        if let Some(w) = df.wave {
            let tx = center.0 + df.dir.0 * df.steps;
            let ty = center.1 + df.dir.1 * df.steps;
            draw_ring(buf, tx, ty, w, area);
        }
    }

    // Spieler-Glyph an center.
    if let Some(cell) = buf.cell_mut((center.0 as u16, center.1 as u16)) {
        cell.set_char('@').set_fg(Color::White);
    }
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
            Span::styled(
                "· 5 gallery · 6 inv-lab · q",
                Style::default().fg(theme::TEXT_DIM),
            ),
        ]);
        f.render_widget(Paragraph::new(line).style(bg), rect);
        return;
    }

    if state.scene == 6 {
        let typed: String = "dash".chars().take(state.shadow_len).collect();
        let shadow_disp = if state.shadow_len == 0 {
            "—".to_string()
        } else {
            format!("{typed}|")
        };
        let line = Line::from(vec![
            tag("hud_lab"),
            Span::styled(" 6:inv ", Style::default().fg(theme::ACCENT)),
            Span::styled("│ ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled(
                format!("g grab({}) ", state.inv_rows.len()),
                Style::default()
                    .fg(theme::PICKUP_BASE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("x clear ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled(
                format!("u skin:{} ", state.inv_skin.label()),
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("j pickup:{} ", state.pickup_style.label()),
                Style::default().fg(theme::TEXT),
            ),
            Span::styled(
                format!("h type:{shadow_disp} "),
                Style::default()
                    .fg(theme::HIGHLIGHT_BG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("l shadow:{} ", state.shadow_style.label()),
                Style::default().fg(theme::TEXT_DIM),
            ),
            Span::styled(
                format!("k pos:{} ", state.inv_anchor.label()),
                Style::default().fg(theme::TEXT_DIM),
            ),
            Span::styled("· 4 · q", Style::default().fg(theme::TEXT_DIM)),
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

    if state.scene == 7 {
        let mech = if state.dash_burst { "burst" } else { "blink" };
        let beam_labels = ["gradient", "charging", "shimmer", "blocks", "chars"];
        let beam = beam_labels[state.dash_beam_style as usize];
        let line = Line::from(vec![
            tag("hud_lab"),
            Span::styled(" 7:dash-aim ", Style::default().fg(theme::ACCENT)),
            Span::styled("│ ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled(
                "◄ ► drehen ",
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("· Enter dash ", Style::default().fg(theme::TEXT)),
            Span::styled(
                format!("b mech:{mech} "),
                Style::default()
                    .fg(theme::HIGHLIGHT_BG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("s strahl:{beam} "),
                Style::default()
                    .fg(theme::PICKUP_BASE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("dir:{:?} ", state.dash_dir),
                Style::default().fg(theme::ACCENT),
            ),
            Span::styled("· 1/2/3/4/6 · q", Style::default().fg(theme::TEXT_DIM)),
        ]);
        f.render_widget(Paragraph::new(line).style(bg), rect);
        return;
    }

    let scene = match state.scene {
        2 => "2:strip",
        3 => "3:diegetic",
        _ => "1:corners",
    };
    let reveal_dim = if state.mode.uses_fx_text() {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    let line = Line::from(vec![
        Span::styled(
            " hud_lab ",
            Style::default()
                .fg(Color::Black)
                .bg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {scene} "), Style::default().fg(theme::ACCENT)),
        Span::styled("│ n notif · ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("m mode:{} ", state.mode.label()),
            Style::default()
                .fg(theme::HIGHLIGHT_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("i reveal:{} ", state.reveal.label()),
            Style::default().fg(reveal_dim),
        ),
        Span::styled(
            format!("c cursor:{} ", state.cursor.label()),
            Style::default().fg(theme::ACCENT),
        ),
        Span::styled(
            "· 1/2/3/4/6 · v inv · f frames · q",
            Style::default().fg(theme::TEXT_DIM),
        ),
    ]);
    f.render_widget(Paragraph::new(line).style(bg), rect);
}
