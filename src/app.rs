use crate::game::arena::Arena;
use crate::game::world::{PlayerId, PlayerView, WorldView};
use crate::game::writing::{StepResult, WritingEngine};
use crate::hud::notify::{NotificationStack, NotifyKind};
use crate::net::server::HostState;

impl Default for App {
    fn default() -> Self {
        Self::new_single()
    }
}

pub enum Mode {
    Single(WritingEngine, Arena),
    Host(HostState),
    Client(WorldView, Arena),
}

pub struct App {
    pub should_quit: bool,
    pub mode: Mode,
    pub last_event: String,
    /// Dynamische Quick-Notifications (oben-mitte, schweben über der Welt).
    /// Ersetzt das frühere statische `trigger_banner`.
    pub notifications: NotificationStack,
    pub debug: bool,
    pub debug_lines: Vec<String>,
}

impl App {
    /// Alias for `new_single` — used by render tests imported from main.
    pub fn new() -> Self {
        Self::new_single()
    }

    pub fn new_with_mode(mode: Mode) -> Self {
        let mut a = App::new_single();
        a.mode = mode;
        a
    }

    pub fn new_single() -> Self {
        Self {
            should_quit: false,
            mode: Mode::Single(WritingEngine::new((0, 0)), Arena::new()),
            last_event: String::from("type to write yourself a path"),
            notifications: NotificationStack::new(),
            debug: false,
            debug_lines: Vec::new(),
        }
    }

    pub fn self_id(&self) -> PlayerId {
        match &self.mode {
            Mode::Single(..) => 0,
            Mode::Host(h) => h.self_id(),
            Mode::Client(w, _) => w.self_id,
        }
    }

    pub fn local_engine(&self) -> Option<&WritingEngine> {
        match &self.mode {
            Mode::Single(e, _) => Some(e),
            Mode::Host(h) => Some(h.local_engine()),
            Mode::Client(..) => None,
        }
    }

    pub fn world_view(&self) -> WorldView {
        match &self.mode {
            Mode::Single(e, _) => WorldView {
                self_id: 0,
                players: vec![PlayerView {
                    id: 0,
                    color: crate::game::world::PALETTE[0],
                    name: "you".into(),
                    trail: e.trail.clone(),
                    cursor: e.cursor,
                    direction: e.direction,
                    is_self: true,
                    is_dead: false,
                    pace: e.pace,
                }],
            },
            Mode::Host(h) => h.world_view(),
            Mode::Client(w, _) => w.clone(),
        }
    }

    /// Aktuelle Sim-Arena fürs Rendering (analog zu `world_view`).
    pub fn arena(&self) -> &Arena {
        match &self.mode {
            Mode::Single(_, a) => a,
            Mode::Host(h) => h.arena(),
            Mode::Client(_, a) => a,
        }
    }

    /// Mutabler Zugriff auf die lokal gehaltene Arena (Single/Client). Host
    /// mutiert seine Arena über `HostState`. Skeleton-Hook: W2 befüllt die
    /// Single-Arena, W3 verdrahtet Pickup/Despawn.
    pub fn arena_mut(&mut self) -> Option<&mut Arena> {
        match &mut self.mode {
            Mode::Single(_, a) | Mode::Client(_, a) => Some(a),
            Mode::Host(_) => None,
        }
    }

    pub fn debug_log<S: Into<String>>(&mut self, line: S) {
        self.debug_lines.push(line.into());
        let max = 12;
        if self.debug_lines.len() > max {
            let drop = self.debug_lines.len() - max;
            self.debug_lines.drain(0..drop);
        }
    }

    pub fn tick(&mut self) {
        // Notifications werden zeitbasiert im Render (mit Frame-`elapsed`)
        // getrieben, nicht hier — `tick` ist frame-/visual-State.
        match &mut self.mode {
            Mode::Single(e, _) => e.tick_visuals(),
            // Host tick_visuals is driven by run_host (which also broadcasts
            // the returned Respawned messages), so we skip it here.
            Mode::Host(_) => {}
            Mode::Client(w, _) => w.tick_visuals(),
        }
    }

    /// Single-player local input. (Host/Client routing added in Task 9.)
    pub fn on_char(&mut self, c: char) {
        if c == ' ' {
            return;
        }
        if let Mode::Single(e, _) = &mut self.mode {
            let result = e.on_char(c);
            self.last_event = match &result {
                StepResult::Wrote(_) => format!("wrote '{}'", c),
                StepResult::WroteAndTurned(_, d) => format!("turned: {:?}", d),
                StepResult::WroteAndStopped(_) => "paused".into(),
                StepResult::Erased => "erased".into(),
            };
            match result {
                StepResult::WroteAndTurned(_, d) => {
                    self.notifications
                        .push(NotifyKind::Info, "⟹  TURNED", format!("{d:?}"));
                }
                StepResult::WroteAndStopped(_) => {
                    self.notifications
                        .push(NotifyKind::Info, "⟹  STOP", "next char overwrites");
                }
                _ => {}
            }
        }
    }

    pub fn on_backspace(&mut self) {
        if let Mode::Single(e, _) = &mut self.mode {
            e.on_backspace();
            self.last_event = format!("walked back. doubt: {}", e.doubt);
        }
    }

    pub fn on_enter(&mut self) {}
}
