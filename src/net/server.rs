use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::game::world::{
    PlayerColor, PlayerId, PlayerSnapshot, PlayerView, WorldView, MAX_PLAYERS, PALETTE,
};
use crate::game::writing::{StepResult, WritingEngine, GLOW_TICKS};
use crate::net::protocol::{decode_line, encode_line, ClientMsg, InputEvent, ServerMsg};

pub enum HostEvent {
    Hello {
        conn_id: u64,
        name: String,
        write: TcpStream,
    },
    Input {
        conn_id: u64,
        ev: InputEvent,
    },
    Disconnected {
        conn_id: u64,
    },
}

/// Spawn the accept loop. Each accepted connection gets a reader thread that
/// first expects a `Hello`, then streams `Input` events, until EOF.
pub fn spawn_listener(listener: TcpListener) -> Receiver<HostEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let counter = AtomicU64::new(1);
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let conn_id = counter.fetch_add(1, Ordering::Relaxed);
            let tx = tx.clone();
            thread::spawn(move || reader_loop(conn_id, stream, tx));
        }
    });
    rx
}

fn reader_loop(conn_id: u64, stream: TcpStream, tx: Sender<HostEvent>) {
    let write = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    // First line must be Hello.
    line.clear();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    match decode_line::<ClientMsg>(&line) {
        Ok(ClientMsg::Hello { name }) => {
            if tx
                .send(HostEvent::Hello {
                    conn_id,
                    name,
                    write,
                })
                .is_err()
            {
                return;
            }
        }
        _ => return,
    }

    // Subsequent lines are Input / Bye.
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => match decode_line::<ClientMsg>(&line) {
                Ok(ClientMsg::Input(ev)) => {
                    if tx.send(HostEvent::Input { conn_id, ev }).is_err() {
                        break;
                    }
                }
                Ok(ClientMsg::Bye) | Err(_) => break,
                Ok(ClientMsg::Hello { .. }) => {} // ignore duplicate hello
            },
            Err(_) => break,
        }
    }
    let _ = tx.send(HostEvent::Disconnected { conn_id });
}

/// Write one server message to a stream (best-effort).
pub fn send_msg(stream: &mut TcpStream, msg: &ServerMsg) -> std::io::Result<()> {
    stream.write_all(encode_line(msg).as_bytes())
}

pub const HOST_ID: PlayerId = 0;

struct Player {
    engine: WritingEngine,
    color_idx: usize,
    name: String,
}

pub struct JoinOutcome {
    pub id: PlayerId,
    pub color: PlayerColor,
    pub welcome: ServerMsg,
    pub joined: ServerMsg,
}

pub struct HostState {
    players: BTreeMap<PlayerId, Player>,
    join_seq: u32,
}

impl HostState {
    pub fn new(host_name: String) -> Self {
        let mut s = Self {
            players: BTreeMap::new(),
            join_seq: 0,
        };
        // Host always exists as id 0, color index 0, spawn (0,0).
        s.insert_player(HOST_ID, 0, host_name);
        s
    }

    /// Returns the lowest free player id (>= 1), or None if all 255 slots are taken.
    fn next_free_id(&self) -> Option<PlayerId> {
        (1..=u8::MAX).find(|id| !self.players.contains_key(id))
    }

    fn insert_player(&mut self, id: PlayerId, color_idx: usize, name: String) {
        let seq = self.join_seq as i32;
        self.join_seq += 1;
        let spawn = (seq * 12, seq * 4);
        self.players.insert(
            id,
            Player {
                engine: WritingEngine::new(spawn),
                color_idx,
                name,
            },
        );
    }

    fn free_color_idx(&self) -> Option<usize> {
        (0..MAX_PLAYERS).find(|idx| !self.players.values().any(|p| p.color_idx == *idx))
    }

    pub fn add_player(&mut self, name: String) -> Result<JoinOutcome, String> {
        let color_idx = self
            .free_color_idx()
            .ok_or_else(|| format!("Spiel voll (max {} Spieler)", MAX_PLAYERS))?;
        let id = self
            .next_free_id()
            .ok_or_else(|| "Spiel voll (alle Spieler-IDs belegt)".to_string())?;
        self.insert_player(id, color_idx, name.clone());
        let color = PALETTE[color_idx];
        let welcome = ServerMsg::Welcome {
            your_id: id,
            color,
            players: self.snapshot(),
        };
        let joined = ServerMsg::PlayerJoined { id, color, name };
        Ok(JoinOutcome {
            id,
            color,
            welcome,
            joined,
        })
    }

    pub fn remove_player(&mut self, id: PlayerId) -> Option<ServerMsg> {
        if id == HOST_ID {
            return None; // host leaving ends the session elsewhere
        }
        self.players
            .remove(&id)
            .map(|_| ServerMsg::PlayerLeft { id })
    }

    pub fn apply_input(&mut self, id: PlayerId, ev: InputEvent) -> Option<ServerMsg> {
        let player = self.players.get_mut(&id)?;
        match ev {
            InputEvent::Char(c) => {
                let result = player.engine.on_char(c);
                let tile = player.engine.trail.last().cloned()?;
                let glow_len = match result {
                    StepResult::WroteAndTurned(..) | StepResult::WroteAndStopped(..) => player
                        .engine
                        .trail
                        .iter()
                        .rev()
                        .take_while(|t| t.glow == GLOW_TICKS)
                        .count()
                        .min(u8::MAX as usize)
                        as u8,
                    _ => 0,
                };
                Some(ServerMsg::Wrote {
                    id,
                    tile,
                    cursor: player.engine.cursor,
                    direction: player.engine.direction,
                    glow_len,
                })
            }
            InputEvent::Backspace => {
                player.engine.on_backspace();
                Some(ServerMsg::Erased {
                    id,
                    cursor: player.engine.cursor,
                })
            }
        }
    }

    pub fn tick_visuals(&mut self) {
        for p in self.players.values_mut() {
            p.engine.tick_visuals();
        }
    }

    pub fn snapshot(&self) -> Vec<PlayerSnapshot> {
        self.players
            .iter()
            .map(|(id, p)| PlayerSnapshot {
                id: *id,
                color: PALETTE[p.color_idx],
                name: p.name.clone(),
                trail: p.engine.trail.clone(),
                cursor: p.engine.cursor,
                direction: p.engine.direction,
            })
            .collect()
    }

    pub fn world_view(&self) -> WorldView {
        let players = self
            .players
            .iter()
            .map(|(id, p)| PlayerView {
                id: *id,
                color: PALETTE[p.color_idx],
                name: p.name.clone(),
                trail: p.engine.trail.clone(),
                cursor: p.engine.cursor,
                direction: p.engine.direction,
                is_self: *id == HOST_ID,
            })
            .collect();
        WorldView {
            players,
            self_id: HOST_ID,
        }
    }

    pub fn self_id(&self) -> PlayerId {
        HOST_ID
    }

    pub fn local_engine(&self) -> &WritingEngine {
        &self.players[&HOST_ID].engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::writing::Direction;

    #[test]
    fn host_exists_with_color_zero() {
        let s = HostState::new("Host".into());
        let wv = s.world_view();
        assert_eq!(wv.players.len(), 1);
        assert_eq!(wv.players[0].id, HOST_ID);
        assert_eq!(wv.players[0].color, PALETTE[0]);
    }

    #[test]
    fn add_players_get_distinct_colors_in_order() {
        let mut s = HostState::new("Host".into());
        let a = s.add_player("A".into()).unwrap();
        let b = s.add_player("B".into()).unwrap();
        assert_eq!(a.color, PALETTE[1]);
        assert_eq!(b.color, PALETTE[2]);
        assert_eq!(a.id, 1);
        assert_eq!(b.id, 2);
    }

    #[test]
    fn full_game_rejects_seventh_player() {
        let mut s = HostState::new("Host".into());
        for _ in 0..(MAX_PLAYERS - 1) {
            s.add_player("x".into()).unwrap();
        }
        assert!(s.add_player("overflow".into()).is_err());
    }

    #[test]
    fn leaving_player_frees_color() {
        let mut s = HostState::new("Host".into());
        let a = s.add_player("A".into()).unwrap(); // color idx 1
        s.remove_player(a.id);
        let c = s.add_player("C".into()).unwrap();
        assert_eq!(c.color, PALETTE[1]); // reused freed slot
    }

    #[test]
    fn apply_char_produces_wrote_and_advances() {
        let mut s = HostState::new("Host".into());
        let msg = s.apply_input(HOST_ID, InputEvent::Char('h')).unwrap();
        match msg {
            ServerMsg::Wrote { id, cursor, .. } => {
                assert_eq!(id, HOST_ID);
                assert_eq!(cursor, (1, 0)); // moved right from (0,0)
            }
            _ => panic!("expected Wrote"),
        }
    }

    #[test]
    fn apply_trigger_sets_glow_len() {
        let mut s = HostState::new("Host".into());
        s.apply_input(HOST_ID, InputEvent::Char('u')).unwrap();
        let msg = s.apply_input(HOST_ID, InputEvent::Char('p')).unwrap();
        match msg {
            ServerMsg::Wrote {
                glow_len,
                direction,
                ..
            } => {
                assert_eq!(direction, Direction::Up);
                assert_eq!(glow_len, 2);
            }
            _ => panic!("expected Wrote"),
        }
    }

    #[test]
    fn add_player_reuses_lowest_free_id() {
        let mut s = HostState::new("Host".into());
        let a = s.add_player("A".into()).unwrap();
        let b = s.add_player("B".into()).unwrap();
        assert_eq!(a.id, 1);
        assert_eq!(b.id, 2);
        // ids never equal HOST_ID
        assert_ne!(a.id, HOST_ID);
        assert_ne!(b.id, HOST_ID);
        // remove player 1 (lowest non-host id)
        s.remove_player(a.id);
        // next joiner should reuse id 1 (lowest free), not 3
        let c = s.add_player("C".into()).unwrap();
        assert_eq!(c.id, 1, "should reuse freed id 1, not allocate 3");
        assert_ne!(c.id, HOST_ID);
    }

    #[test]
    fn snapshot_reflects_written_tiles() {
        let mut s = HostState::new("Host".into());
        s.apply_input(HOST_ID, InputEvent::Char('h')).unwrap();
        s.apply_input(HOST_ID, InputEvent::Char('i')).unwrap();
        let snap = s.snapshot();
        let host = snap.iter().find(|p| p.id == HOST_ID).unwrap();
        assert_eq!(host.trail.len(), 2);
    }
}
