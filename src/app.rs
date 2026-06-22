use crate::game::writing::{StepResult, WritingEngine};

pub struct App {
    pub should_quit: bool,
    pub writing: WritingEngine,
    pub last_event: String,
    /// Sticky trigger banner — set when a trigger fires, decremented per tick.
    pub trigger_banner: Option<String>,
    pub trigger_banner_ticks: u32,
    pub debug: bool,
    pub debug_lines: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            writing: WritingEngine::new((0, 0)),
            last_event: String::from("type to write yourself a path"),
            trigger_banner: None,
            trigger_banner_ticks: 0,
            debug: false,
            debug_lines: Vec::new(),
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
        self.writing.tick_visuals();
    }

    pub fn on_char(&mut self, c: char) {
        // Spacebar is disabled — it writes no tile and never moves the cursor.
        if c == ' ' {
            return;
        }
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

    pub fn on_backspace(&mut self) {
        self.writing.on_backspace();
        self.last_event = format!("walked back. doubt: {}", self.writing.doubt);
    }

    pub fn on_enter(&mut self) {
        // In immediate-mode, triggers fire as soon as the word is typed —
        // Enter has no role. Ignore.
    }
}
