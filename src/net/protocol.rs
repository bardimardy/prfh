use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::game::world::{PlayerColor, PlayerId, PlayerSnapshot};
use crate::game::writing::{Direction, Tile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEvent {
    Char(char),
    Backspace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    Hello { name: String },
    Input(InputEvent),
    Bye,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome {
        your_id: PlayerId,
        color: PlayerColor,
        players: Vec<PlayerSnapshot>,
    },
    PlayerJoined {
        id: PlayerId,
        color: PlayerColor,
        name: String,
    },
    PlayerLeft {
        id: PlayerId,
    },
    Wrote {
        id: PlayerId,
        tile: Tile,
        cursor: (i32, i32),
        direction: Direction,
        glow_len: u8,
    },
    Erased {
        id: PlayerId,
        cursor: (i32, i32),
    },
    Reject {
        reason: String,
    },
    Died {
        id: PlayerId,
    },
    Respawned {
        id: PlayerId,
        pos: (i32, i32),
    },
}

/// Serialize a message to a single compact RON line terminated by '\n'.
pub fn encode_line<T: Serialize>(msg: &T) -> String {
    let mut s = ron::ser::to_string(msg).expect("RON serialization cannot fail for our types");
    s.push('\n');
    s
}

/// Parse one RON line into a message.
pub fn decode_line<T: DeserializeOwned>(line: &str) -> Result<T> {
    Ok(ron::from_str(line.trim_end())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::world::PALETTE;

    #[test]
    fn client_msg_roundtrip() {
        for msg in [
            ClientMsg::Hello { name: "Max".into() },
            ClientMsg::Input(InputEvent::Char('z')),
            ClientMsg::Input(InputEvent::Backspace),
            ClientMsg::Bye,
        ] {
            let line = encode_line(&msg);
            assert!(line.ends_with('\n'));
            assert!(
                !line.trim_end().contains('\n'),
                "compact RON must be single-line"
            );
            let back: ClientMsg = decode_line(&line).unwrap();
            assert_eq!(msg, back);
        }
    }

    #[test]
    fn server_msg_wrote_roundtrip() {
        let msg = ServerMsg::Wrote {
            id: 2,
            tile: Tile {
                pos: (4, 1),
                ch: 'q',
                tick: 9,
                glow: 0,
                brightness: crate::game::writing::TILE_MAX_BRIGHTNESS,
            },
            cursor: (5, 1),
            direction: Direction::Down,
            glow_len: 0,
        };
        let back: ServerMsg = decode_line(&encode_line(&msg)).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn server_msg_welcome_roundtrip() {
        let msg = ServerMsg::Welcome {
            your_id: 1,
            color: PALETTE[1],
            players: vec![],
        };
        let back: ServerMsg = decode_line(&encode_line(&msg)).unwrap();
        assert_eq!(msg, back);
    }
}
