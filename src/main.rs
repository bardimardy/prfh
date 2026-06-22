use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};

use prfh::{app::App, render};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> Result<()> {
    let debug = std::env::var("PRFH_DEBUG").is_ok();
    let mut app = App::new();
    app.debug = debug;

    while !app.should_quit {
        terminal.draw(|f| render::draw(f, &app))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                // Strict: only Press. Warp.dev and some terminals duplicate keys
                // via Repeat events, which would double-type every char and break
                // immediate-mode trigger detection.
                if key.kind != KeyEventKind::Press {
                    if debug {
                        app.debug_log(format!(
                            "ignored {:?} {:?} mods={:?}",
                            key.kind, key.code, key.modifiers
                        ));
                    }
                    continue;
                }
                if debug {
                    app.debug_log(format!("recv {:?} mods={:?}", key.code, key.modifiers));
                }
                match key.code {
                    KeyCode::Esc => app.should_quit = true,
                    KeyCode::Char(c) => app.on_char(c),
                    KeyCode::Backspace => app.on_backspace(),
                    KeyCode::Enter => app.on_enter(),
                    _ => {}
                }
            }
        }

        app.tick();
    }

    Ok(())
}
