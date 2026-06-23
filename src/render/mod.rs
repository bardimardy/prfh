use crate::app::App;
use crate::game::world::WorldView;
use crate::game::writing::Direction;
use crate::theme;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction as LayoutDirection, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::time::Duration;
use tachyonfx::EffectManager;

/// Post-Render-Hook: treibt einen `EffectManager` gegen den Frame-Buffer.
/// Generisch über den Key-Typ `K`, damit der spätere Live-Call (C, #31) die
/// Key-Strategie frei wählt — diese Phase legt KEINE `App`-Felder an. In `draw`
/// wird das später als `process_effects(mgr, elapsed, f.buffer_mut(), area)`
/// aufgerufen; hier nur die wiederverwendbare, testbare Funktion.
pub fn process_effects<K: Clone + std::fmt::Debug + Ord>(
    manager: &mut EffectManager<K>,
    elapsed: Duration,
    buf: &mut Buffer,
    area: Rect,
) {
    manager.process_effects(elapsed.into(), buf, area);
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(f.area());

    let world = app.world_view();
    draw_hud(f, chunks[0], app, &world);
    draw_banner(f, chunks[1], app);
    draw_world(f, chunks[2], &world);
    draw_bottom(f, chunks[3], app, &world);

    let self_dead = world.players.iter().any(|p| p.is_self && p.is_dead);
    if self_dead {
        draw_death_overlay(f, app);
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
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" debug (PRFH_DEBUG) "),
    );
    f.render_widget(p, rect);
}

fn draw_banner(f: &mut Frame, area: Rect, app: &App) {
    if let Some(msg) = &app.trigger_banner {
        let p = Paragraph::new(Line::from(Span::styled(
            msg.clone(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
    }
}

fn draw_death_overlay(f: &mut Frame, _app: &App) {
    use ratatui::widgets::Clear;
    let area = f.area();
    let w = area.width.min(40);
    let h = 3u16;
    let rect = Rect {
        x: area.width.saturating_sub(w) / 2,
        y: area.height / 2 - 1,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let p = Paragraph::new(Line::from(Span::styled(
        " ✝  Du bist tot — Respawn läuft… ",
        Style::default()
            .fg(Color::White)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(ratatui::layout::Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White).bg(Color::Red)),
    );
    f.render_widget(p, rect);
}

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

    let hud = Paragraph::new(Line::from(vec![
        Span::styled("dir ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("{arrow} "),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("combo ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("x{}", combo),
            Style::default()
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(hud, area);
}

fn draw_world(f: &mut Frame, area: Rect, world: &WorldView) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" /work/repo/career.md ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let w = inner.width as i32;
    let h = inner.height as i32;
    let center = (w / 2, h / 2);

    let self_player = world.players.iter().find(|p| p.is_self);
    let cursor = self_player.map(|p| p.cursor).unwrap_or((0, 0));

    let mut grid: Vec<Vec<Option<(char, Style)>>> = vec![vec![None; w as usize]; h as usize];

    // Collect all tiles across all players and sort by tick so the most
    // recently written tile always wins at any given cell, regardless of
    // which player wrote it (fixes host-vs-client render order).
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
        // Fully-faded tail tiles render nothing — no black block, and they don't
        // occlude other players' tiles at the same cell. Glowing tiles always show.
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

    // Cursor marker: only rendered for the local player.
    for player in &world.players {
        if !player.is_self {
            continue;
        }
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
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Rgb(player.color.r, player.color.g, player.color.b))
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        };
        grid[ry as usize][rx as usize] = Some((arrow_ch, style));
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
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_bottom(f: &mut Frame, area: Rect, _app: &App, world: &WorldView) {
    let mut spans: Vec<Span> = world
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
    spans.push(Span::styled("  [Esc]", Style::default().fg(theme::ACCENT)));
    spans.push(Span::styled(" quit", Style::default().fg(theme::TEXT_DIM)));

    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_string(app: &App) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn topbar_shows_only_dir_and_combo() {
        let app = App::new();
        let out = render_to_string(&app);
        // combo bleibt sichtbar ...
        assert!(out.contains("combo"), "combo fehlt in der Topbar");
        // ... aber der ganze Altbestand ist raus:
        assert!(!out.contains("PULL REQUEST"), "Titel-Banner noch da");
        assert!(!out.contains("word:"), "word-Anzeige noch in der Topbar");
        assert!(!out.contains("doubt"), "doubt noch in der Topbar");
        assert!(!out.contains("day"), "day noch in der Topbar");
    }

    #[test]
    fn last_event_only_in_debug_overlay() {
        // Sichtbarer, eindeutiger Marker als last_event.
        let mut app = App::new();
        app.last_event = "ZZMARKERZZ".into();

        // Ohne Debug: Marker darf nirgends auftauchen.
        app.debug = false;
        assert!(
            !render_to_string(&app).contains("ZZMARKERZZ"),
            "last_event leakt ohne PRFH_DEBUG"
        );

        // Mit Debug: Marker erscheint im Overlay.
        app.debug = true;
        assert!(
            render_to_string(&app).contains("ZZMARKERZZ"),
            "last_event fehlt im Debug-Overlay"
        );
    }

    #[test]
    fn no_verbose_trigger_help() {
        let app = App::new();
        let out = render_to_string(&app);
        assert!(
            !out.contains("up down left right"),
            "verbose Trigger-Hilfe noch da"
        );
        assert!(out.contains("Esc"), "Quit-Hinweis fehlt");
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
}
