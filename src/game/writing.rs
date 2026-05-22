#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        }
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Direction(Direction),
    Back,
    Stop,
}

pub fn match_trigger(word: &str) -> Option<Trigger> {
    match word.to_ascii_lowercase().as_str() {
        "up" => Some(Trigger::Direction(Direction::Up)),
        "down" => Some(Trigger::Direction(Direction::Down)),
        "left" => Some(Trigger::Direction(Direction::Left)),
        "right" => Some(Trigger::Direction(Direction::Right)),
        "back" => Some(Trigger::Back),
        "stop" => Some(Trigger::Stop),
        _ => None,
    }
}

pub fn is_trigger_word(word: &str) -> bool {
    match_trigger(word).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub pos: (i32, i32),
    pub ch: char,
}

#[derive(Debug, Clone)]
pub struct WritingEngine {
    pub cursor: (i32, i32),
    pub direction: Direction,
    pub trail: Vec<Tile>,
    pub current_word: String,
    pub combo: u32,
    pub doubt: u32,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepResult {
    Wrote(Tile),
    WroteAndTurned(Tile, Direction),
    WroteAndStopped(Tile),
    Erased,
}

impl WritingEngine {
    pub fn new(start: (i32, i32)) -> Self {
        Self {
            cursor: start,
            direction: Direction::Right,
            trail: Vec::new(),
            current_word: String::new(),
            combo: 0,
            doubt: 0,
            paused: false,
        }
    }

    pub fn on_char(&mut self, ch: char) -> StepResult {
        let is_boundary = ch.is_whitespace() || matches!(ch, '.' | ',' | '!' | '?' | ';' | ':');

        let tile = Tile {
            pos: self.cursor,
            ch,
        };
        self.trail.push(tile.clone());

        let mut turned_to: Option<Direction> = None;
        let mut stopped = false;

        if is_boundary {
            if let Some(trigger) = match_trigger(&self.current_word) {
                match trigger {
                    Trigger::Direction(d) => {
                        self.direction = d;
                        turned_to = Some(d);
                    }
                    Trigger::Back => {
                        self.direction = self.direction.opposite();
                        turned_to = Some(self.direction);
                    }
                    Trigger::Stop => {
                        self.paused = true;
                        stopped = true;
                    }
                }
            }
            self.current_word.clear();
        } else {
            self.current_word.push(ch);
        }

        if self.paused && !stopped {
            // Stop-trigger just fired: this char overwrites in place, then unpause.
            self.paused = false;
        } else if !self.paused {
            let (dx, dy) = self.direction.delta();
            self.cursor = (self.cursor.0 + dx, self.cursor.1 + dy);
        }

        // Combo: increment on successful write (non-erasure)
        self.combo = self.combo.saturating_add(1);

        if let Some(d) = turned_to {
            StepResult::WroteAndTurned(tile, d)
        } else if stopped {
            StepResult::WroteAndStopped(tile)
        } else {
            StepResult::Wrote(tile)
        }
    }

    /// Newline: jump to next line, reset direction to Right.
    pub fn on_newline(&mut self) {
        self.cursor = (0, self.cursor.1 + 1);
        self.direction = Direction::Right;
        self.current_word.clear();
    }

    /// Flush the current word as if a boundary was hit, without writing a char or stepping.
    /// Returns Some(Direction) if the flush turned us, None otherwise.
    pub fn flush_word(&mut self) -> Option<Direction> {
        let trigger = match_trigger(&self.current_word);
        self.current_word.clear();
        match trigger {
            Some(Trigger::Direction(d)) => {
                self.direction = d;
                Some(d)
            }
            Some(Trigger::Back) => {
                self.direction = self.direction.opposite();
                Some(self.direction)
            }
            Some(Trigger::Stop) => {
                self.paused = true;
                None
            }
            None => None,
        }
    }

    pub fn on_backspace(&mut self) -> StepResult {
        if let Some(last) = self.trail.pop() {
            self.cursor = last.pos;
            if !self.current_word.is_empty() {
                self.current_word.pop();
            }
            self.doubt = self.doubt.saturating_add(1);
            self.combo = 0;
        }
        StepResult::Erased
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_direction_is_right() {
        let e = WritingEngine::new((0, 0));
        assert_eq!(e.direction, Direction::Right);
    }

    #[test]
    fn writes_advance_right_by_default() {
        let mut e = WritingEngine::new((0, 0));
        e.on_char('h');
        e.on_char('i');
        assert_eq!(e.cursor, (2, 0));
        assert_eq!(e.trail.len(), 2);
    }

    #[test]
    fn up_trigger_changes_direction_after_boundary() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "go up ".chars() {
            e.on_char(ch);
        }
        // 'g','o',' ','u','p',' ' — 6 steps right (1 each), then direction up
        assert_eq!(e.direction, Direction::Up);
    }

    #[test]
    fn upgrade_does_not_trigger_up() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "upgrade ".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Right);
    }

    #[test]
    fn down_trigger_changes_to_down() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "go down ".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Down);
    }

    #[test]
    fn back_reverses_direction() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "go up ".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Up);
        for ch in "back ".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Down);
    }

    #[test]
    fn backspace_walks_back_and_increases_doubt() {
        let mut e = WritingEngine::new((0, 0));
        e.on_char('h');
        e.on_char('i');
        assert_eq!(e.cursor, (2, 0));
        e.on_backspace();
        assert_eq!(e.cursor, (1, 0));
        assert_eq!(e.doubt, 1);
        e.on_backspace();
        assert_eq!(e.cursor, (0, 0));
        assert_eq!(e.doubt, 2);
    }

    #[test]
    fn flush_word_triggers_without_stepping() {
        let mut e = WritingEngine::new((0, 0));
        e.on_char('u');
        e.on_char('p');
        let start = e.cursor;
        let turned = e.flush_word();
        assert_eq!(turned, Some(Direction::Up));
        assert_eq!(e.direction, Direction::Up);
        assert_eq!(e.cursor, start, "flush_word should not move cursor");
        assert!(e.current_word.is_empty());
    }

    #[test]
    fn flush_word_with_no_match_returns_none() {
        let mut e = WritingEngine::new((0, 0));
        e.on_char('h');
        e.on_char('i');
        assert_eq!(e.flush_word(), None);
        assert_eq!(e.direction, Direction::Right);
    }

    #[test]
    fn punctuation_acts_as_boundary() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "up.".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Up);
    }

    #[test]
    fn left_trigger_moves_cursor_left_after_space() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "left ".chars() {
            e.on_char(ch);
        }
        // 'l','e','f','t' = 4 right steps, then space: trigger Left + step left.
        assert_eq!(e.direction, Direction::Left);
        assert_eq!(e.cursor, (3, 0));
        // Next char goes further left.
        e.on_char('x');
        assert_eq!(e.cursor, (2, 0));
    }

    #[test]
    fn right_trigger_keeps_default_direction() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "right ".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Right);
        assert_eq!(e.cursor, (6, 0)); // 5 chars + 1 step right after trigger
    }

    #[test]
    fn stop_pauses_then_overwrites_then_resumes() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "stop ".chars() {
            e.on_char(ch);
        }
        // 's','t','o','p' = 4 right steps, then space: stop trigger, paused=true, no step.
        assert_eq!(e.cursor, (4, 0));
        assert!(e.paused);
        // Next char overwrites in place, unpauses, but doesn't step yet.
        e.on_char('x');
        assert_eq!(e.cursor, (4, 0));
        assert!(!e.paused);
        // Subsequent chars step normally.
        e.on_char('y');
        assert_eq!(e.cursor, (5, 0));
    }

    #[test]
    fn combo_increments_per_write_and_resets_on_backspace() {
        let mut e = WritingEngine::new((0, 0));
        e.on_char('h');
        e.on_char('i');
        assert_eq!(e.combo, 2);
        e.on_backspace();
        assert_eq!(e.combo, 0);
    }

    #[test]
    fn newline_resets_to_left_edge_and_right_direction() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "up ".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Up);
        let prev_y = e.cursor.1;
        e.on_newline();
        assert_eq!(e.cursor, (0, prev_y + 1));
        assert_eq!(e.direction, Direction::Right);
    }

    #[test]
    fn is_trigger_word_works() {
        assert!(is_trigger_word("up"));
        assert!(is_trigger_word("Down"));
        assert!(is_trigger_word("STOP"));
        assert!(!is_trigger_word("upgrade"));
        assert!(!is_trigger_word(""));
    }
}
