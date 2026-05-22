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

fn match_trigger(word: &str) -> Option<Trigger> {
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

        if !self.paused {
            let (dx, dy) = self.direction.delta();
            self.cursor = (self.cursor.0 + dx, self.cursor.1 + dy);
        } else if !stopped {
            self.paused = false;
        }

        if let Some(d) = turned_to {
            StepResult::WroteAndTurned(tile, d)
        } else if stopped {
            StepResult::WroteAndStopped(tile)
        } else {
            StepResult::Wrote(tile)
        }
    }

    pub fn on_backspace(&mut self) -> StepResult {
        if let Some(last) = self.trail.pop() {
            self.cursor = last.pos;
            if !self.current_word.is_empty() {
                self.current_word.pop();
            }
            self.doubt += 1;
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
    fn punctuation_acts_as_boundary() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "up.".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Up);
    }
}
