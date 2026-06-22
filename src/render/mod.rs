use crate::app::App;
use crate::game::writing::Direction;
use crate::theme;
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

    draw_hud(f, chunks[0], app);
    draw_banner(f, chunks[1], app);
    draw_world(f, chunks[2], app);
    draw_bottom(f, chunks[3], app);

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
    lines.push(Line::from(Span::styled(
        format!(
            "dir={:?} word=\"{}\" cur={:?}",
            app.writing.direction, app.writing.current_word, app.writing.cursor
        ),
        Style::default().fg(Color::LightCyan),
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

fn draw_hud(f: &mut Frame, area: Rect, app: &App) {
    let arrow = match app.writing.direction {
        Direction::Up => "↑",
        Direction::Down => "↓",
        Direction::Left => "←",
        Direction::Right => "→",
    };

    let hud = Paragraph::new(Line::from(vec![
        Span::styled("dir ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("{arrow} "),
            Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("combo ", Style::default().fg(theme::TEXT_DIM)),
        Span::styled(
            format!("x{}", app.writing.combo),
            Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD),
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

    // Render the writing trail in world-space, centered on cursor
    let w = inner.width as i32;
    let h = inner.height as i32;
    let center = (w / 2, h / 2);
    let cursor = app.writing.cursor;

    // Per-cell glyph + optional style. None = empty space.
    let mut grid: Vec<Vec<Option<(char, Style)>>> = vec![vec![None; w as usize]; h as usize];

    let now = app.writing.tick;
    // Fade math: brightness drops by FADE_PER_TICK per tick down to MIN_BRIGHTNESS.
    const FADE_PER_TICK: u64 = 2;
    const MAX_BRIGHTNESS: u64 = 200;
    const MIN_BRIGHTNESS: u64 = 60;

    for tile in &app.writing.trail {
        let rx = tile.pos.0 - cursor.0 + center.0;
        let ry = tile.pos.1 - cursor.1 + center.1;
        if rx < 0 || ry < 0 || rx >= w || ry >= h {
            continue;
        }
        let style = if tile.glow > 0 {
            Style::default()
                .fg(Color::LightYellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            let age = now.saturating_sub(tile.tick);
            let b = MAX_BRIGHTNESS
                .saturating_sub(age.saturating_mul(FADE_PER_TICK))
                .max(MIN_BRIGHTNESS) as u8;
            Style::default().fg(Color::Rgb(b, b, b))
        };
        // Later tiles overwrite earlier ones on the same cell (covers overwrite-on-stop).
        grid[ry as usize][rx as usize] = Some((tile.ch, style));
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
    let empty_style = Style::default();

    let lines: Vec<Line> = grid
        .iter()
        .enumerate()
        .map(|(y, row)| {
            let mut spans: Vec<Span> = Vec::with_capacity(row.len());
            for (x, cell) in row.iter().enumerate() {
                if x as i32 == center.0 && y as i32 == center.1 {
                    spans.push(Span::styled(arrow_ch.to_string(), cursor_style));
                } else if let Some((ch, style)) = cell {
                    spans.push(Span::styled(ch.to_string(), *style));
                } else {
                    spans.push(Span::styled(" ".to_string(), empty_style));
                }
            }
            Line::from(spans)
        })
        .collect();
    let world = Paragraph::new(lines);
    f.render_widget(world, inner);
}

fn draw_bottom(f: &mut Frame, area: Rect, app: &App) {
    let inner_lines = vec![
        Line::from(Span::styled(
            app.last_event.as_str(),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
            Span::raw(" quit  "),
            Span::raw("triggers fire immediately: "),
            Span::styled("up down left right back stop", Style::default().fg(Color::Yellow)),
        ]),
    ];

    let p = Paragraph::new(inner_lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
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
}
