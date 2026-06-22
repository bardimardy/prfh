use serde::{Deserialize, Serialize};

use crate::game::writing::{Direction, Tile};

pub type PlayerId = u8;

pub const MAX_PLAYERS: usize = 6;
pub const TRAIL_CAP: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub const PALETTE: [PlayerColor; MAX_PLAYERS] = [
    PlayerColor { r: 90, g: 220, b: 120 },
    PlayerColor { r: 90, g: 200, b: 230 },
    PlayerColor { r: 220, g: 110, b: 210 },
    PlayerColor { r: 235, g: 210, b: 90 },
    PlayerColor { r: 120, g: 150, b: 245 },
    PlayerColor { r: 235, g: 100, b: 100 },
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub id: PlayerId,
    pub color: PlayerColor,
    pub name: String,
    pub trail: Vec<Tile>,
    pub cursor: (i32, i32),
    pub direction: Direction,
}

#[derive(Debug, Clone)]
pub struct PlayerView {
    pub id: PlayerId,
    pub color: PlayerColor,
    pub name: String,
    pub trail: Vec<Tile>,
    pub cursor: (i32, i32),
    pub direction: Direction,
    pub is_self: bool,
}

impl PlayerView {
    /// Push a tile, enforcing the trail cap (drop oldest when full).
    pub fn push_tile(&mut self, tile: Tile) {
        if self.trail.len() >= TRAIL_CAP {
            self.trail.remove(0);
        }
        self.trail.push(tile);
    }
}

#[derive(Debug, Clone)]
pub struct WorldView {
    pub players: Vec<PlayerView>,
    pub self_id: PlayerId,
}

impl WorldView {
    pub fn new(self_id: PlayerId) -> Self {
        Self { players: Vec::new(), self_id }
    }

    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut PlayerView> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    /// Decrement glow on every tile of every player (called once per frame).
    pub fn tick_visuals(&mut self) {
        for p in &mut self.players {
            for t in &mut p.trail {
                if t.glow > 0 {
                    t.glow -= 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::writing::GLOW_TICKS;

    fn view_with_one_player() -> WorldView {
        let mut w = WorldView::new(1);
        w.players.push(PlayerView {
            id: 1,
            color: PALETTE[0],
            name: "P1".into(),
            trail: Vec::new(),
            cursor: (0, 0),
            direction: Direction::Right,
            is_self: true,
        });
        w
    }

    #[test]
    fn push_tile_enforces_trail_cap() {
        let mut w = view_with_one_player();
        let p = w.player_mut(1).unwrap();
        for i in 0..(TRAIL_CAP + 5) {
            p.push_tile(Tile { pos: (i as i32, 0), ch: 'a', tick: i as u64, glow: 0 });
        }
        assert_eq!(p.trail.len(), TRAIL_CAP);
        // Oldest dropped: first remaining tile is the 5th pushed.
        assert_eq!(p.trail[0].pos, (5, 0));
    }

    #[test]
    fn tick_visuals_decrements_glow_to_zero() {
        let mut w = view_with_one_player();
        w.player_mut(1).unwrap().push_tile(Tile { pos: (0, 0), ch: 'x', tick: 0, glow: GLOW_TICKS });
        w.tick_visuals();
        assert_eq!(w.players[0].trail[0].glow, GLOW_TICKS - 1);
        for _ in 0..GLOW_TICKS + 5 {
            w.tick_visuals();
        }
        assert_eq!(w.players[0].trail[0].glow, 0);
    }
}
