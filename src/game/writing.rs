use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

const TRIGGER_WORDS: &[&str] = &["right", "down", "left", "back", "stop", "up"];

/// Check whether the suffix of the buffer ends with any trigger word.
/// Returns the matched trigger. Longest match wins (so "right" is checked
/// before "up" even though "up" is shorter).
pub fn match_trigger_suffix(word: &str) -> Option<Trigger> {
    find_trigger_suffix(word).map(|(t, _)| t)
}

fn find_trigger_suffix(word: &str) -> Option<(Trigger, usize)> {
    let lower = word.to_ascii_lowercase();
    for tw in TRIGGER_WORDS {
        if lower.ends_with(tw) {
            return match_trigger(tw).map(|t| (t, tw.len()));
        }
    }
    None
}

pub fn is_trigger_word(word: &str) -> bool {
    match_trigger(word).is_some()
}

/// Returns true if the buffer's lowercase suffix matches any trigger.
/// Used by the HUD to highlight when the user is about to fire a trigger.
pub fn buffer_ends_with_trigger(word: &str) -> bool {
    match_trigger_suffix(word).is_some()
}

/// Initial brightness of a freshly written tile.
pub const TILE_MAX_BRIGHTNESS: u8 = 200;

/// Number of most-recent tiles that are always kept at full brightness.
pub const TRAIL_SAFE_ZONE: usize = 75;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    pub pos: (i32, i32),
    pub ch: char,
    /// Engine tick when this tile was written.
    pub tick: u64,
    /// Remaining glow ticks. Non-zero = highlight as part of a recently-fired trigger.
    pub glow: u32,
    /// Current brightness (0 = invisible and will be removed, TILE_MAX_BRIGHTNESS = full).
    pub brightness: u8,
}

/// How many ticks a trigger-tile glows after firing.
pub const GLOW_TICKS: u32 = 30;

/// Derive a per-frame brightness decrease based on how long the player has
/// been idle (in render frames, ~60 fps).
///   < 6 frames  (~100 ms): 0   — just typed, no fade
///   6–59 frames (up to 1s): 1–3 — gentle fade
///   ≥ 60 frames (> 1s):     5   — fast fade
fn fade_rate(idle_frames: u32) -> u8 {
    match idle_frames {
        0..=5 => 0,
        6..=59 => 1 + ((idle_frames - 6) * 4 / 54).min(4) as u8,
        _ => 50,
    }
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
    /// Monotonically increasing tick counter, advanced on every char write.
    pub tick: u64,
    /// Render frames elapsed since the last on_char call. Drives fade speed.
    pub idle_frames: u32,
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
            tick: 0,
            idle_frames: 0,
        }
    }

    /// Render-time tick: decrement glow, fade old tiles, remove invisible ones.
    /// Called once per frame from the app loop, regardless of input.
    pub fn tick_visuals(&mut self) {
        self.idle_frames = self.idle_frames.saturating_add(1);
        let rate = fade_rate(self.idle_frames);

        let safe = TRAIL_SAFE_ZONE;
        let len = self.trail.len();

        for (i, t) in self.trail.iter_mut().enumerate() {
            if t.glow > 0 {
                t.glow -= 1;
            }
            // Tiles inside the safe zone keep full brightness.
            if i + safe >= len {
                t.brightness = TILE_MAX_BRIGHTNESS;
            } else {
                t.brightness = t.brightness.saturating_sub(rate);
            }
        }

        // Remove tiles that have fully faded (brightness == 0).
        self.trail.retain(|t| t.brightness > 0);
    }

    pub fn on_char(&mut self, ch: char) -> StepResult {
        let is_boundary = ch.is_whitespace() || matches!(ch, '.' | ',' | '!' | '?' | ';' | ':');

        self.idle_frames = 0;
        let tile = Tile {
            pos: self.cursor,
            ch,
            tick: self.tick,
            glow: 0,
            brightness: TILE_MAX_BRIGHTNESS,
        };
        self.trail.push(tile.clone());

        // Step in the CURRENT direction first — the char that completes a trigger
        // word still writes in the old direction. The NEW direction takes effect
        // on the next char.
        if self.paused {
            // Stop-trigger fired previously: overwrite this char in place, then unpause.
            self.paused = false;
        } else {
            let (dx, dy) = self.direction.delta();
            self.cursor = (self.cursor.0 + dx, self.cursor.1 + dy);
        }
        self.combo = self.combo.saturating_add(1);

        let mut turned_to: Option<Direction> = None;
        let mut stopped = false;

        if is_boundary {
            // Boundary char just resets the word buffer (immediate-mode model
            // means triggers have already fired the moment the word was complete).
            self.current_word.clear();
        } else {
            self.current_word.push(ch);
            // Immediate trigger: fire as soon as the buffer's suffix matches
            // a trigger word. This means "helloup" also fires Up.
            if let Some((trigger, tw_len)) = find_trigger_suffix(&self.current_word) {
                let n = self.trail.len();
                let start = n.saturating_sub(tw_len);
                for t in &mut self.trail[start..n] {
                    t.glow = GLOW_TICKS;
                }
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
                self.current_word.clear();
            }
        }

        self.tick = self.tick.saturating_add(1);

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
    fn up_triggers_immediately_no_space_needed() {
        let mut e = WritingEngine::new((0, 0));
        e.on_char('u');
        e.on_char('p');
        // Trigger fired the moment "up" was complete.
        assert_eq!(e.direction, Direction::Up);
        // The 'p' itself still wrote in the OLD direction (Right).
        assert_eq!(e.cursor, (2, 0));
        // The next char goes up.
        e.on_char('x');
        assert_eq!(e.cursor, (2, -1));
    }

    #[test]
    fn upgrade_also_triggers_up_immediate_mode() {
        // In immediate-mode, "upgrade" fires up after 'p', then "grade" goes up.
        let mut e = WritingEngine::new((0, 0));
        for ch in "upgrade".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Up);
    }

    #[test]
    fn down_triggers_immediately() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "down".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Down);
    }

    #[test]
    fn back_reverses_immediately() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "up".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Up);
        for ch in "back".chars() {
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
    fn punctuation_clears_buffer_without_trigger() {
        // Punctuation no longer triggers (immediate-mode already triggered on 'p').
        let mut e = WritingEngine::new((0, 0));
        for ch in "up.".chars() {
            e.on_char(ch);
        }
        // Trigger fired on 'p'. Punct resets buffer. Direction is still Up.
        assert_eq!(e.direction, Direction::Up);
        assert!(e.current_word.is_empty());
    }

    #[test]
    fn left_triggers_immediately() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "left".chars() {
            e.on_char(ch);
        }
        // 'l','e','f','t' wrote going right (4 steps), trigger fires on 't'.
        assert_eq!(e.direction, Direction::Left);
        assert_eq!(e.cursor, (4, 0));
        // Next char goes left.
        e.on_char('x');
        assert_eq!(e.cursor, (3, 0));
    }

    #[test]
    fn right_triggers_immediately() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "right".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Right);
        assert_eq!(e.cursor, (5, 0));
    }

    #[test]
    fn stop_triggers_immediately_and_pauses_next_char() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "stop".chars() {
            e.on_char(ch);
        }
        // 's','t','o','p' wrote going right (4 steps), trigger fired on 'p'.
        assert_eq!(e.cursor, (4, 0));
        assert!(e.paused);
        // Next char overwrites in place, unpauses, but doesn't step.
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
    fn is_trigger_word_works() {
        assert!(is_trigger_word("up"));
        assert!(is_trigger_word("Down"));
        assert!(is_trigger_word("STOP"));
        assert!(!is_trigger_word("upgrade"));
        assert!(!is_trigger_word(""));
    }

    #[test]
    fn suffix_trigger_fires_inside_unbroken_sentence() {
        // No spaces — user types one long string. Triggers must still fire.
        let mut e = WritingEngine::new((0, 0));
        for ch in "helloworldup".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Up);
        // After the trigger fires the buffer is cleared.
        assert!(e.current_word.is_empty());
        for ch in "down".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Down);
    }

    #[test]
    fn suffix_trigger_left_after_arbitrary_text() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "iwillturnleft".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Left);
    }

    #[test]
    fn tick_advances_per_char_write() {
        let mut e = WritingEngine::new((0, 0));
        assert_eq!(e.tick, 0);
        e.on_char('a');
        e.on_char('b');
        assert_eq!(e.tick, 2);
        assert_eq!(e.trail[0].tick, 0);
        assert_eq!(e.trail[1].tick, 1);
    }

    #[test]
    fn trigger_marks_last_n_tiles_as_glowing() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "hellup".chars() {
            e.on_char(ch);
        }
        // 'u' and 'p' (last two tiles) should glow; the rest must not.
        let n = e.trail.len();
        assert_eq!(e.trail[n - 1].glow, GLOW_TICKS);
        assert_eq!(e.trail[n - 2].glow, GLOW_TICKS);
        assert_eq!(e.trail[n - 3].glow, 0);
    }

    #[test]
    fn tick_visuals_decrements_glow() {
        let mut e = WritingEngine::new((0, 0));
        for ch in "up".chars() {
            e.on_char(ch);
        }
        let g0 = e.trail.last().unwrap().glow;
        e.tick_visuals();
        assert_eq!(e.trail.last().unwrap().glow, g0 - 1);
        // Saturates at 0.
        for _ in 0..(GLOW_TICKS + 5) {
            e.tick_visuals();
        }
        assert_eq!(e.trail.last().unwrap().glow, 0);
    }

    #[test]
    fn longest_suffix_wins_right_not_t() {
        // "right" must be matched before any shorter trigger that could
        // end the same string. (None do, but defensive test.)
        let mut e = WritingEngine::new((0, 0));
        for ch in "turnright".chars() {
            e.on_char(ch);
        }
        assert_eq!(e.direction, Direction::Right);
    }

    #[test]
    fn tile_and_direction_ron_roundtrip() {
        let t = Tile {
            pos: (3, -2),
            ch: 'x',
            tick: 7,
            glow: GLOW_TICKS,
            brightness: TILE_MAX_BRIGHTNESS,
        };
        let s = ron::to_string(&t).unwrap();
        let back: Tile = ron::from_str(&s).unwrap();
        assert_eq!(t, back);

        let d = Direction::Left;
        let s = ron::to_string(&d).unwrap();
        let back: Direction = ron::from_str(&s).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn safe_zone_tiles_never_fade() {
        let mut e = WritingEngine::new((0, 0));
        // Write exactly TRAIL_SAFE_ZONE tiles.
        for ch in ('a'..='z').cycle().take(TRAIL_SAFE_ZONE) {
            e.on_char(ch);
        }
        // Simulate many idle frames (fast fade rate).
        for _ in 0..200 {
            e.tick_visuals();
        }
        // All tiles are in the safe zone — none should have been removed.
        assert_eq!(e.trail.len(), TRAIL_SAFE_ZONE);
        // And all should be at max brightness.
        assert!(e.trail.iter().all(|t| t.brightness == TILE_MAX_BRIGHTNESS));
    }

    #[test]
    fn tiles_outside_safe_zone_fade_and_are_removed() {
        let mut e = WritingEngine::new((0, 0));
        // Write enough tiles so the first ones are outside the safe zone.
        for ch in ('a'..='z').cycle().take(TRAIL_SAFE_ZONE + 10) {
            e.on_char(ch);
        }
        let initial_len = e.trail.len();
        // Trigger fast fade: many idle frames.
        for _ in 0..300 {
            e.tick_visuals();
        }
        // Some tiles outside the safe zone should have been removed.
        assert!(
            e.trail.len() < initial_len,
            "old tiles should have been removed"
        );
        // The safe zone is still intact.
        assert!(e.trail.len() >= TRAIL_SAFE_ZONE);
    }
}
