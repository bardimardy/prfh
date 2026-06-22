use crate::app::App;
use crate::game::world::WorldView;
use crate::game::writing::{buffer_ends_with_trigger, Direction};
use ratatui::{
    layout::{Constraint, Direction as LayoutDirection, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(5),
        ])
        .split(f.area());

    let world = app.world_view();
    draw_hud(f, chunks[0], app, &world);
    draw_banner(f, chunks[1], app);
    draw_world(f, chunks[2], &world);
    draw_bottom(f, chunks[3], app, &world);

    if app.debug {
        draw_debug_overlay(f, app);
    }
}

fn draw_debug_overlay(f: &mut Frame, app: &App) {
    use ratatui::widgets::Clear;
    let area = f.area();
    let w = area.width.min(60);
    let h = (app.debug_lines.len() as u16 + 4).min(area.height);
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

    let (word_display, word_is_trigger, combo, doubt) = match app.local_engine() {
        Some(e) => (
            if e.current_word.is_empty() {
                "—".to_string()
            } else {
                e.current_word.clone()
            },
            buffer_ends_with_trigger(&e.current_word),
            e.combo,
            e.doubt,
        ),
        None => ("—".to_string(), false, 0, 0),
    };
    let word_color = if word_is_trigger {
        Color::LightGreen
    } else {
        Color::DarkGray
    };

    let hud = Paragraph::new(Line::from(vec![
        Span::styled(
            " PULL REQUEST FROM HELL ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("dir {} ", arrow),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  word: "),
        Span::styled(
            word_display,
            Style::default().fg(word_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("combo x{}", combo),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw("  "),
        Span::styled(
            format!("doubt {}", doubt),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  "),
        Span::styled(
            format!("day {}", app.day),
            Style::default().fg(Color::Yellow),
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

    // newest tick across all trails, for fade reference
    let now = world
        .players
        .iter()
        .flat_map(|p| p.trail.iter().map(|t| t.tick))
        .max()
        .unwrap_or(0);

    const FADE_PER_TICK: u64 = 2;
    const MAX_BRIGHTNESS: u64 = 200;
    const MIN_BRIGHTNESS: u64 = 60;

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
        let age = now.saturating_sub(tile.tick);
        let b = MAX_BRIGHTNESS
            .saturating_sub(age.saturating_mul(FADE_PER_TICK))
            .max(MIN_BRIGHTNESS);
        let style = if tile.glow > 0 {
            Style::default()
                .fg(Color::LightYellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else if *is_self {
            Style::default().fg(Color::Rgb(b as u8, b as u8, b as u8))
        } else {
            let scale = |c: u8| ((c as u64 * b) / MAX_BRIGHTNESS).min(255) as u8;
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

fn draw_bottom(f: &mut Frame, area: Rect, app: &App, world: &WorldView) {
    let roster: Vec<Span> = world
        .players
        .iter()
        .flat_map(|p| {
            let label = if p.is_self {
                format!("{}(du)", p.name)
            } else {
                p.name.clone()
            };
            vec![Span::styled(
                format!("{} ", label),
                Style::default()
                    .fg(Color::Rgb(p.color.r, p.color.g, p.color.b))
                    .add_modifier(Modifier::BOLD),
            )]
        })
        .collect();

    let inner_lines = vec![
        Line::from(roster),
        Line::from(Span::styled(
            app.last_event.as_str(),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
            Span::raw(" quit  "),
            Span::raw("triggers: "),
            Span::styled(
                "up down left right back stop",
                Style::default().fg(Color::Yellow),
            ),
        ]),
    ];

    let p = Paragraph::new(inner_lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
