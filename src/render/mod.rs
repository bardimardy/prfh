use crate::app::{App, Scene};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    match app.scene {
        Scene::Boot => draw_boot(f),
        Scene::Menu | Scene::Shell | Scene::Day0 => draw_shell(f, app),
    }
}

fn draw_boot(f: &mut Frame) {
    let area = f.area();
    let text = Paragraph::new("Loading career...")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(text, area);
}

fn draw_shell(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header / HUD
    let hud = Paragraph::new(Line::from(vec![
        Span::styled(
            " PULL REQUEST FROM HELL ",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Day {}", app.day),
            Style::default().fg(Color::Yellow),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(hud, chunks[0]);

    // History
    let lines: Vec<Line> = app
        .history
        .iter()
        .map(|l| Line::from(l.as_str()))
        .collect();
    let history = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(" terminal "));
    f.render_widget(history, chunks[1]);

    // Prompt
    let prompt = Paragraph::new(Line::from(vec![
        Span::styled("$ ", Style::default().fg(Color::Green)),
        Span::raw(&app.input_buffer),
        Span::styled(
            "_",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(prompt, chunks[2]);
}
