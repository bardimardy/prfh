use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    time::{Duration, Instant},
};

use prfh::{
    app::{App, Mode},
    render,
};

/// Resolve an optional join address before entering raw mode.
/// Prints a status line to stdout, then either returns the resolved
/// `"host:port"` string or exits the process with an error message.
fn resolve_join_addr(raw_addr: Option<String>) -> String {
    use prfh::net::discovery::{discover, TCP_PORT};
    use std::time::Duration as StdDuration;

    match raw_addr {
        Some(a) if a.contains(':') => a,
        Some(a) => format!("{}:{}", a, TCP_PORT),
        None => {
            println!("Suche nach Spielen im LAN...");
            let found = discover(StdDuration::from_secs(2));
            if found.is_empty() {
                eprintln!("Keine Spiele im LAN gefunden. Nutze `prfh join <ip>`.");
                std::process::exit(1);
            }
            let e = &found[0];
            format!("{}:{}", e.addr, e.tcp_port)
        }
    }
}

enum Cli {
    Single,
    Host { name: String },
    Join { addr: String, name: String },
}

fn parse_cli() -> Cli {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name_of = |args: &[String]| -> String {
        args.iter()
            .position(|a| a == "--name")
            .and_then(|i| args.get(i + 1).cloned())
            .unwrap_or_default()
    };
    match args.first().map(|s| s.as_str()) {
        Some("host") => Cli::Host {
            name: {
                let n = name_of(&args);
                if n.is_empty() {
                    "Host".into()
                } else {
                    n
                }
            },
        },
        Some("join") => {
            let raw_addr = args.get(1).filter(|a| !a.starts_with("--")).cloned();
            let name = name_of(&args);
            let addr = resolve_join_addr(raw_addr);
            Cli::Join {
                addr,
                name: if name.is_empty() {
                    "Player".into()
                } else {
                    name
                },
            }
        }
        _ => Cli::Single,
    }
}

fn main() -> Result<()> {
    let cli = parse_cli();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let debug = std::env::var("PRFH_DEBUG").is_ok();

    let result = match cli {
        Cli::Single => run(&mut terminal, debug),
        Cli::Host { name } => run_host(&mut terminal, name, debug),
        Cli::Join { addr, name } => run_client(&mut terminal, addr, name, debug),
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, debug: bool) -> Result<()>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let mut app = App::new_single();
    app.debug = debug;
    let mut last_draw = Instant::now();

    while !app.should_quit {
        let elapsed = last_draw.elapsed();
        last_draw = Instant::now();
        terminal.draw(|f| render::draw(f, &mut app, elapsed))?;

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
                    KeyCode::Tab => app.toggle_cast(),
                    KeyCode::Char('`') => app.toggle_inventory(),
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

fn run_client<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    addr: String,
    name: String,
    debug: bool,
) -> Result<()>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    use prfh::net::client::connect;
    use prfh::net::protocol::{InputEvent, ServerMsg};

    let name = if name.is_empty() {
        "Player".into()
    } else {
        name
    };
    let (world, arena, mut handle) = connect(&addr, &name)?;
    let mut app = App::new_with_mode(Mode::Client(world, arena));
    app.debug = debug;
    let mut last_draw = Instant::now();

    while !app.should_quit {
        let elapsed = last_draw.elapsed();
        last_draw = Instant::now();
        terminal.draw(|f| render::draw(f, &mut app, elapsed))?;

        let self_is_dead = app
            .world_view()
            .players
            .iter()
            .find(|p| p.is_self)
            .map(|p| p.is_dead)
            .unwrap_or(false);

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc => app.should_quit = true,
                        KeyCode::Char(' ') => {}
                        KeyCode::Char(c) if !self_is_dead => handle.send_input(InputEvent::Char(c)),
                        KeyCode::Backspace if !self_is_dead => {
                            handle.send_input(InputEvent::Backspace)
                        }
                        _ => {}
                    }
                }
            }
        }

        let mut host_gone = false;
        loop {
            match handle.rx.try_recv() {
                Ok(msg) => {
                    if let Mode::Client(w, arena) = &mut app.mode {
                        match msg {
                            ServerMsg::EntitySpawned { entity } => arena.apply_spawned(entity),
                            ServerMsg::EntityDespawned { id } => arena.apply_despawned(id),
                            other => w.apply(other),
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    host_gone = true;
                    break;
                }
            }
        }
        if host_gone {
            app.last_event = "Host getrennt — beende.".into();
            app.should_quit = true;
        }

        app.tick();
    }
    Ok(())
}

fn run_host<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    name: String,
    debug: bool,
) -> Result<()>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    use prfh::net::discovery::TCP_PORT;
    use prfh::net::protocol::{InputEvent, ServerMsg};
    use prfh::net::server::{send_msg, spawn_listener, HostEvent, HostState, HOST_ID};
    use std::collections::HashMap;
    use std::net::TcpListener;

    let listener = TcpListener::bind(("0.0.0.0", TCP_PORT))?;
    listener.set_nonblocking(false)?;
    let rx = spawn_listener(listener);
    let announce_name = name.clone();
    prfh::net::discovery::spawn_announce(announce_name, TCP_PORT);

    let mut streams: HashMap<u64, std::net::TcpStream> = HashMap::new();
    let mut conn_player: HashMap<u64, prfh::game::world::PlayerId> = HashMap::new();

    let mut app = App::new_with_mode(Mode::Host(HostState::new(name)));
    app.debug = debug;
    let mut last_draw = Instant::now();

    while !app.should_quit {
        let elapsed = last_draw.elapsed();
        last_draw = Instant::now();
        terminal.draw(|f| render::draw(f, &mut app, elapsed))?;

        // (a) local input
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Mode::Host(h) = &mut app.mode {
                        let ev = match key.code {
                            KeyCode::Esc => {
                                app.should_quit = true;
                                None
                            }
                            KeyCode::Char(' ') => None,
                            KeyCode::Char(c) => Some(InputEvent::Char(c)),
                            KeyCode::Backspace => Some(InputEvent::Backspace),
                            _ => None,
                        };
                        if let Some(ev) = ev {
                            if let Some(msg) = h.apply_input(HOST_ID, ev) {
                                broadcast(&mut streams, None, &msg);
                            }
                        }
                    }
                }
            }
        }

        // (b) network events
        while let Ok(ev) = rx.try_recv() {
            if let Mode::Host(h) = &mut app.mode {
                match ev {
                    HostEvent::Hello {
                        conn_id,
                        name,
                        mut write,
                    } => match h.add_player(name) {
                        Ok(outcome) => {
                            let _ = send_msg(&mut write, &outcome.welcome);
                            conn_player.insert(conn_id, outcome.id);
                            streams.insert(conn_id, write);
                            broadcast(&mut streams, Some(conn_id), &outcome.joined);
                        }
                        Err(reason) => {
                            let _ = send_msg(&mut write, &ServerMsg::Reject { reason });
                        }
                    },
                    HostEvent::Input { conn_id, ev } => {
                        if let Some(&pid) = conn_player.get(&conn_id) {
                            if let Some(msg) = h.apply_input(pid, ev) {
                                broadcast(&mut streams, None, &msg);
                            }
                        }
                    }
                    HostEvent::Disconnected { conn_id } => {
                        // Rejected or duplicate connections were never added to
                        // conn_player, so the guard safely falls through for them.
                        if let Some(pid) = conn_player.remove(&conn_id) {
                            streams.remove(&conn_id);
                            if let Some(msg) = h.remove_player(pid) {
                                broadcast(&mut streams, None, &msg);
                            }
                        }
                    }
                }
            }
        }

        // (c) tick visuals + broadcast any respawn events
        if let Mode::Host(h) = &mut app.mode {
            for msg in h.tick_visuals() {
                broadcast(&mut streams, None, &msg);
            }
        }
        app.tick(); // visual state (Host hat tick_visuals oben schon aufgerufen → No-Op)
    }
    Ok(())
}

fn broadcast(
    streams: &mut std::collections::HashMap<u64, std::net::TcpStream>,
    exclude: Option<u64>,
    msg: &prfh::net::protocol::ServerMsg,
) {
    use prfh::net::server::send_msg;
    let mut dead = Vec::new();
    for (cid, s) in streams.iter_mut() {
        if Some(*cid) == exclude {
            continue;
        }
        if send_msg(s, msg).is_err() {
            dead.push(*cid);
        }
    }
    for cid in dead {
        streams.remove(&cid);
    }
}
