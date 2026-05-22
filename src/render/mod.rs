use crate::app::{App, Mode};
use crate::game::writing::{is_trigger_word, Direction};
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
            Constraint::Min(5),
            Constraint::Length(5),
        ])
        .split(f.area());

    draw_hud(f, chunks[0], app);
    draw_world(f, chunks[1], app);
    draw_bottom(f, chunks[2], app);
}

fn draw_hud(f: &mut Frame, area: Rect, app: &App) {
    let mode_label = match app.mode {
        Mode::World => "WORLD",
        Mode::Shell => "SHELL",
    };
    let mode_color = match app.mode {
        Mode::World => Color::Green,
        Mode::Shell => Color::Cyan,
    };
    let arrow = match app.writing.direction {
        Direction::Up => "↑",
        Direction::Down => "↓",
        Direction::Left => "←",
        Direction::Right => "→",
    };

    let word = &app.writing.current_word;
    let word_is_trigger = is_trigger_word(word);
    let word_color = if word_is_trigger {
        Color::LightGreen
    } else {
        Color::DarkGray
    };
    let word_display = if word.is_empty() {
        "—".to_string()
    } else {
        word.clone()
    };

    let hud = Paragraph::new(Line::from(vec![
        Span::styled(
            " PULL REQUEST FROM HELL ",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(format!("[{}]", mode_label), Style::default().fg(mode_color)),
        Span::raw("  "),
        Span::styled(format!("dir {} ", arrow), Style::default().fg(Color::Yellow)),
        Span::raw("  word: "),
        Span::styled(word_display, Style::default().fg(word_color).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            format!("combo x{}", app.writing.combo),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw("  "),
        Span::styled(
            format!("doubt {}", app.writing.doubt),
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

fn draw_world(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" /work/repo/career.md ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    if matches!(app.mode, Mode::Shell) {
        let lines: Vec<Line> = app
            .shell_history
            .iter()
            .map(|l| Line::from(l.as_str()))
            .collect();
        let history = Paragraph::new(lines).wrap(Wrap { trim: false });
        f.render_widget(history, inner);
        return;
    }

    // Render the writing trail in world-space, centered on cursor
    let w = inner.width as i32;
    let h = inner.height as i32;
    let center = (w / 2, h / 2);
    let cursor = app.writing.cursor;

    let mut grid: Vec<Vec<char>> = vec![vec![' '; w as usize]; h as usize];

    for tile in &app.writing.trail {
        let rx = tile.pos.0 - cursor.0 + center.0;
        let ry = tile.pos.1 - cursor.1 + center.1;
        if rx >= 0 && ry >= 0 && rx < w && ry < h {
            grid[ry as usize][rx as usize] = tile.ch;
        }
    }

    // Direction-indicator glyph (sits at cursor center as an inverted cell)
    let arrow_ch = match app.writing.direction {
        Direction::Up => '▲',
        Direction::Down => '▼',
        Direction::Left => '◀',
        Direction::Right => '▶',
    };

    let cursor_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let trail_style = Style::default().fg(Color::Gray);

    let lines: Vec<Line> = grid
        .iter()
        .enumerate()
        .map(|(y, row)| {
            let mut spans: Vec<Span> = Vec::with_capacity(row.len());
            for (x, &ch) in row.iter().enumerate() {
                if x as i32 == center.0 && y as i32 == center.1 {
                    spans.push(Span::styled(arrow_ch.to_string(), cursor_style));
                } else {
                    let s: String = ch.to_string();
                    spans.push(Span::styled(s, trail_style));
                }
            }
            Line::from(spans)
        })
        .collect();
    let world = Paragraph::new(lines);
    f.render_widget(world, inner);
}

fn draw_bottom(f: &mut Frame, area: Rect, app: &App) {
    let inner_lines = match app.mode {
        Mode::World => vec![
            Line::from(Span::styled(
                app.last_event.as_str(),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(vec![
                Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
                Span::raw(" fire trigger  "),
                Span::styled("[Tab]", Style::default().fg(Color::Cyan)),
                Span::raw(" shell  "),
                Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
                Span::raw(" quit  "),
                Span::raw("triggers (need space/punct/⏎ after): "),
                Span::styled("up down left right back stop", Style::default().fg(Color::Yellow)),
            ]),
        ],
        Mode::Shell => vec![Line::from(vec![
            Span::styled("$ ", Style::default().fg(Color::Green)),
            Span::raw(&app.shell_buffer),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ])],
    };

    let p = Paragraph::new(inner_lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
