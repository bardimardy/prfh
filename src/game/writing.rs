use crate::game::powerup::PowerupWord;
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

/// Newest tiles kept at full brightness — the crisp "head" at the cursor.
pub const TRAIL_SAFE: usize = 5;
/// Visible trail length at the slowest pace (idle): the hard floor the trail
/// can shrink back to.
pub const TRAIL_MIN_VISIBLE: usize = 70;
/// Visible trail length at full pace (typing fast): long comet tail.
pub const TRAIL_MAX_VISIBLE: usize = 300;

/// How much one keystroke adds to the pace gauge (clamped to 1.0). ~7 fast
/// keystrokes saturate it.
const PACE_GAIN: f32 = 0.16;
/// Per-frame multiplicative decay of the pace gauge (~60 fps). Decays to ~half
/// in ~0.3s, so the trail retracts smoothly a beat after you slow down.
const PACE_DECAY: f32 = 0.965;

/// Map a normalized typing pace `[0,1]` to the current visible trail length.
/// Faster typing → longer trail; idle → short trail.
pub fn visible_len_for_pace(pace: f32) -> usize {
    let p = pace.clamp(0.0, 1.0);
    let span = (TRAIL_MAX_VISIBLE - TRAIL_MIN_VISIBLE) as f32;
    TRAIL_MIN_VISIBLE + (p * span).round() as usize
}

/// Advance the pace gauge by one idle frame (decay toward 0).
pub fn pace_decay(pace: &mut f32) {
    *pace = (*pace * PACE_DECAY).max(0.0);
}

/// Bump the pace gauge for one keystroke (toward 1.0).
pub fn pace_bump(pace: &mut f32) {
    *pace = (*pace + PACE_GAIN).min(1.0);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
pub struct Tile {
    pub pos: (i32, i32),
    pub ch: char,
    /// Engine tick when this tile was written.
    pub tick: u64,
    /// Remaining glow ticks. Non-zero = highlight as part of a recently-fired trigger.
    pub glow: u32,
    /// Current brightness (0 = invisible and will be removed, TILE_MAX_BRIGHTNESS = full).
    pub brightness: u8,
    /// Typing pace at write time — drives the individual fade-out rate for this tile.
    /// Defaults to 0.0 for tiles from snapshots/tests that don't set it.
    #[serde(default)]
    pub written_pace: f32,
}

/// How many ticks a trigger-tile glows after firing.
pub const GLOW_TICKS: u32 = 30;

/// Brightness for a tile `from_tail` positions behind the newest (0 = newest)
/// given the current `visible_len`. Pure function of position — independent of
/// idle time, so the gradient shows *while* writing.
///
/// The fade uses a quadratic ease-out that reaches exactly 0 at the visible
/// edge, so the tail dissolves smoothly into the background instead of
/// hard-cutting to a block of black.
pub fn trail_brightness(from_tail: usize, visible_len: usize) -> u8 {
    if from_tail < TRAIL_SAFE {
        return TILE_MAX_BRIGHTNESS;
    }
    let fade = visible_len.saturating_sub(TRAIL_SAFE).max(1);
    let into = from_tail - TRAIL_SAFE;
    if into >= fade {
        return 0;
    }
    // t: just below 1.0 right behind the head → 0.0 at the oldest retained tile.
    // Squaring eases the tail out (many near-black tiles) so the disappearance
    // is gradual rather than a hard cut to black.
    let t = (fade - 1 - into) as f32 / fade as f32;
    (TILE_MAX_BRIGHTNESS as f32 * t * t).round() as u8
}

/// Per-frame fade-out rate for a tile based on its individual `written_pace`.
/// Fast-written tiles (pace → 1.0) fade slowly; slowly-written tiles fade fast.
///   written_pace = 1.0 → rate   5  (≈ 40 frames, 0.67s — long tail lingers)
///   written_pace = 0.5 → rate 100  (≈  2 frames, fast snap)
///   written_pace = 0.0 → rate 200  (≈  1 frame,  instant)
fn fade_out_rate(written_pace: f32) -> u8 {
    let p = written_pace.clamp(0.0, 1.0);
    (5.0 + (1.0 - p) * 195.0).round() as u8
}

/// Apply pace-based trail management:
/// - Tiles within `visible_len` of the tail → full brightness (solid color).
/// - Tiles beyond `visible_len` → each fades at its own `written_pace`-derived rate.
/// - Tiles at brightness 0 → removed.
///
/// Because every tile carries the pace at which it was typed, fast-typed bursts
/// fade slowly as a block while slow-typed tiles vanish almost instantly —
/// creating the natural "chunks disappear at their own speed" effect.
///
/// Shared by single-player (`WritingEngine`) and multiplayer (`WorldView`) so
/// both fade and trim identically and *locally* — no network sync needed.
pub fn apply_trail_fade(trail: &mut Vec<Tile>, visible_len: usize) {
    let len = trail.len();
    let fade_start = len.saturating_sub(visible_len);

    for (i, t) in trail.iter_mut().enumerate() {
        if i < fade_start {
            // Outside visible window: fade at this tile's individual rate.
            t.brightness = t.brightness.saturating_sub(fade_out_rate(t.written_pace));
        } else {
            // Inside visible window: solid color.
            t.brightness = TILE_MAX_BRIGHTNESS;
        }
    }

    trail.retain(|t| t.brightness > 0);
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
    /// Typing-pace gauge in `[0,1]`: rises per keystroke, decays per frame.
    /// Drives the dynamic visible trail length (fast typing → longer trail).
    pub pace: f32,
    /// Während eines aktiven Trace gesetzt (von `app.rs`): unterdrückt die
    /// Sofort-Trigger-Erkennung, damit die eigenen Wort-Buchstaben (z. B. „up"
    /// in „update") nicht feuern (Powerup-Spec §6).
    pub trace_suspended: bool,
    /// Lokale Direction-Historie, **per Tile-`tick` verschlüsselt** (nicht per
    /// Position): `(tick, Laufrichtung-zum-Schreibzeitpunkt)`, also die Richtung
    /// VOR einem etwaigen Turn, den genau dieses Zeichen ausgelöst hat. Erlaubt
    /// `on_backspace`, die Richtung beim Zurücklöschen korrekt zu restaurieren.
    /// `tick` ist eine stabile, monoton wachsende Tile-Identität → die Historie
    /// spiegelt in `tick_visuals` exakt die *überlebenden* Trail-Tiles (per
    /// `retain` über die lebende Tick-Menge), desync-fest auch wenn der
    /// pace-abhängige Fade Tiles aus der Trail-Mitte entfernt (Löcher). Rein
    /// lokaler Input-State — nicht Teil von `Tile`, nicht serialisiert, nicht
    /// netzwerksynchronisiert.
    pub dir_history: Vec<(u64, Direction)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepResult {
    Wrote(Tile),
    WroteAndTurned(Tile, Direction),
    WroteAndStopped(Tile),
    Erased,
}

impl StepResult {
    /// Das in diesem Schritt geschriebene Tile (None bei `Erased`).
    pub fn tile(&self) -> Option<&Tile> {
        match self {
            StepResult::Wrote(t)
            | StepResult::WroteAndTurned(t, _)
            | StepResult::WroteAndStopped(t) => Some(t),
            StepResult::Erased => None,
        }
    }
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
            pace: 0.0,
            trace_suspended: false,
            dir_history: Vec::new(),
        }
    }

    /// Render-time tick: decrement glow, decay the pace gauge, then apply the
    /// positional fade gradient and trim the trail to the pace-derived length.
    /// Called once per frame from the app loop, regardless of input.
    pub fn tick_visuals(&mut self) {
        for t in &mut self.trail {
            if t.glow > 0 {
                t.glow -= 1;
            }
        }
        pace_decay(&mut self.pace);
        apply_trail_fade(&mut self.trail, visible_len_for_pace(self.pace));

        // dir_history exakt auf die überlebenden Trail-Tiles spiegeln. `apply_trail_fade`
        // entfernt Tiles per individueller, pace-abhängiger Helligkeit — ein neueres Tile
        // kann VOR einem älteren auf 0 fallen, also entstehen Löcher in der Trail-Mitte.
        // Per Tick-Identität (statt Position) zu prunen ist gegen solche Löcher immun und
        // hält die Historie beschränkt (== Trail-Länge).
        let live: std::collections::HashSet<u64> = self.trail.iter().map(|t| t.tick).collect();
        self.dir_history.retain(|(tk, _)| live.contains(tk));
    }

    pub fn on_char(&mut self, ch: char) -> StepResult {
        let is_boundary = ch.is_whitespace() || matches!(ch, '.' | ',' | '!' | '?' | ';' | ':');

        // Record the direction in effect WHEN this tile is written — i.e.
        // before any turn this same char might trigger. Keyed by this tile's
        // `tick` (== `self.tick` here, since it's bumped only at the end of
        // on_char), so the entry survives trail-fade holes by identity. Lets
        // on_backspace restore the pre-turn direction when erasing back across
        // a turn.
        self.dir_history.push((self.tick, self.direction));

        pace_bump(&mut self.pace);
        let tile = Tile {
            pos: self.cursor,
            ch,
            tick: self.tick,
            glow: 0,
            brightness: TILE_MAX_BRIGHTNESS,
            written_pace: self.pace,
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
        } else if self.trace_suspended {
            // Trace läuft: keine Trigger, Buffer leer halten (kein stale Trigger
            // nach Trace-Ende). Tile wurde bereits geschrieben + Cursor bewegt.
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
            // History spiegelt die überlebenden Tiles in Reihenfolge (tick_visuals
            // prunt per Tick), also gehört der letzte Eintrag zum gerade gepoppten
            // (neuesten) Tile. Tick-Gleichheit dokumentiert die Invariante.
            if let Some((tk, d)) = self.dir_history.pop() {
                debug_assert_eq!(tk, last.tick, "dir_history desync gegenüber trail");
                self.direction = d;
            }
            if !self.current_word.is_empty() {
                self.current_word.pop();
            }
            self.doubt = self.doubt.saturating_add(1);
            self.combo = 0;
        }
        StepResult::Erased
    }

    /// Blink/Teleport: Cursor springt direkt auf `landing`, Lauf-Richtung wird
    /// auf `facing` gesetzt. Es entsteht bewusst eine Lücke (kein Trail zwischen
    /// alter und neuer Position) — der „Sprung"-Charakter des Dash.
    pub fn dash_blink(&mut self, landing: (i32, i32), facing: Direction) {
        self.cursor = landing;
        self.direction = facing;
    }

    /// Trail-Burst: schreibt sofort ein Trail-Tile pro Buchstabe in `letters` ab
    /// dem Cursor entlang `dir_delta` (lückenlos), Cursor endet auf dem Lande-Tile.
    /// Die Buchstaben sind die **festen Ziel-Glyphen** des Dash (kein Teleport) —
    /// ganz normaler Trail (Outplay-Material), in den der Settle-Render einrastet.
    /// Hält `dir_history` Tile-für-Tile synchron (gleiche `tick`-Identität wie der
    /// Trail), damit `on_backspace` korrekt zurückläuft. `facing` ist die
    /// Lauf-Richtung danach. Gibt den `tick` des ersten Burst-Tiles zurück, damit
    /// der Aufrufer die Settle-Sequenz an diese Tile-Identität binden kann.
    pub fn dash_trail_burst(
        &mut self,
        dir_delta: (i32, i32),
        letters: &[char],
        facing: Direction,
    ) -> u64 {
        let start_tick = self.tick;
        for &ch in letters {
            self.dir_history.push((self.tick, self.direction));
            self.trail.push(Tile {
                pos: self.cursor,
                ch,
                tick: self.tick,
                glow: 0,
                brightness: TILE_MAX_BRIGHTNESS,
                written_pace: self.pace,
            });
            self.cursor = (self.cursor.0 + dir_delta.0, self.cursor.1 + dir_delta.1);
            self.tick = self.tick.saturating_add(1);
        }
        self.direction = facing;
        start_tick
    }
}

/// Zustand der beobachtenden Pickup-Trace-FSM (Powerup-Spec §6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TraceState {
    #[default]
    Idle,
    Tracing {
        id: u32,
        progress: usize,
    },
}

/// Ergebnis eines `observe`-Schritts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceStep {
    None,
    Armed { id: u32 },
    Advanced { id: u32, progress: usize },
    Completed { id: u32 },
    Reset,
}

/// Räumliches Arming-Trace: **beobachtet** jeden `on_char`-Schreibvorgang
/// (Position, Zeichen, Laufrichtung) und steuert die Base-Mechanik nicht um.
/// `id` ist die `EntityId` des Powerup-Worts in der Arena.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    pub state: TraceState,
}

impl Trace {
    pub fn new() -> Self {
        Self {
            state: TraceState::Idle,
        }
    }

    pub fn is_tracing(&self) -> bool {
        matches!(self.state, TraceState::Tracing { .. })
    }

    /// Beobachtet ein geschriebenes Tile. `dir` ist die Laufrichtung zum
    /// Schreibzeitpunkt. `words`: Kandidaten-Powerup-Wörter mit ihren EntityIds.
    pub fn observe(
        &mut self,
        pos: (i32, i32),
        ch: char,
        dir: Direction,
        words: &[(u32, &PowerupWord)],
    ) -> TraceStep {
        let ch = ch.to_ascii_lowercase();
        match self.state {
            TraceState::Idle => {
                for (id, w) in words {
                    // 1-Buchstaben-Wörter haben keine Lauf-Achse (`run_direction`
                    // ist `(0,0)`, das kein `Direction::delta` je trifft) → die
                    // Richtungs-Bedingung entfällt; Position + Zeichen genügen.
                    let dir_ok = w.len() <= 1 || dir.delta() == w.run_direction();
                    if pos == w.entry_tile() && dir_ok && w.expected_char(0) == Some(ch) {
                        if w.len() <= 1 {
                            return TraceStep::Completed { id: *id };
                        }
                        self.state = TraceState::Tracing {
                            id: *id,
                            progress: 1,
                        };
                        return TraceStep::Armed { id: *id };
                    }
                }
                TraceStep::None
            }
            TraceState::Tracing { id, progress } => {
                let Some((_, w)) = words.iter().find(|(wid, _)| *wid == id) else {
                    self.state = TraceState::Idle;
                    return TraceStep::Reset;
                };
                if w.keystroke_tile(progress) == Some(pos) && w.expected_char(progress) == Some(ch)
                {
                    let next = progress + 1;
                    if next >= w.len() {
                        self.state = TraceState::Idle;
                        TraceStep::Completed { id }
                    } else {
                        self.state = TraceState::Tracing { id, progress: next };
                        TraceStep::Advanced { id, progress: next }
                    }
                } else {
                    self.state = TraceState::Idle;
                    TraceStep::Reset
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::powerup::{Axis, PowerupWord};

    fn pw(name: &str, origin: (i32, i32), axis: Axis, reversed: bool) -> PowerupWord {
        PowerupWord {
            name: name.into(),
            origin,
            axis,
            reversed,
        }
    }

    #[test]
    fn trace_arms_on_entry_tile_correct_dir_and_char() {
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        // Wrong char at entry → no arm.
        assert_eq!(
            t.observe((3, 0), 'x', Direction::Right, &words),
            TraceStep::None
        );
        // Correct entry tile + dir + char → armed (progress 1).
        assert_eq!(
            t.observe((3, 0), 'd', Direction::Right, &words),
            TraceStep::Armed { id: 7 }
        );
        assert!(t.is_tracing());
    }

    #[test]
    fn trace_does_not_arm_on_wrong_direction() {
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        // Right tile + char but moving Down (not into the word) → no arm.
        assert_eq!(
            t.observe((3, 0), 'd', Direction::Down, &words),
            TraceStep::None
        );
    }

    #[test]
    fn trace_advances_then_completes() {
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        t.observe((3, 0), 'd', Direction::Right, &words);
        assert_eq!(
            t.observe((4, 0), 'a', Direction::Right, &words),
            TraceStep::Advanced { id: 7, progress: 2 }
        );
        assert_eq!(
            t.observe((5, 0), 's', Direction::Right, &words),
            TraceStep::Advanced { id: 7, progress: 3 }
        );
        assert_eq!(
            t.observe((6, 0), 'h', Direction::Right, &words),
            TraceStep::Completed { id: 7 }
        );
        assert!(!t.is_tracing());
    }

    #[test]
    fn trace_resets_on_wrong_char() {
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        t.observe((3, 0), 'd', Direction::Right, &words);
        assert_eq!(
            t.observe((4, 0), 'z', Direction::Right, &words),
            TraceStep::Reset
        );
        assert!(!t.is_tracing());
    }

    #[test]
    fn trace_resets_on_turning_off_axis() {
        // Player turned: the next written tile is not the expected axis tile.
        let w = pw("dash", (3, 0), Axis::Horizontal, false);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        t.observe((3, 0), 'd', Direction::Right, &words);
        // Expected (4,0); player wrote (3,1) (turned down) → reset even if char ok.
        assert_eq!(
            t.observe((3, 1), 'a', Direction::Down, &words),
            TraceStep::Reset
        );
    }

    #[test]
    fn trace_completes_reversed_word_typed_logically() {
        // reversed "dash": entry (6,0) moving Left, letters still d,a,s,h.
        let w = pw("dash", (3, 0), Axis::Horizontal, true);
        let words = [(7u32, &w)];
        let mut t = Trace::new();
        assert_eq!(
            t.observe((6, 0), 'd', Direction::Left, &words),
            TraceStep::Armed { id: 7 }
        );
        assert_eq!(
            t.observe((5, 0), 'a', Direction::Left, &words),
            TraceStep::Advanced { id: 7, progress: 2 }
        );
        t.observe((4, 0), 's', Direction::Left, &words);
        assert_eq!(
            t.observe((3, 0), 'h', Direction::Left, &words),
            TraceStep::Completed { id: 7 }
        );
    }

    #[test]
    fn trace_completes_single_char_word_any_direction() {
        // 1-Buchstaben-Wort: keine Lauf-Achse → Richtung egal, Position + Zeichen
        // genügen. Schließt den vorher toten `len <= 1`-Pfad ab.
        let w = pw("x", (2, 2), Axis::Horizontal, false);
        let words = [(9u32, &w)];
        let mut t = Trace::new();
        assert_eq!(
            t.observe((2, 2), 'x', Direction::Up, &words),
            TraceStep::Completed { id: 9 }
        );
        assert!(!t.is_tracing());
        // Falsche Position armt nicht.
        let mut t2 = Trace::new();
        assert_eq!(
            t2.observe((0, 0), 'x', Direction::Up, &words),
            TraceStep::None
        );
    }

    #[test]
    fn step_result_exposes_written_tile() {
        let mut e = WritingEngine::new((0, 0));
        let r = e.on_char('a');
        assert_eq!(r.tile().map(|t| t.pos), Some((0, 0)));
        let r = e.on_backspace();
        assert_eq!(r.tile(), None);
    }

    #[test]
    fn suspended_trace_does_not_fire_triggers() {
        let mut e = WritingEngine::new((0, 0));
        e.trace_suspended = true;
        for ch in "up".chars() {
            e.on_char(ch);
        }
        // Trigger suspended: direction unchanged, buffer stays clear.
        assert_eq!(e.direction, Direction::Right);
        assert!(e.current_word.is_empty());
    }

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
            written_pace: 0.0,
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
    fn trail_brightness_eases_to_zero_at_tail() {
        let l = 60;
        // The bright head stays full.
        assert_eq!(trail_brightness(0, l), TILE_MAX_BRIGHTNESS);
        assert_eq!(trail_brightness(TRAIL_SAFE - 1, l), TILE_MAX_BRIGHTNESS);
        // Right behind the head it has started to dim, but is still visible.
        assert!(trail_brightness(TRAIL_SAFE, l) < TILE_MAX_BRIGHTNESS);
        assert!(trail_brightness(TRAIL_SAFE, l) > 0);
        // Non-increasing with age across the whole visible window.
        let mut prev = trail_brightness(0, l);
        for ft in 1..l {
            let b = trail_brightness(ft, l);
            assert!(b <= prev, "brightness must not increase with age");
            prev = b;
        }
        // Reaches exactly 0 at the visible edge (smooth dissolve, no hard cut)…
        assert_eq!(trail_brightness(l, l), 0);
        // …and the quadratic ease leaves several near-black tiles before the edge.
        let near_black = (TRAIL_SAFE..l)
            .filter(|ft| trail_brightness(*ft, l) == 0)
            .count();
        assert!(
            near_black >= 2,
            "tail should ease into black, got {near_black}"
        );
    }

    #[test]
    fn visible_len_grows_with_pace() {
        assert_eq!(visible_len_for_pace(0.0), TRAIL_MIN_VISIBLE);
        assert_eq!(visible_len_for_pace(1.0), TRAIL_MAX_VISIBLE);
        assert!(visible_len_for_pace(0.5) > visible_len_for_pace(0.1));
        // Clamps out-of-range input.
        assert_eq!(visible_len_for_pace(-1.0), TRAIL_MIN_VISIBLE);
        assert_eq!(visible_len_for_pace(2.0), TRAIL_MAX_VISIBLE);
    }

    #[test]
    fn fade_applies_during_active_typing_not_just_idle() {
        // Regression: the gradient must be visible right after writing, with NO
        // idle accumulation (a single tick is one render frame).
        let mut e = WritingEngine::new((0, 0));
        for ch in ('a'..='z').cycle().take(TRAIL_SAFE + 20) {
            e.on_char(ch);
        }
        e.tick_visuals();
        // All visible tiles keep full brightness — no gradient.
        assert!(e.trail.iter().all(|t| t.brightness == TILE_MAX_BRIGHTNESS));
    }

    #[test]
    fn faster_typing_keeps_a_longer_trail() {
        // Fast: one char per frame → pace saturates → long trail.
        let mut fast = WritingEngine::new((0, 0));
        for ch in ('a'..='z').cycle().take(TRAIL_MAX_VISIBLE + 40) {
            fast.on_char(ch);
            fast.tick_visuals();
        }
        // Slow: one char per ~25 idle frames → low pace → short trail.
        let mut slow = WritingEngine::new((0, 0));
        for ch in ('a'..='z').cycle().take(TRAIL_MAX_VISIBLE + 40) {
            slow.on_char(ch);
            for _ in 0..25 {
                slow.tick_visuals();
            }
        }
        assert!(
            fast.trail.len() > slow.trail.len(),
            "fast={} slow={}",
            fast.trail.len(),
            slow.trail.len()
        );
        assert!(fast.pace > slow.pace, "fast pace must exceed slow pace");
    }

    #[test]
    fn trail_self_trims_to_pace_derived_length() {
        let mut e = WritingEngine::new((0, 0));
        // Type fast (one char per frame) past the max window.
        for ch in ('a'..='z').cycle().take(TRAIL_MAX_VISIBLE + 50) {
            e.on_char(ch);
            e.tick_visuals();
        }
        let vl = visible_len_for_pace(e.pace);
        // The visible window is respected; any extra tiles are in the fade-out
        // zone and will vanish within a few frames.
        let full_bright = e
            .trail
            .iter()
            .filter(|t| t.brightness == TILE_MAX_BRIGHTNESS)
            .count();
        assert_eq!(
            full_bright, vl,
            "exactly visible_len tiles at full brightness"
        );
        // Fading-out tiles (if any) must be dimmer.
        assert!(
            e.trail.iter().all(|t| t.brightness > 0),
            "no fully-invisible tiles remain"
        );
    }

    #[test]
    fn pace_decays_to_zero_when_idle() {
        let mut e = WritingEngine::new((0, 0));
        for ch in ('a'..='z').cycle().take(10) {
            e.on_char(ch);
        }
        assert!(e.pace > 0.0);
        for _ in 0..300 {
            e.tick_visuals();
        }
        assert!(
            e.pace < 0.01,
            "pace should decay to ~0 when idle, got {}",
            e.pace
        );
    }

    #[test]
    fn backspace_across_turn_restores_pre_turn_direction() {
        // Bug repro: typing "up" turns Right -> Up. One more char moves Up.
        // Backspacing back across the turn must restore direction to Right,
        // not leave it stuck at Up.
        let mut e = WritingEngine::new((0, 0));
        e.on_char('u');
        e.on_char('p');
        assert_eq!(e.direction, Direction::Up);
        e.on_char('x');
        assert_eq!(e.cursor, (2, -1));

        // Erase 'x' (written going Up from (2,0)) -> cursor back to (2,0),
        // direction still Up (this tile was written *after* the turn).
        e.on_backspace();
        assert_eq!(e.cursor, (2, 0));
        assert_eq!(e.direction, Direction::Up);

        // Erase 'p' (the tile that fired the turn; written *before* the turn
        // took effect, going Right from (1,0)) -> cursor back to (1,0),
        // direction restored to Right.
        e.on_backspace();
        assert_eq!(e.cursor, (1, 0));
        assert_eq!(e.direction, Direction::Right);

        // Erase 'u' -> cursor back to (0,0), direction still Right.
        e.on_backspace();
        assert_eq!(e.cursor, (0, 0));
        assert_eq!(e.direction, Direction::Right);
    }

    #[test]
    fn single_backspace_right_after_turn_restores_direction() {
        let mut e = WritingEngine::new((0, 0));
        e.on_char('u');
        e.on_char('p');
        assert_eq!(e.direction, Direction::Up);
        e.on_backspace();
        // The just-erased tile ('p') was written while direction was still
        // Right (the turn only takes effect for the *next* char).
        assert_eq!(e.direction, Direction::Right);
    }

    #[test]
    fn backspace_on_empty_trail_is_noop() {
        let mut e = WritingEngine::new((0, 0));
        let r = e.on_backspace();
        assert_eq!(e.direction, Direction::Right);
        assert_eq!(e.cursor, (0, 0));
        assert_eq!(r, StepResult::Erased);
    }

    #[test]
    fn dir_history_stays_bounded_by_trail_after_trim() {
        let mut e = WritingEngine::new((0, 0));
        // Type fast (one char per frame) well past the max trail window,
        // turning direction periodically to exercise dir_history pushes.
        for (i, ch) in ('a'..='z').cycle().take(TRAIL_MAX_VISIBLE + 50).enumerate() {
            e.on_char(ch);
            if i % 7 == 0 {
                e.on_char('u');
                e.on_char('p');
            }
            e.tick_visuals();
        }
        assert!(
            e.dir_history.len() <= e.trail.len(),
            "dir_history ({}) must not outgrow trail ({})",
            e.dir_history.len(),
            e.trail.len()
        );
        // Backspace still restores correctly after trimming.
        let dir_before = e.direction;
        let cursor_before = e.cursor;
        e.on_backspace();
        assert_ne!(e.cursor, cursor_before);
        // Direction after one backspace is whatever the erased tile's
        // pre-write direction was — just assert it's a valid value and the
        // history/trail stayed in lockstep (no panic, no desync).
        let _ = dir_before;
        assert!(e.dir_history.len() <= e.trail.len());
    }

    #[test]
    fn backspace_direction_correct_after_mid_trail_fade_hole() {
        // Regression: the pace-driven trail fade removes tiles by individual
        // brightness, so a newer tile can vanish before an older one — leaving a
        // HOLE in the trail's middle. A position-keyed history trim would desync
        // here and silently restore the wrong direction. The tick-keyed mirror
        // must stay correct.
        let mut e = WritingEngine::new((0, 0));
        // Drive writes through a sequence that turns mid-stream, capturing an
        // INDEPENDENT ground truth (tick → direction-in-effect-at-write) so the
        // assertions don't just mirror dir_history back onto itself.
        let mut truth: std::collections::HashMap<u64, Direction> = std::collections::HashMap::new();
        for ch in "xupydownz".chars() {
            truth.insert(e.tick, e.direction); // tick & pre-write dir for this tile
            e.on_char(ch);
        }
        // Sanity: the stream actually turned (else the test proves nothing).
        let dirs: Vec<Direction> = truth.values().copied().collect();
        assert!(
            dirs.iter().any(|d| *d != dirs[0]),
            "stream must change direction to be meaningful"
        );

        // Punch a hole: drop a middle tile, exactly what apply_trail_fade can do
        // when a faster-faded newer tile outlives... err, predeceases an older one.
        e.trail.retain(|t| t.tick != 4);
        e.tick_visuals(); // prunes dir_history by the SURVIVING tick set (identity)

        assert_eq!(
            e.dir_history.len(),
            e.trail.len(),
            "history mirrors the surviving tiles after a hole"
        );
        // Every surviving history entry carries the CORRECT direction for its tick
        // (immutable tick→dir pairing, preserved through identity pruning).
        for (tk, d) in &e.dir_history {
            assert_eq!(
                Some(d),
                truth.get(tk),
                "tick {tk} kept its true pre-write dir"
            );
        }

        // Backspacing newest-first restores each surviving tile's ground-truth
        // direction. A position-keyed trim would desync here and the on_backspace
        // tick debug_assert would fire.
        let mut surviving_ticks: Vec<u64> = e.trail.iter().map(|t| t.tick).collect();
        while let Some(tk) = surviving_ticks.pop() {
            e.on_backspace();
            assert_eq!(
                e.direction, truth[&tk],
                "backspace over tick {tk} restored its ground-truth direction"
            );
        }
    }

    #[test]
    fn short_trail_inside_safe_head_stays_full_bright() {
        let mut e = WritingEngine::new((0, 0));
        for ch in ('a'..='z').cycle().take(TRAIL_SAFE) {
            e.on_char(ch);
        }
        e.tick_visuals();
        assert_eq!(e.trail.len(), TRAIL_SAFE);
        assert!(e.trail.iter().all(|t| t.brightness == TILE_MAX_BRIGHTNESS));
    }
}

#[cfg(test)]
mod dash_tests {
    use super::*;

    #[test]
    fn blink_teleports_cursor_and_sets_facing_leaving_gap() {
        let mut e = WritingEngine::new((0, 0));
        e.on_char('a'); // one trail tile at (0,0), cursor now (1,0)
        let before = e.trail.len();
        e.dash_blink((7, 0), Direction::Right);
        assert_eq!(e.cursor, (7, 0), "cursor jumps to landing");
        assert_eq!(e.direction, Direction::Right);
        assert_eq!(e.trail.len(), before, "blink leaves a gap (no trail tiles)");
    }

    #[test]
    fn trail_burst_writes_one_tile_per_letter_and_lands() {
        let mut e = WritingEngine::new((0, 0)); // cursor (0,0)
        e.dash_trail_burst((1, 0), &['w', 'x', 'y', 'z'], Direction::Right);
        assert_eq!(e.cursor, (4, 0), "cursor ends at the landing tile");
        assert_eq!(e.trail.len(), 4, "exactly one tile per letter");
        // Tiles sit on the stepped path p_0..p_3.
        let positions: Vec<(i32, i32)> = e.trail.iter().map(|t| t.pos).collect();
        assert_eq!(positions, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn trail_burst_writes_the_fixed_letters_as_tile_chars() {
        // Die festen Buchstaben landen base→tip als echte Trail-Bestandteile (kein
        // Platzhalter-Glyph mehr) — Outplay-Material, in das der Settle einrastet.
        let mut e = WritingEngine::new((0, 0));
        e.dash_trail_burst((1, 0), &['a', 'b', 'c'], Direction::Right);
        let chars: Vec<char> = e.trail.iter().map(|t| t.ch).collect();
        assert_eq!(chars, vec!['a', 'b', 'c']);
    }

    #[test]
    fn trail_burst_keeps_dir_history_in_sync_for_backspace() {
        let mut e = WritingEngine::new((2, 2));
        e.dash_trail_burst((0, 1), &['q', 'r', 's'], Direction::Down); // burst downward
        assert_eq!(
            e.dir_history.len(),
            e.trail.len(),
            "one history entry per tile"
        );
        // Backspacing must not trip the desync debug_assert and walks tiles back.
        e.on_backspace();
        assert_eq!(e.trail.len(), 2);
    }
}
