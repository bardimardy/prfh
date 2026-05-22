use crate::game::shell::{parse, ShellCommand};

pub enum Scene {
    Boot,
    Menu,
    Day0,
    Shell,
}

pub struct App {
    pub should_quit: bool,
    pub scene: Scene,
    pub input_buffer: String,
    pub history: Vec<String>,
    pub day: i64,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            scene: Scene::Boot,
            input_buffer: String::new(),
            history: vec!["Loading career...".into()],
            day: 4380,
        }
    }

    pub fn tick(&mut self) {
        if matches!(self.scene, Scene::Boot) {
            self.scene = Scene::Menu;
        }
    }

    pub fn on_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    pub fn on_backspace(&mut self) {
        self.input_buffer.pop();
    }

    pub fn on_enter(&mut self) {
        let line = std::mem::take(&mut self.input_buffer);
        let cmd = parse(&line);
        let response = self.handle_command(cmd, &line);
        self.history.push(format!("$ {}", line));
        if !response.is_empty() {
            self.history.push(response);
        }
        if self.history.len() > 200 {
            let drain_n = self.history.len() - 200;
            self.history.drain(0..drain_n);
        }
    }

    fn handle_command(&mut self, cmd: ShellCommand, raw: &str) -> String {
        match cmd {
            ShellCommand::Help => "Available: cd, ls, cat, grep, rm, sudo, jetpack, git, exit".into(),
            ShellCommand::Ls => "  bug42  reviewer.md  TODO.txt  coffee/".into(),
            ShellCommand::Cd(target) => format!("(would cd to: {})", target),
            ShellCommand::Cat(path) => format!("(would cat: {})", path),
            ShellCommand::Grep(pattern) => format!("(would grep: {})", pattern),
            ShellCommand::Rm(target) => format!("rm: cannot remove '{}': In active code review", target),
            ShellCommand::Sudo(ability) => format!("[sudo] activating: {}", ability),
            ShellCommand::Jetpack => "☕ jetpack: caffeine reservoir empty.".into(),
            ShellCommand::GitStash => {
                self.should_quit = true;
                "Stashing run. You'll wake up tomorrow having lost progress.".into()
            }
            ShellCommand::Exit => {
                self.should_quit = true;
                "There is no escape. (exiting anyway)".into()
            }
            ShellCommand::Invalid(_) if raw.trim().is_empty() => String::new(),
            ShellCommand::Invalid(s) => format!("zsh: command not found: {}", s),
        }
    }
}
