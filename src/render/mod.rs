use crate::app::App;
use crate::game::world::WorldView;
use crate::game::writing::Direction;
use crate::hud::{anchor_rect, Anchor};
use crate::theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
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
    let area = f.area();
    let world = app.world_view();

    draw_world(f, area, &world);
    draw_hud(f, area, app, &world);

    // Notifications oben-mitte, über der Welt (mutabel: halten ihre Effekte).
    app.notifications.render(f.buffer_mut(), area, elapsed);

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
            Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(dir_line), anchor_rect(area, Anchor::TopLeft, 8, 1));

    // combo — oben-rechts
    let combo_line = Line::from(vec![
        Span::styled("combo ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("x{combo}"),
            Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD),
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
    f.render_widget(Paragraph::new(quit), anchor_rect(area, Anchor::BottomRight, 10, 1));
}

/// Frameless Welt: füllt `area` komplett, cursor-zentriert. Kein Rahmen, kein
/// Titel — die HUD-Overlays liegen darüber.
fn draw_world(f: &mut Frame, area: Rect, world: &WorldView) {
    let w = area.width as i32;
    let h = area.height as i32;
    let center = (w / 2, h / 2);

    let self_player = world.players.iter().find(|p| p.is_self);
    let cursor = self_player.map(|p| p.cursor).unwrap_or((0, 0));

    let mut grid: Vec<Vec<Option<(char, Style)>>> = vec![vec![None; w as usize]; h as usize];

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
        if player.is_self || !player.is_dead {
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_string(app: &mut App) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, app, Duration::ZERO))
            .unwrap();
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
