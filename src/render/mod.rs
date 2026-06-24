use crate::app::App;
use crate::game::arena::{Arena, EntityKind};
use crate::game::world::WorldView;
use crate::game::writing::{Direction, TraceState};
use crate::hud::{anchor_rect, Anchor};
use crate::theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};
use std::time::Duration;
use tachyonfx::EffectManager;

/// Post-Render-Hook: treibt einen `EffectManager` gegen den Frame-Buffer.
/// Generisch über den Key-Typ `K`, damit der spätere Live-Call (Pickup-/Wellen-
/// Effekte, #31) die Key-Strategie frei wählt. Die dynamischen Notifications
/// nutzen diesen Hook nicht — sie halten ihre Effekte selbst (s. `hud::notify`).
pub fn process_effects<K: Clone + std::fmt::Debug + Ord>(
    manager: &mut EffectManager<K>,
    elapsed: Duration,
    buf: &mut Buffer,
    area: Rect,
) {
    manager.process_effects(elapsed.into(), buf, area);
}

/// Frameless full-screen-Render: die Welt füllt den ganzen Screen, HUD-Teile
/// schweben als Overlays an Ankern darüber. `elapsed` treibt die zeitbasierten
/// Notifications (deshalb `&mut App`).
pub fn draw(f: &mut Frame, app: &mut App, elapsed: Duration) {
    // Animations-Uhren render-time fortschreiben (shimmer-Phase, Cast-Welle, Pickup).
    app.anim_clock += elapsed;
    if let Some(age) = app.cast_wave.as_mut() {
        *age += elapsed;
        if age.as_secs_f32() > RING_DUR {
            app.cast_wave = None;
        }
    }
    app.advance_pickup_anim(elapsed);

    let area = f.area();
    let world = app.world_view();
    let clock = app.anim_clock;
    let cast_wave = app.cast_wave;
    let cast_mode = app.cast_mode;

    let trace: Option<(u32, usize)> = match app.trace.state {
        TraceState::Tracing { id, progress } => Some((id, progress)),
        TraceState::Idle => None,
    };

    draw_world(f, area, &world, app.arena(), clock, trace);
    draw_hud(f, area, app, &world);

    // Notifications oben-mitte, über der Welt (mutabel: halten ihre Effekte).
    app.notifications.render(f.buffer_mut(), area, elapsed);

    // Cast-Buffer-Indikator + transparenter Rainbow-Ring (render-time, über der
    // Welt; der Ring berührt nur seine Bande → Spielfeld bleibt sichtbar).
    if cast_mode {
        draw_cast_buffer(f, area, app);
    }
    if let Some(age) = cast_wave {
        let center = ((area.width / 2) as i32, (area.height / 2) as i32);
        draw_cast_ring(f.buffer_mut(), center, age, area);
    }

    if app.inventory_open() {
        draw_inventory(f, area, app);
    }

    let self_dead = world.players.iter().any(|p| p.is_self && p.is_dead);
    if self_dead {
        draw_death_overlay(f, area);
    }
    if app.debug {
        draw_debug_overlay(f, app);
    }
}

fn draw_debug_overlay(f: &mut Frame, app: &App) {
    use ratatui::widgets::Clear;
    let area = f.area();
    let w = area.width.min(60);
    let h = (app.debug_lines.len() as u16 + 5).min(area.height);
    let rect = Rect {
        x: area.width.saturating_sub(w + 1),
        y: 4,
        width: w,
        height: h,
    };
    let mut lines: Vec<Line> = Vec::new();
    if let Some(e) = app.local_engine() {
        lines.push(Line::from(Span::styled(
            format!(
                "dir={:?} word=\"{}\" cur={:?}",
                e.direction, e.current_word, e.cursor
            ),
            Style::default().fg(Color::LightCyan),
        )));
    }
    lines.push(Line::from(Span::styled(
        format!("last: {}", app.last_event),
        Style::default().fg(theme::TEXT_DIM),
    )));
    for l in &app.debug_lines {
        lines.push(Line::from(Span::styled(
            l.clone(),
            Style::default().fg(Color::Gray),
        )));
    }
    f.render_widget(Clear, rect);
    // Debug-Overlay ist ein Dev-Werkzeug (PRFH_DEBUG) und behält bewusst seinen
    // Rahmen — es ist nicht Teil der frameless Spiel-UI.
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" debug (PRFH_DEBUG) "),
    );
    f.render_widget(p, rect);
}

/// Tod-Overlay: frameless, solides Danger-Panel mittig.
fn draw_death_overlay(f: &mut Frame, area: Rect) {
    use ratatui::widgets::Clear;
    let rect = anchor_rect(area, Anchor::Center, area.width.min(40), 1);
    f.render_widget(Clear, rect);
    let p = Paragraph::new(Line::from(Span::styled(
        " ✝  Du bist tot — Respawn läuft… ",
        Style::default()
            .fg(Color::White)
            .bg(theme::DANGER)
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(ratatui::layout::Alignment::Center)
    .style(Style::default().bg(theme::DANGER));
    f.render_widget(p, rect);
}

/// HUD-Layout 1 (Ecken): `dir` oben-links, `combo` oben-rechts, Spielerliste
/// unten-links, Quit-Hinweis unten-rechts. Frameless — schwebt über der Welt.
fn draw_hud(f: &mut Frame, area: Rect, app: &App, world: &WorldView) {
    let dir = world
        .players
        .iter()
        .find(|p| p.is_self)
        .map(|p| p.direction)
        .unwrap_or(Direction::Right);
    let arrow = match dir {
        Direction::Up => "↑",
        Direction::Down => "↓",
        Direction::Left => "←",
        Direction::Right => "→",
    };
    let combo = app.local_engine().map(|e| e.combo).unwrap_or(0);

    // dir — oben-links
    let dir_line = Line::from(vec![
        Span::styled("dir ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            arrow.to_string(),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(
        Paragraph::new(dir_line),
        anchor_rect(area, Anchor::TopLeft, 8, 1),
    );

    // combo — oben-rechts
    let combo_line = Line::from(vec![
        Span::styled("combo ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("x{combo}"),
            Style::default()
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(
        Paragraph::new(combo_line),
        anchor_rect(area, Anchor::TopRight, 12, 1),
    );

    // Spielerliste — unten-links
    let mut players: Vec<Span> = world
        .players
        .iter()
        .flat_map(|p| {
            let label = if p.is_self {
                format!("{}(du) ", p.name)
            } else {
                format!("{} ", p.name)
            };
            vec![Span::styled(
                label,
                Style::default()
                    .fg(Color::Rgb(p.color.r, p.color.g, p.color.b))
                    .add_modifier(Modifier::BOLD),
            )]
        })
        .collect();
    if players.is_empty() {
        players.push(Span::raw(""));
    }
    f.render_widget(
        Paragraph::new(Line::from(players)),
        anchor_rect(area, Anchor::BottomLeft, area.width.saturating_sub(10), 1),
    );

    // Quit-Hinweis — unten-rechts
    let quit = Line::from(vec![
        Span::styled("[Esc]", Style::default().fg(theme::ACCENT)),
        Span::styled(" quit", Style::default().fg(theme::TEXT_DIM)),
    ]);
    f.render_widget(
        Paragraph::new(quit),
        anchor_rect(area, Anchor::BottomRight, 10, 1),
    );
}

/// Frameless Welt: füllt `area` komplett, cursor-zentriert. Kein Rahmen, kein
/// Titel — die HUD-Overlays liegen darüber.
fn draw_world(
    f: &mut Frame,
    area: Rect,
    world: &WorldView,
    arena: &Arena,
    clock: Duration,
    trace: Option<(u32, usize)>,
) {
    let w = area.width as i32;
    let h = area.height as i32;
    let center = (w / 2, h / 2);

    let self_player = world.players.iter().find(|p| p.is_self);
    let cursor = self_player.map(|p| p.cursor).unwrap_or((0, 0));

    let mut grid: Vec<Vec<Option<(char, Style)>>> = vec![vec![None; w as usize]; h as usize];
    let t = clock.as_secs_f32();

    // Alle Tiles aller Spieler nach tick sortieren, damit das zuletzt
    // geschriebene Tile an jeder Zelle gewinnt (fixt Host-vs-Client-Reihenfolge).
    let mut all_tiles: Vec<(
        &crate::game::writing::Tile,
        &crate::game::world::PlayerColor,
        bool, // is_self
    )> = world
        .players
        .iter()
        .flat_map(|p| p.trail.iter().map(move |t| (t, &p.color, p.is_self)))
        .collect();
    all_tiles.sort_unstable_by_key(|(t, _, _)| t.tick);

    for (tile, color, is_self) in &all_tiles {
        let rx = tile.pos.0 - cursor.0 + center.0;
        let ry = tile.pos.1 - cursor.1 + center.1;
        if rx < 0 || ry < 0 || rx >= w || ry >= h {
            continue;
        }
        // Voll verblasste Tail-Tiles rendern nichts (kein schwarzer Block, keine
        // Verdeckung anderer Spieler an derselben Zelle). Glühende Tiles immer.
        if tile.brightness == 0 && tile.glow == 0 {
            continue;
        }
        let b = tile.brightness as u64;
        let max = crate::game::writing::TILE_MAX_BRIGHTNESS as u64;
        let style = if tile.glow > 0 {
            Style::default()
                .fg(Color::LightYellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else if *is_self {
            Style::default().fg(Color::Rgb(b as u8, b as u8, b as u8))
        } else {
            let scale = |c: u8| ((c as u64 * b) / max).min(255) as u8;
            Style::default().fg(Color::Rgb(scale(color.r), scale(color.g), scale(color.b)))
        };
        grid[ry as usize][rx as usize] = Some((tile.ch, style));
    }

    // Powerup-Wörter NACH den Trails zeichnen → Top-Layer: der eigene Trail
    // überdeckt das Wort nicht mehr (Pickup-Gefühl B). Cursor-zentrierte
    // Transform wie die Tiles; jedes Tile an seiner Position, Shimmer-Idle-Look.
    for e in &arena.entities {
        match &e.kind {
            EntityKind::PowerupWord(pw) => {
                let letters: Vec<char> = pw.name.chars().collect();
                // Aktiver Trace auf GENAU diesem Wort? Dann Fortschritt + Next-Tile.
                let active = trace.filter(|(tid, _)| *tid == e.id).map(|(_, p)| p);
                let next_style = Style::default()
                    .fg(theme::HIGHLIGHT_FG)
                    .bg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD);
                let traced_style = Style::default()
                    .fg(theme::HIGHLIGHT_FG)
                    .bg(theme::HIGHLIGHT_BG)
                    .add_modifier(Modifier::BOLD);
                for (i, tile) in pw.tiles().iter().enumerate() {
                    let rx = tile.0 - cursor.0 + center.0;
                    let ry = tile.1 - cursor.1 + center.1;
                    if rx < 0 || ry < 0 || rx >= w || ry >= h {
                        continue;
                    }
                    let ch = if pw.reversed {
                        letters[letters.len() - 1 - i]
                    } else {
                        letters[i]
                    };
                    // Keystroke k landet auf keystroke_tile(k); bei reversed ist
                    // das Tile-Index n-1-k. „logischer Fortschritt" dieses Tiles:
                    let logical = if pw.reversed {
                        letters.len() - 1 - i
                    } else {
                        i
                    };
                    let style = match active {
                        Some(p) if logical < p => traced_style,
                        Some(p) if logical == p => next_style,
                        _ => shimmer_style(t, i),
                    };
                    grid[ry as usize][rx as usize] = Some((ch, style));
                }
            }
        }
    }

    // Cursor-Marker: Block-Stil in Akzentfarbe (eigener Spieler), Mitspieler in
    // ihrer Farbe.
    for player in &world.players {
        let rx = player.cursor.0 - cursor.0 + center.0;
        let ry = player.cursor.1 - cursor.1 + center.1;
        if rx < 0 || ry < 0 || rx >= w || ry >= h {
            continue;
        }
        let arrow_ch = match player.direction {
            Direction::Up => '▲',
            Direction::Down => '▼',
            Direction::Left => '◀',
            Direction::Right => '▶',
        };
        let style = if player.is_self {
            Style::default()
                .fg(theme::HIGHLIGHT_FG)
                .bg(theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Rgb(player.color.r, player.color.g, player.color.b))
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        };
        // Eigener Cursor-Pfeil wird während eines aktiven Trace unterdrückt —
        // die Next-Tile-Hervorhebung (oben) steht an seiner Stelle (Pickup C).
        let suppress_self = player.is_self && trace.is_some();
        if (player.is_self || !player.is_dead) && !suppress_self {
            grid[ry as usize][rx as usize] = Some((arrow_ch, style));
        }
    }

    let empty_style = Style::default();
    let lines: Vec<Line> = grid
        .iter()
        .map(|row| {
            let mut spans: Vec<Span> = Vec::with_capacity(row.len());
            for cell in row.iter() {
                match cell {
                    Some((ch, style)) => spans.push(Span::styled(ch.to_string(), *style)),
                    None => spans.push(Span::styled(" ".to_string(), empty_style)),
                }
            }
            Line::from(spans)
        })
        .collect();
    f.render_widget(Paragraph::new(lines), area);
}

/// Dauer der Cast-Ring-Animation (Sekunden) — snappy/dynamisch.
const RING_DUR: f32 = 0.38;

/// shimmer Idle-Style eines Powerup-Tiles: gray→white-Band, das übers Wort
/// wandert. Reine Funktion aus `(t, index)` → scroll-immun (Skill `effects`,
/// Learning #37): das Wort scrollt cursor-zentriert mit, ein tachyonfx-Zell-
/// Effekt würde über logisch andere Zeichen schmieren.
fn shimmer_style(t: f32, i: usize) -> Style {
    let phase = t * 7.0 - i as f32 * 0.95;
    let l = 0.5 + 0.5 * phase.sin();
    let v = (0x55_u16 as f32 + (0xE6 - 0x55) as f32 * l).round() as u8;
    Style::default()
        .fg(Color::Rgb(v, v, (v as u16 + 7).min(255) as u8))
        .add_modifier(Modifier::BOLD)
}

/// Linearer RGB-Lerp zwischen zwei Farben (`t` ∈ 0..1).
fn blend(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let rgb = |c: Color| -> (u8, u8, u8) {
        if let Color::Rgb(r, g, b) = c { (r, g, b) } else { (0, 0, 0) }
    };
    let (ar, ag, ab) = rgb(a);
    let (br, bg, bb) = rgb(b);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(l(ar, br), l(ag, bg), l(ab, bb))
}

/// HSL→RGB für den Rainbow-Cast-Ring (helle, pastellige Farben).
fn hsl(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
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

/// Transparenter Rainbow-Glyph-Ring (gewählte Cast-Signatur): berührt NUR die
/// expandierende Ring-Bande — alle anderen Zellen bleiben unberührt, das
/// Spielfeld bleibt sichtbar. Render-time-Math (`sqrt(dx² + 4·dy²)`, 2:1-
/// Zellaspekt) → smear-frei über scrollendem Inhalt. Heller Pastell-Regenbogen
/// nach Winkel, dünne Bande + Stipple → luftig. Bewusst KEIN tachyonfx-Effekt
/// (`evolve_into` blankt nicht-erreichte Zellen auf ' ' → verdeckt das Feld).
fn draw_cast_ring(buf: &mut Buffer, center: (i32, i32), age: Duration, area: Rect) {
    const MAXR: f32 = 17.0;
    const BAND: f32 = 1.5;
    let (cx, cy) = center;
    let p = (age.as_secs_f32() / RING_DUR).clamp(0.0, 1.0);
    let r = (1.0 - (1.0 - p) * (1.0 - p)) * MAXR; // QuadOut
    let life = 1.0 - p;
    for y in area.top() as i32..area.bottom() as i32 {
        for x in area.left() as i32..area.right() as i32 {
            let dxf = (x - cx) as f32;
            let dy = (y - cy) as f32 * 2.0;
            let d = (dxf * dxf + dy * dy).sqrt();
            let off = (d - r).abs();
            if off > BAND {
                continue;
            }
            let intensity = (1.0 - off / BAND) * life;
            if intensity < 0.12 {
                continue;
            }
            let hsh = (x as u64)
                .wrapping_mul(2_654_435_761)
                .wrapping_add((y as u64).wrapping_mul(40_503));
            if hsh % 5 < 2 {
                continue; // ~40 % Stipple → weniger dense
            }
            let hue = dy.atan2(dxf).to_degrees() + 360.0 + p * 50.0;
            let col = hsl(hue, 0.55, 0.74 + 0.12 * intensity);
            let ch = if intensity > 0.66 { '•' } else { '·' };
            if let Some(cell) = buf.cell_mut((x as u16, y as u16)) {
                cell.set_char(ch).set_fg(col);
            }
        }
    }
}

/// Cast-Buffer-Indikator (Powerup-Spec §7): gematchter Prefix im Pink-Kasten,
/// Rest gedämpft. Volles Inventar-Overlay-UI bleibt W3.
fn draw_cast_buffer(f: &mut Frame, area: Rect, app: &App) {
    let buf = &app.cast_buffer;
    // Längster Prefix-Match bestimmt den hervorgehobenen Rest (Shadow-Suffix).
    let suffix = app
        .inventory
        .prefix_matches(buf)
        .first()
        .map(|p| p.name[buf.len().min(p.name.len())..].to_string())
        .unwrap_or_default();
    let line = Line::from(vec![
        Span::styled(
            " cast ▸ ",
            Style::default()
                .fg(theme::ACCENT)
                .bg(theme::PANEL_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            buf.clone(),
            Style::default()
                .fg(theme::HIGHLIGHT_FG)
                .bg(theme::HIGHLIGHT_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            suffix,
            Style::default().fg(theme::TEXT_DIM).bg(theme::PANEL_BG),
        ),
        Span::styled(" ", Style::default().bg(theme::PANEL_BG)),
    ]);
    let rect = anchor_rect(area, Anchor::BottomCenter, 28, 1);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::PANEL_BG)),
        rect,
    );
}

/// Render-time pop-pulse Zeilen-Farbe für eine frisch eingesammelte Inventar-Zeile.
///
/// Phase `p = age / PICKUP_ANIM_DUR`:
/// - Flash-Decay `(1 - p/0.30)²`: `PICKUP_FLASH` über `ACCENT`-bg, abgeklungen bei ~30 %.
/// - Dann Doppel-Hue-Puls `(1-p)·(0.5 + 0.5·sin(4π·p))` über `PICKUP_BASE`,
///   blendend nach `TEXT` (Body-Grau) bei p=1.
///
/// Kein tachyonfx; reine render-time-Math → scroll-immun + unit-testbar.
fn popup_pulse_line(name: &str, age: Duration) -> Line<'static> {
    use crate::app::PICKUP_ANIM_DUR;
    use std::f32::consts::PI;
    let p = (age.as_secs_f32() / PICKUP_ANIM_DUR.as_secs_f32()).clamp(0.0, 1.0);
    // Flash: grelle PICKUP_FLASH-fg über ACCENT-bg, abgeklungen bis ~30 % der Dauer.
    let flash = (1.0 - p / 0.30).clamp(0.0, 1.0).powi(2);
    // Hue-Puls: zwei Wellenberge über PICKUP_BASE, die auf TEXT ausklingen.
    let pulse = ((1.0 - p) * (0.5 + 0.5 * (PI * 4.0 * p).sin())).clamp(0.0, 1.0);
    let base = blend(theme::TEXT, theme::PICKUP_BASE, pulse);
    let fg = blend(base, theme::PICKUP_FLASH, flash);
    let bg = blend(theme::PANEL_BG, theme::ACCENT, flash * 0.7);
    Line::from(Span::styled(
        format!(" {name:<8}"),
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    ))
}

/// Inventar-Overlay (§8): top-right verankert, `InvSkin::Rounded` — gerundeter
/// Rahmen, PANEL_BG-Füllung, blauer ACCENT-Titel ` POWERUPS `, §8-Atemzeilen.
/// Wächst dynamisch nach unten (1 Zeile pro Item). Liegt als Top-Overlay über
/// Welt und HUD; `Clear` räumt die Welt darunter.
fn draw_inventory(f: &mut Frame, area: Rect, app: &App) {
    const WIDTH: u16 = 34;
    // §8: 1 Blank-Zeile über Items + 1 unter Items; +2 für den Rahmen.
    let item_count = app.inventory.items.len().max(1); // „— leer —" wenn leer
    let h = (item_count as u16 + 1 + 1 + 2).min(area.height); // items + 2×blank + 2×border
    let rect = anchor_rect(area, Anchor::TopRight, WIDTH, h);

    f.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::TEXT_DIM))
        .style(Style::default().bg(theme::PANEL_BG))
        .title(Span::styled(
            " POWERUPS ",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    // §8: 1 PANEL_BG-Leerzeile über den Item-Zeilen.
    let blank = Line::from(Span::styled(" ", Style::default().bg(theme::PANEL_BG)));

    let mut lines: Vec<Line> = Vec::new();
    lines.push(blank.clone());

    if app.inventory.items.is_empty() {
        lines.push(Line::from(Span::styled(
            "  — leer —",
            Style::default().fg(theme::TEXT_DIM).bg(theme::PANEL_BG),
        )));
    } else {
        for (slot, item) in app.inventory.items.iter().enumerate() {
            // Name-Feld: feste Breite (layout-shift-invariant, relevant für Task 7).
            // Wenn diese Zeile gerade animiert (pop-pulse), Farbe render-time berechnen.
            let line = if let Some(anim) = app.pickup_anim.as_ref().filter(|a| a.slot == slot) {
                popup_pulse_line(&item.name, anim.age)
            } else {
                Line::from(Span::styled(
                    format!(" {:<8}", item.name),
                    Style::default()
                        .fg(theme::TEXT)
                        .bg(theme::PANEL_BG)
                        .add_modifier(Modifier::BOLD),
                ))
            };
            lines.push(line);
        }
    }

    // §8: 1 PANEL_BG-Leerzeile unter den Item-Zeilen.
    lines.push(blank);

    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme::PANEL_BG)),
        inner,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_string(app: &mut App) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app, Duration::ZERO)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn draw_world_renders_arena_entity_at_expected_cell() {
        use crate::game::arena::EntityKind;
        use crate::game::powerup::{Axis, PowerupWord};
        let mut app = App::new();
        // Offset vom Cursor (0,0), damit der Cursor-Marker die Entität nicht
        // überdeckt. 'z' kommt im HUD nicht vor → eindeutiger Treffer.
        // origin (5,-2), horizontal, not reversed → p_0=(5,-2) zeigt 'z'.
        app.arena_mut().unwrap().spawn(
            (5, -2),
            EntityKind::PowerupWord(PowerupWord {
                name: "zoom".into(),
                origin: (5, -2),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
        // Screen-Transform: (5,-2) - cursor(0,0) + center(40,12) = (45,10).
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app, Duration::ZERO)).unwrap();
        let buf = terminal.backend().buffer();
        assert_eq!(
            buf.cell((45, 10)).unwrap().symbol(),
            "z",
            "Powerup-Entität sollte bei (45,10) als 'z' gerendert werden"
        );
    }

    #[test]
    fn topbar_shows_only_dir_and_combo() {
        let mut app = App::new();
        let out = render_to_string(&mut app);
        assert!(out.contains("combo"), "combo fehlt in der Topbar");
        assert!(!out.contains("PULL REQUEST"), "Titel-Banner noch da");
        assert!(!out.contains("word:"), "word-Anzeige noch in der Topbar");
        assert!(!out.contains("doubt"), "doubt noch in der Topbar");
        assert!(!out.contains("day"), "day noch in der Topbar");
    }

    #[test]
    fn frameless_no_career_title_no_borders() {
        let mut app = App::new();
        let out = render_to_string(&mut app);
        assert!(!out.contains("career.md"), "career.md-Altlast noch da");
        // Keine Box-Drawing-Rahmen der Spiel-UI (Welt/HUD frameless).
        assert!(!out.contains('┌'), "Rahmen-Ecke ┌ noch da");
        assert!(!out.contains('└'), "Rahmen-Ecke └ noch da");
    }

    #[test]
    fn last_event_only_in_debug_overlay() {
        let mut app = App::new();
        app.last_event = "ZZMARKERZZ".into();

        app.debug = false;
        assert!(
            !render_to_string(&mut app).contains("ZZMARKERZZ"),
            "last_event leakt ohne PRFH_DEBUG"
        );

        app.debug = true;
        assert!(
            render_to_string(&mut app).contains("ZZMARKERZZ"),
            "last_event fehlt im Debug-Overlay"
        );
    }

    #[test]
    fn no_verbose_trigger_help() {
        let mut app = App::new();
        let out = render_to_string(&mut app);
        assert!(
            !out.contains("up down left right"),
            "verbose Trigger-Hilfe noch da"
        );
        assert!(out.contains("Esc"), "Quit-Hinweis fehlt");
    }

    #[test]
    fn live_notification_renders_many_frames_without_panic() {
        // Voller verdrahteter Pfad: Turn feuert eine Notification, dann viele
        // Frames rendern — fängt u.a. die expand-Panik (Panel-Welle am Timer-Ende).
        let mut app = App::new();
        app.on_char('u');
        app.on_char('p'); // löst TURNED-Notification aus
        assert!(!app.notifications.is_empty());
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        for _ in 0..60 {
            terminal
                .draw(|f| draw(f, &mut app, Duration::from_millis(50)))
                .unwrap();
        }
    }

    #[test]
    fn cast_flow_renders_many_frames_without_panic() {
        // Cast-Welle + Cast-Buffer + shimmer-Wort über viele Frames: darf nicht
        // paniken (render-time-Math, kein tachyonfx).
        use crate::game::arena::EntityKind;
        use crate::game::powerup::{Axis, PowerupWord};
        let mut app = App::new();
        app.arena_mut().unwrap().spawn(
            (3, 0),
            EntityKind::PowerupWord(PowerupWord {
                name: "dash".into(),
                origin: (3, 0),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
        app.cast_mode = true;
        app.cast_buffer = "da".into();
        app.cast_wave = Some(Duration::ZERO);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        for _ in 0..40 {
            terminal
                .draw(|f| draw(f, &mut app, Duration::from_millis(50)))
                .unwrap();
        }
    }

    #[test]
    fn process_effects_hook_drives_manager_without_panic() {
        use crate::effects;
        use ratatui::buffer::Buffer;
        use std::time::Duration;
        use tachyonfx::EffectManager;

        let mut mgr: EffectManager<()> = EffectManager::default();
        mgr.add_effect(effects::pickup());

        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 12));
        let area = buf.area;
        for _ in 0..40 {
            process_effects(&mut mgr, Duration::from_millis(50), &mut buf, area);
        }
    }

    #[test]
    fn powerup_word_not_hidden_by_trail_tile() {
        use crate::app::Mode;
        use crate::game::arena::EntityKind;
        use crate::game::powerup::{Axis, PowerupWord};
        use crate::game::writing::{Tile, TILE_MAX_BRIGHTNESS};
        let mut app = App::new();
        // Wort offset vom Cursor (Cursor-Marker soll nicht stören): origin (5,-2).
        app.arena_mut().unwrap().spawn(
            (5, -2),
            EntityKind::PowerupWord(PowerupWord {
                name: "zoom".into(),
                origin: (5, -2),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
        // Ein Trail-Tile genau AUF das erste Wort-Tile (5,-2) legen.
        if let Mode::Single(e, _) = &mut app.mode {
            e.trail.push(Tile {
                pos: (5, -2),
                ch: 'Q',
                tick: 99,
                glow: 0,
                brightness: TILE_MAX_BRIGHTNESS,
                written_pace: 0.0,
            });
        }
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app, Duration::ZERO)).unwrap();
        let buf = terminal.backend().buffer();
        // Screen-Transform: (5,-2) - (0,0) + (40,12) = (45,10).
        assert_eq!(
            buf.cell((45, 10)).unwrap().symbol(),
            "z",
            "Powerup-Wort muss über dem Trail liegen (Top-Layer)"
        );
    }

    #[test]
    fn trace_feedback_colors_forward_word() {
        // Nicht-reversed "dash": logical == physischer Index i. progress=2 →
        // i0,i1 getraced (HIGHLIGHT_BG), i2 next (ACCENT), i3 shimmer (weder/noch).
        use crate::game::arena::EntityKind;
        use crate::game::powerup::{Axis, PowerupWord};
        use crate::game::writing::TraceState;
        let mut app = App::new();
        app.arena_mut().unwrap().spawn(
            (5, -2),
            EntityKind::PowerupWord(PowerupWord {
                name: "dash".into(),
                origin: (5, -2),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
        let id = app.arena().entities[0].id;
        app.trace.state = TraceState::Tracing { id, progress: 2 };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app, Duration::ZERO)).unwrap();
        let buf = terminal.backend().buffer();

        // Screen-Transform: (5,-2) - cursor(0,0) + center(40,12) = (45,10);
        // Tiles liegen aufsteigend ab origin → 45,46,47,48 für i=0..3.
        assert_eq!(
            buf.cell((45, 10)).unwrap().bg,
            theme::HIGHLIGHT_BG,
            "i0 (logical0 < progress2) sollte getraced (HIGHLIGHT_BG) sein"
        );
        assert_eq!(
            buf.cell((46, 10)).unwrap().bg,
            theme::HIGHLIGHT_BG,
            "i1 (logical1 < progress2) sollte getraced (HIGHLIGHT_BG) sein"
        );
        assert_eq!(
            buf.cell((47, 10)).unwrap().bg,
            theme::ACCENT,
            "i2 (logical2 == progress2) sollte Next-Tile (ACCENT) sein"
        );
        let shimmer_bg = buf.cell((48, 10)).unwrap().bg;
        assert_ne!(
            shimmer_bg,
            theme::HIGHLIGHT_BG,
            "i3 (logical3 > progress2) sollte NICHT getraced sein"
        );
        assert_ne!(
            shimmer_bg,
            theme::ACCENT,
            "i3 (logical3 > progress2) sollte NICHT Next-Tile sein"
        );
    }

    #[test]
    fn trace_feedback_colors_reversed_word() {
        // Reversed "dash": physisches Tile i zeigt letters[n-1-i], logical = n-1-i.
        // progress=2 → i0 logical3 shimmer, i1 logical2 next (ACCENT),
        // i2 logical1 getraced (HIGHLIGHT_BG). Das ist die riskante Index-Math.
        use crate::game::arena::EntityKind;
        use crate::game::powerup::{Axis, PowerupWord};
        use crate::game::writing::TraceState;
        let mut app = App::new();
        app.arena_mut().unwrap().spawn(
            (5, -2),
            EntityKind::PowerupWord(PowerupWord {
                name: "dash".into(),
                origin: (5, -2),
                axis: Axis::Horizontal,
                reversed: true,
            }),
        );
        let id = app.arena().entities[0].id;
        app.trace.state = TraceState::Tracing { id, progress: 2 };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app, Duration::ZERO)).unwrap();
        let buf = terminal.backend().buffer();

        // Screen-Transform wie oben: physische Tiles 45,46,47,48 für i=0..3.
        let shimmer_bg = buf.cell((45, 10)).unwrap().bg;
        assert_ne!(
            shimmer_bg,
            theme::HIGHLIGHT_BG,
            "i0 (logical3 > progress2) sollte NICHT getraced sein"
        );
        assert_ne!(
            shimmer_bg,
            theme::ACCENT,
            "i0 (logical3 > progress2) sollte NICHT Next-Tile sein"
        );
        assert_eq!(
            buf.cell((46, 10)).unwrap().bg,
            theme::ACCENT,
            "i1 (logical2 == progress2) sollte Next-Tile (ACCENT) sein"
        );
        assert_eq!(
            buf.cell((47, 10)).unwrap().bg,
            theme::HIGHLIGHT_BG,
            "i2 (logical1 < progress2) sollte getraced (HIGHLIGHT_BG) sein"
        );
    }

    #[test]
    fn draw_inventory_renders_without_panic_when_open() {
        let mut app = App::new_single();
        app.inventory.add(crate::game::powerup::Powerup {
            id: 1,
            name: "dash".into(),
            effect_tag: crate::game::powerup::EffectTag::Test,
        });
        assert!(app.inventory_open());
        // Ganzer draw-Pfad darf nicht paniken (Inventar oben rechts, dynamische Höhe).
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::render::draw(f, &mut app, std::time::Duration::from_millis(16)))
            .unwrap();
    }

    #[test]
    fn trace_feedback_renders_many_frames_without_panic() {
        // Aktiver Trace-State (getractes Wort + Next-Tile-Highlight + unterdrückter
        // Cursor) über viele Frames: reine render-time-Math, darf nicht paniken.
        use crate::game::arena::EntityKind;
        use crate::game::powerup::{Axis, PowerupWord};
        use crate::game::writing::TraceState;
        let mut app = App::new();
        app.arena_mut().unwrap().spawn(
            (3, 0),
            EntityKind::PowerupWord(PowerupWord {
                name: "dash".into(),
                origin: (3, 0),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
        // Trace mitten im Wort: id muss zur gespawnten Entität passen.
        let id = app.arena().entities[0].id;
        app.trace.state = TraceState::Tracing { id, progress: 2 };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        for _ in 0..40 {
            terminal
                .draw(|f| draw(f, &mut app, Duration::from_millis(50)))
                .unwrap();
        }
    }

    #[test]
    fn pickup_anim_renders_and_clears_without_panic() {
        use crate::game::powerup::{EffectEvent, EffectTag, Powerup};
        let mut app = App::new_single();
        app.inventory.add(Powerup { id: 1, name: "dash".into(), effect_tag: EffectTag::Test });
        app.apply_effect_event(EffectEvent::Pickup { slot: 0, name: "dash".into() });
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        // mehrere Frames über die Anim-Dauer hinaus — darf nicht paniken, Anim klärt
        for _ in 0..50 {
            terminal
                .draw(|f| crate::render::draw(f, &mut app, std::time::Duration::from_millis(16)))
                .unwrap();
        }
        assert!(app.pickup_anim.is_none(), "Anim nach Ablauf geräumt");
    }
}
