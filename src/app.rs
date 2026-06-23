use crate::game::world::{PlayerId, PlayerView, WorldView};
use crate::game::writing::{StepResult, WritingEngine};
use crate::net::server::HostState;

impl Default for App {
    fn default() -> Self {
        Self::new_single()
    }
}

pub enum Mode {
    Single(WritingEngine),
    Host(HostState),
    Client(WorldView),
}

pub struct App {
    pub should_quit: bool,
    pub mode: Mode,
    pub last_event: String,
    pub trigger_banner: Option<String>,
    pub trigger_banner_ticks: u32,
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
            mode: Mode::Single(WritingEngine::new((0, 0))),
            last_event: String::from("type to write yourself a path"),
            trigger_banner: None,
            trigger_banner_ticks: 0,
            debug: false,
            debug_lines: Vec::new(),
        }
    }

    pub fn self_id(&self) -> PlayerId {
        match &self.mode {
            Mode::Single(_) => 0,
            Mode::Host(h) => h.self_id(),
            Mode::Client(w) => w.self_id,
        }
    }

    pub fn local_engine(&self) -> Option<&WritingEngine> {
        match &self.mode {
            Mode::Single(e) => Some(e),
            Mode::Host(h) => Some(h.local_engine()),
            Mode::Client(_) => None,
        }
    }

    pub fn world_view(&self) -> WorldView {
        match &self.mode {
            Mode::Single(e) => WorldView {
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
                }],
            },
            Mode::Host(h) => h.world_view(),
            Mode::Client(w) => w.clone(),
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
        if self.trigger_banner_ticks > 0 {
            self.trigger_banner_ticks -= 1;
            if self.trigger_banner_ticks == 0 {
                self.trigger_banner = None;
            }
        }
        match &mut self.mode {
            Mode::Single(e) => {
                let _ = e.tick_visuals();
            }
            // Host tick_visuals is driven by run_host (which also broadcasts
            // the returned messages), so we skip it here.
            Mode::Host(_) => {}
            Mode::Client(w) => w.tick_visuals(),
        }
    }

    /// Single-player local input. (Host/Client routing added in Task 9.)
    pub fn on_char(&mut self, c: char) {
        if c == ' ' {
            return;
        }
        if let Mode::Single(e) = &mut self.mode {
            let result = e.on_char(c);
            self.last_event = match &result {
                StepResult::Wrote(_) => format!("wrote '{}'", c),
                StepResult::WroteAndTurned(_, d) => format!("turned: {:?}", d),
                StepResult::WroteAndStopped(_) => "paused".into(),
                StepResult::Erased => "erased".into(),
            };
            if let StepResult::WroteAndTurned(_, d) = result {
                self.set_banner(format!("⟹ TURNED: {:?}", d));
            }
            if matches!(result, StepResult::WroteAndStopped(_)) {
                self.set_banner("⟹ STOP — next char overwrites".into());
            }
        }
    }

    pub fn on_backspace(&mut self) {
        if let Mode::Single(e) = &mut self.mode {
            e.on_backspace();
            self.last_event = format!("walked back. doubt: {}", e.doubt);
        }
    }

    pub fn on_enter(&mut self) {}

    fn set_banner(&mut self, msg: String) {
        self.trigger_banner = Some(msg);
        self.trigger_banner_ticks = 90;
    }
}
