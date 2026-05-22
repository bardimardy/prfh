use crate::game::shell::{parse, ShellCommand};
use crate::game::writing::{StepResult, WritingEngine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    World,
    Shell,
}

pub struct App {
    pub should_quit: bool,
    pub mode: Mode,
    pub writing: WritingEngine,
    pub shell_buffer: String,
    pub shell_history: Vec<String>,
    pub day: i64,
    pub last_event: String,
    /// Sticky trigger banner — set when a trigger fires, decremented per tick.
    pub trigger_banner: Option<String>,
    pub trigger_banner_ticks: u32,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            mode: Mode::World,
            writing: WritingEngine::new((0, 0)),
            shell_buffer: String::new(),
            shell_history: vec!["Loading career...".into()],
            day: 4380,
            last_event: String::from("type to write yourself a path"),
            trigger_banner: None,
            trigger_banner_ticks: 0,
        }
    }

    pub fn tick(&mut self) {
        if self.trigger_banner_ticks > 0 {
            self.trigger_banner_ticks -= 1;
            if self.trigger_banner_ticks == 0 {
                self.trigger_banner = None;
            }
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            Mode::World => Mode::Shell,
            Mode::Shell => Mode::World,
        };
    }

    pub fn on_char(&mut self, c: char) {
        match self.mode {
            Mode::World => {
                let result = self.writing.on_char(c);
                self.last_event = match &result {
                    StepResult::Wrote(_) => format!("wrote '{}'", c),
                    StepResult::WroteAndTurned(_, d) => format!("turned: {:?}", d),
                    StepResult::WroteAndStopped(_) => "paused".into(),
                    StepResult::Erased => "erased".into(),
                };
                if let StepResult::WroteAndTurned(_, d) = result {
                    self.trigger_banner = Some(format!("⟹ TURNED: {:?}", d));
                    self.trigger_banner_ticks = 90; // ~1.5s at 60 FPS
                }
                if matches!(result, StepResult::WroteAndStopped(_)) {
                    self.trigger_banner = Some("⟹ STOP — next char overwrites".into());
                    self.trigger_banner_ticks = 90;
                }
            }
            Mode::Shell => {
                self.shell_buffer.push(c);
            }
        }
    }

    pub fn on_backspace(&mut self) {
        match self.mode {
            Mode::World => {
                self.writing.on_backspace();
                self.last_event = format!("walked back. doubt: {}", self.writing.doubt);
            }
            Mode::Shell => {
                self.shell_buffer.pop();
            }
        }
    }

    pub fn on_enter(&mut self) {
        if matches!(self.mode, Mode::World) {
            // In immediate-mode, triggers fire as soon as the word is typed —
            // Enter has no role in World mode. Ignore.
            return;
        }
        if matches!(self.mode, Mode::Shell) {
            let line = std::mem::take(&mut self.shell_buffer);
            let cmd = parse(&line);
            let response = self.handle_shell(cmd, &line);
            self.shell_history.push(format!("$ {}", line));
            if !response.is_empty() {
                self.shell_history.push(response);
            }
            if self.shell_history.len() > 200 {
                let drain_n = self.shell_history.len() - 200;
                self.shell_history.drain(0..drain_n);
            }
        }
    }

    fn handle_shell(&mut self, cmd: ShellCommand, raw: &str) -> String {
        match cmd {
            ShellCommand::Help => {
                "available: cd, ls, cat, grep, rm, sudo, jetpack, git stash, exit".into()
            }
            ShellCommand::Ls => "  bug42  reviewer.md  TODO.txt  coffee/".into(),
            ShellCommand::Cd(t) => format!("(would cd to: {})", t),
            ShellCommand::Cat(p) => format!("(would cat: {})", p),
            ShellCommand::Grep(p) => format!("(would grep: {})", p),
            ShellCommand::Rm(t) => format!("rm: cannot remove '{}': in active code review", t),
            ShellCommand::Sudo(a) => format!("[sudo] activating: {}", a),
            ShellCommand::Jetpack => "☕ jetpack: caffeine reservoir empty.".into(),
            ShellCommand::GitStash => {
                self.should_quit = true;
                "stashing run. you'll wake up tomorrow having lost progress.".into()
            }
            ShellCommand::Exit => {
                self.should_quit = true;
                "there is no escape. (exiting anyway)".into()
            }
            ShellCommand::Invalid(_) if raw.trim().is_empty() => String::new(),
            ShellCommand::Invalid(s) => format!("zsh: command not found: {}", s),
        }
    }
}
