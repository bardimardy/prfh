use serde::{Deserialize, Serialize};

use crate::game::writing::{Direction, Tile, GLOW_TICKS};
use crate::net::protocol::ServerMsg;

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
    PlayerColor {
        r: 90,
        g: 220,
        b: 120,
    },
    PlayerColor {
        r: 90,
        g: 200,
        b: 230,
    },
    PlayerColor {
        r: 220,
        g: 110,
        b: 210,
    },
    PlayerColor {
        r: 235,
        g: 210,
        b: 90,
    },
    PlayerColor {
        r: 120,
        g: 150,
        b: 245,
    },
    PlayerColor {
        r: 235,
        g: 100,
        b: 100,
    },
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub id: PlayerId,
    pub color: PlayerColor,
    pub name: String,
    pub trail: Vec<Tile>,
    pub cursor: (i32, i32),
    pub direction: Direction,
    pub is_dead: bool,
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
    pub is_dead: bool,
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
        Self {
            players: Vec::new(),
            self_id,
        }
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

    /// Apply a server message to the view (client-side state machine).
    /// `Welcome` and `Reject` are handled at connect time, not here.
    pub fn apply(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::Welcome {
                your_id, players, ..
            } => {
                self.self_id = your_id;
                self.players = players
                    .into_iter()
                    .map(|s| PlayerView {
                        is_self: s.id == your_id,
                        is_dead: s.is_dead,
                        id: s.id,
                        color: s.color,
                        name: s.name,
                        trail: s.trail,
                        cursor: s.cursor,
                        direction: s.direction,
                    })
                    .collect();
            }
            ServerMsg::PlayerJoined { id, color, name } => {
                if !self.players.iter().any(|p| p.id == id) {
                    self.players.push(PlayerView {
                        id,
                        color,
                        name,
                        trail: Vec::new(),
                        cursor: (0, 0),
                        direction: Direction::Right,
                        is_self: id == self.self_id,
                        is_dead: false,
                    });
                }
            }
            ServerMsg::PlayerLeft { id } => {
                self.players.retain(|p| p.id != id);
            }
            ServerMsg::Wrote {
                id,
                tile,
                cursor,
                direction,
                glow_len,
            } => {
                if let Some(p) = self.player_mut(id) {
                    p.push_tile(tile);
                    p.cursor = cursor;
                    p.direction = direction;
                    let n = p.trail.len();
                    let start = n.saturating_sub(glow_len as usize);
                    for t in &mut p.trail[start..n] {
                        t.glow = GLOW_TICKS;
                    }
                }
            }
            ServerMsg::Erased { id, cursor } => {
                if let Some(p) = self.player_mut(id) {
                    p.trail.pop();
                    p.cursor = cursor;
                }
            }
            ServerMsg::Reject { .. } => {}
            ServerMsg::Died { id } => {
                if let Some(p) = self.player_mut(id) {
                    p.trail.clear();
                    p.is_dead = true;
                }
            }
            ServerMsg::Respawned { id, pos } => {
                if let Some(p) = self.player_mut(id) {
                    p.trail.clear();
                    p.cursor = pos;
                    p.is_dead = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            is_dead: false,
        });
        w
    }

    #[test]
    fn push_tile_enforces_trail_cap() {
        let mut w = view_with_one_player();
        let p = w.player_mut(1).unwrap();
        for i in 0..(TRAIL_CAP + 5) {
            p.push_tile(Tile {
                pos: (i as i32, 0),
                ch: 'a',
                tick: i as u64,
                glow: 0,
                brightness: crate::game::writing::TILE_MAX_BRIGHTNESS,
            });
        }
        assert_eq!(p.trail.len(), TRAIL_CAP);
        // Oldest dropped: first remaining tile is the 5th pushed.
        assert_eq!(p.trail[0].pos, (5, 0));
    }

    #[test]
    fn tick_visuals_decrements_glow_to_zero() {
        let mut w = view_with_one_player();
        w.player_mut(1).unwrap().push_tile(Tile {
            pos: (0, 0),
            ch: 'x',
            tick: 0,
            glow: GLOW_TICKS,
            brightness: crate::game::writing::TILE_MAX_BRIGHTNESS,
        });
        w.tick_visuals();
        assert_eq!(w.players[0].trail[0].glow, GLOW_TICKS - 1);
        for _ in 0..GLOW_TICKS + 5 {
            w.tick_visuals();
        }
        assert_eq!(w.players[0].trail[0].glow, 0);
    }

    #[test]
    fn apply_welcome_populates_players_and_self() {
        use crate::net::protocol::ServerMsg;
        let mut w = WorldView::new(0);
        w.apply(ServerMsg::Welcome {
            your_id: 2,
            color: PALETTE[2],
            players: vec![PlayerSnapshot {
                id: 2,
                color: PALETTE[2],
                name: "Me".into(),
                trail: vec![],
                cursor: (1, 1),
                direction: Direction::Up,
                is_dead: false,
            }],
        });
        assert_eq!(w.self_id, 2);
        assert_eq!(w.players.len(), 1);
        assert!(w.players[0].is_self);
    }

    #[test]
    fn apply_wrote_appends_tile_sets_glow_and_cursor() {
        use crate::net::protocol::ServerMsg;
        let mut w = view_with_one_player(); // self_id = 1, player 1
        w.apply(ServerMsg::Wrote {
            id: 1,
            tile: Tile {
                pos: (0, 0),
                ch: 'u',
                tick: 0,
                glow: 0,
                brightness: crate::game::writing::TILE_MAX_BRIGHTNESS,
            },
            cursor: (1, 0),
            direction: Direction::Right,
            glow_len: 0,
        });
        w.apply(ServerMsg::Wrote {
            id: 1,
            tile: Tile {
                pos: (1, 0),
                ch: 'p',
                tick: 1,
                glow: 0,
                brightness: crate::game::writing::TILE_MAX_BRIGHTNESS,
            },
            cursor: (2, 0),
            direction: Direction::Up,
            glow_len: 2, // trigger fired: last two tiles glow
        });
        let p = &w.players[0];
        assert_eq!(p.trail.len(), 2);
        assert_eq!(p.cursor, (2, 0));
        assert_eq!(p.direction, Direction::Up);
        assert_eq!(p.trail[0].glow, GLOW_TICKS);
        assert_eq!(p.trail[1].glow, GLOW_TICKS);
    }

    #[test]
    fn apply_erased_pops_tile() {
        use crate::net::protocol::ServerMsg;
        let mut w = view_with_one_player();
        w.player_mut(1).unwrap().push_tile(Tile {
            pos: (0, 0),
            ch: 'a',
            tick: 0,
            glow: 0,
            brightness: crate::game::writing::TILE_MAX_BRIGHTNESS,
        });
        w.apply(ServerMsg::Erased {
            id: 1,
            cursor: (0, 0),
        });
        assert!(w.players[0].trail.is_empty());
        assert_eq!(w.players[0].cursor, (0, 0));
    }

    #[test]
    fn apply_player_left_removes() {
        use crate::net::protocol::ServerMsg;
        let mut w = view_with_one_player();
        w.apply(ServerMsg::PlayerLeft { id: 1 });
        assert!(w.players.is_empty());
    }
}
