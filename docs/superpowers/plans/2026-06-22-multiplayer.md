# LAN-Multiplayer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mehrere Spieler im selben LAN schreiben gleichzeitig auf einem gemeinsamen Spielfeld, jeder mit eigenfarbiger Buchstaben-Spur, in Echtzeit sichtbar.

**Architecture:** Host-Client über TCP. Der Host ist autoritativ und hält eine `WritingEngine` pro Spieler; Clients senden Tastenanschläge und rendern den empfangenen Zustand. Das Rendering hängt ausschließlich von einer neuen `WorldView`-Struktur ab (entkoppelt von Engine und Netzwerk). Discovery per UDP-Broadcast, manuelle IP als Fallback. Netzwerk-I/O läuft in eigenen Threads, verbunden mit der Render-Schleife über `std::sync::mpsc`.

**Tech Stack:** Rust 2021, Ratatui 0.28, Crossterm 0.28, `serde`+`ron` (Wire-Format), `std::net` + `std::thread` + `std::sync::mpsc`. **Keine neuen Dependencies.**

**Spec:** `docs/superpowers/specs/2026-06-22-multiplayer-design.md`
**Issue:** #25 · **Branch:** `issue-25`

## Global Constraints

- Rust 2021, `cargo fmt`-Stil. `cargo build` und `cargo test` müssen **warnungs- und fehlerfrei** sein. Kein `#[allow]` zum Verstecken; toten Code entfernen.
- **Keine neuen Crate-Dependencies.** Nur `std::net`, `std::thread`, `std::sync::mpsc` + vorhandenes `serde`/`ron`.
- `main` bleibt immer grün. Bestehende `WritingEngine`-Tests bleiben unverändert grün.
- Häufig committen (ein Commit pro Task-Abschluss, Format `feat(#25): …` / `refactor(#25): …` / `test(#25): …`).
- Konstanten (verbindlich): `TCP_PORT = 7777`, `DISCOVERY_PORT = 7778`, `MAX_PLAYERS = 6`, `TRAIL_CAP = 4000`.
- Palette (genau diese 6 RGB-Werte, in dieser Reihenfolge vergeben):
  `(90,220,120)` Grün · `(90,200,230)` Cyan · `(220,110,210)` Magenta · `(235,210,90)` Gelb · `(120,150,245)` Blau · `(235,100,100)` Rot.
- Spawn-Position für Beitritts-Sequenz `k` (Host = 0): `(k*12, k*4)`.
- Leertaste bleibt deaktiviert (kein Tile, kein Schritt) — Filter am Input-Rand, vor dem Senden.

---

## File Structure

**Neu:**
- `src/net/mod.rs` — Modul-Deklaration (`protocol`, `server`, `client`, `discovery`).
- `src/net/protocol.rs` — Message-Enums + RON-Zeilen-Framing.
- `src/net/server.rs` — `HostState` (reine Logik) + Host-Netzwerk-Threads.
- `src/net/client.rs` — Client-Netzwerk-Threads (reine Render-Logik liegt in `WorldView`).
- `src/net/discovery.rs` — UDP-Broadcast Announce/Listen + Lobby-Dedup.
- `src/game/world.rs` — `PlayerId`, `PlayerColor`, `PALETTE`, Konstanten, `PlayerView`, `PlayerSnapshot`, `WorldView` (+ `apply`/`tick_visuals`).

**Geändert:**
- `src/game/writing.rs` — `Serialize`/`Deserialize` auf `Tile` und `Direction`.
- `src/game/mod.rs` — `pub mod world;`.
- `src/lib.rs` — `pub mod net;`.
- `src/render/mod.rs` — `draw_world`/`draw` auf `WorldView` umgestellt + Roster.
- `src/app.rs` — `Mode`-Enum (Single/Host/Client), `world_view()`, `local_engine()`.
- `src/main.rs` — CLI-Dispatch (`host`/`join`), Netzwerk-Threads, Kanal-Integration, Lobby-Prompt.

---

## Task 1: `Tile` und `Direction` serialisierbar machen

**Files:**
- Modify: `src/game/writing.rs:1-7` (Direction), `src/game/writing.rs:77-85` (Tile)

**Interfaces:**
- Produces: `Tile: Serialize + Deserialize`, `Direction: Serialize + Deserialize` (für Protokoll & WorldView).

- [ ] **Step 1: Test schreiben** — in `src/game/writing.rs` im `#[cfg(test)] mod tests`:

```rust
#[test]
fn tile_and_direction_ron_roundtrip() {
    let t = Tile { pos: (3, -2), ch: 'x', tick: 7, glow: GLOW_TICKS };
    let s = ron::to_string(&t).unwrap();
    let back: Tile = ron::from_str(&s).unwrap();
    assert_eq!(t, back);

    let d = Direction::Left;
    let s = ron::to_string(&d).unwrap();
    let back: Direction = ron::from_str(&s).unwrap();
    assert_eq!(d, back);
}
```

- [ ] **Step 2: Test ausführen, Fehlschlag sehen**

Run: `cargo test tile_and_direction_ron_roundtrip`
Expected: Compile-Fehler — `Tile`/`Direction` implementieren `Serialize`/`Deserialize` nicht.

- [ ] **Step 3: Derives ergänzen**

Oben in `writing.rs` Import sicherstellen:
```rust
use serde::{Deserialize, Serialize};
```
Bei `Direction` (Zeile 1) die Ableitung erweitern:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
```
Bei `Tile` (Zeile 77) ebenso:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
```

- [ ] **Step 4: Test ausführen, grün sehen**

Run: `cargo test tile_and_direction_ron_roundtrip`
Expected: PASS. Außerdem `cargo build` warnungsfrei.

- [ ] **Step 5: Commit**

```bash
git add src/game/writing.rs
git commit -m "feat(#25): Tile und Direction serde-serialisierbar"
```

---

## Task 2: `world.rs` — Render-Modell `WorldView`

**Files:**
- Create: `src/game/world.rs`
- Modify: `src/game/mod.rs` (`pub mod world;`)
- Test: in `src/game/world.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Tile`, `Direction`, `GLOW_TICKS` aus `crate::game::writing`.
- Produces:
  - `type PlayerId = u8;`
  - `struct PlayerColor { r: u8, g: u8, b: u8 }` (`Copy`, serde)
  - `const PALETTE: [PlayerColor; 6]`, `const MAX_PLAYERS: usize = 6`, `const TRAIL_CAP: usize = 4000`
  - `struct PlayerSnapshot { id, color, name, trail, cursor, direction }` (serde)
  - `struct PlayerView { id, color, name, trail, cursor, direction, is_self }`
  - `struct WorldView { players: Vec<PlayerView>, self_id: PlayerId }`
  - `WorldView::apply(&mut self, msg: ServerMsg)`, `WorldView::tick_visuals(&mut self)`, `WorldView::player_mut(&mut self, id) -> Option<&mut PlayerView>`

> **Hinweis:** `apply` referenziert `ServerMsg` aus Task 3. Reihenfolge bei der Umsetzung: erst Task 3 (Protokoll), dann `apply` hier ergänzen — ODER `world.rs` zuerst ohne `apply` anlegen und `apply` in Task 3 nachziehen. Empfohlen: **Task 3 vor dem `apply`-Teil von Task 2** umsetzen. Die reinen Typen + `tick_visuals` + Trail-Cap unten sind unabhängig und zuerst dran.

- [ ] **Step 1: Datei mit Typen + Trail-Cap-Push anlegen**

`src/game/world.rs`:
```rust
use serde::{Deserialize, Serialize};

use crate::game::writing::{Direction, Tile, GLOW_TICKS};

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
```

In `src/game/mod.rs` ergänzen:
```rust
pub mod world;
```

- [ ] **Step 2: Tests für Trail-Cap und tick_visuals schreiben**

```rust
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
```

- [ ] **Step 3: Tests ausführen**

Run: `cargo test --lib game::world`
Expected: PASS (beide). `cargo build` warnungsfrei.

- [ ] **Step 4: Commit**

```bash
git add src/game/world.rs src/game/mod.rs
git commit -m "feat(#25): WorldView Render-Modell + Trail-Cap"
```

> `WorldView::apply` wird in Task 3 (Step „apply ergänzen") hinzugefügt, sobald `ServerMsg` existiert.

---

## Task 3: `protocol.rs` — Nachrichten + Zeilen-Framing, und `WorldView::apply`

**Files:**
- Create: `src/net/mod.rs`, `src/net/protocol.rs`
- Modify: `src/lib.rs` (`pub mod net;`), `src/game/world.rs` (`apply` ergänzen)
- Test: in `src/net/protocol.rs` und `src/game/world.rs`

**Interfaces:**
- Consumes: `Tile`, `Direction`, `PlayerId`, `PlayerColor`, `PlayerSnapshot`.
- Produces:
  - `enum InputEvent { Char(char), Backspace }` (serde, Copy)
  - `enum ClientMsg { Hello { name: String }, Input(InputEvent), Bye }` (serde)
  - `enum ServerMsg { Welcome { your_id, color, players: Vec<PlayerSnapshot> }, PlayerJoined { id, color, name }, PlayerLeft { id }, Wrote { id, tile, cursor, direction, glow_len: u8 }, Erased { id, cursor }, Reject { reason: String } }` (serde, Clone)
  - `fn encode_line<T: Serialize>(msg: &T) -> String` (kompaktes RON + `\n`)
  - `fn decode_line<T: DeserializeOwned>(line: &str) -> anyhow::Result<T>`

- [ ] **Step 1: `src/net/mod.rs` und `src/net/protocol.rs` anlegen**

`src/net/mod.rs`:
```rust
pub mod protocol;
pub mod server;
pub mod client;
pub mod discovery;
```
> Falls `server`/`client`/`discovery` noch nicht existieren, beim Anlegen dieser Task die noch fehlenden Module vorerst auskommentieren und in ihren jeweiligen Tasks aktivieren. Empfehlung: hier nur `pub mod protocol;` aktiv lassen und die anderen `pub mod`-Zeilen erst in Task 4/8/5 ergänzen.

`src/net/protocol.rs`:
```rust
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
```

In `src/lib.rs` ergänzen:
```rust
pub mod net;
```

- [ ] **Step 2: Roundtrip-Tests schreiben** (in `src/net/protocol.rs`):

```rust
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
            assert!(!line.trim_end().contains('\n'), "compact RON must be single-line");
            let back: ClientMsg = decode_line(&line).unwrap();
            assert_eq!(msg, back);
        }
    }

    #[test]
    fn server_msg_wrote_roundtrip() {
        let msg = ServerMsg::Wrote {
            id: 2,
            tile: Tile { pos: (4, 1), ch: 'q', tick: 9, glow: 0 },
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
```

- [ ] **Step 3: Tests ausführen**

Run: `cargo test --lib net::protocol`
Expected: PASS.

- [ ] **Step 4: `WorldView::apply` in `world.rs` ergänzen**

In `src/game/world.rs` Import erweitern und Methode hinzufügen:
```rust
use crate::net::protocol::ServerMsg;
```
```rust
impl WorldView {
    /// Apply a server message to the view (client-side state machine).
    /// `Welcome` and `Reject` are handled at connect time, not here.
    pub fn apply(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::Welcome { your_id, players, .. } => {
                self.self_id = your_id;
                self.players = players
                    .into_iter()
                    .map(|s| PlayerView {
                        is_self: s.id == your_id,
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
                    });
                }
            }
            ServerMsg::PlayerLeft { id } => {
                self.players.retain(|p| p.id != id);
            }
            ServerMsg::Wrote { id, tile, cursor, direction, glow_len } => {
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
        }
    }
}
```

- [ ] **Step 5: `apply`-Tests schreiben** (in `src/game/world.rs` tests):

```rust
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
        tile: Tile { pos: (0, 0), ch: 'u', tick: 0, glow: 0 },
        cursor: (1, 0),
        direction: Direction::Right,
        glow_len: 0,
    });
    w.apply(ServerMsg::Wrote {
        id: 1,
        tile: Tile { pos: (1, 0), ch: 'p', tick: 1, glow: 0 },
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
    w.player_mut(1).unwrap().push_tile(Tile { pos: (0, 0), ch: 'a', tick: 0, glow: 0 });
    w.apply(ServerMsg::Erased { id: 1, cursor: (0, 0) });
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
```

- [ ] **Step 6: Tests ausführen**

Run: `cargo test --lib`
Expected: alle PASS, inkl. `game::world` und `net::protocol`. `cargo build` warnungsfrei.

- [ ] **Step 7: Commit**

```bash
git add src/net/mod.rs src/net/protocol.rs src/lib.rs src/game/world.rs
git commit -m "feat(#25): Wire-Protokoll + WorldView::apply"
```

---

## Task 4: `HostState` — autoritative Spiel-Logik (ohne Sockets)

**Files:**
- Create: `src/net/server.rs` (nur `HostState` + Tests in dieser Task; Threads in Task 8)
- Modify: `src/net/mod.rs` (`pub mod server;` aktivieren)
- Test: in `src/net/server.rs`

**Interfaces:**
- Consumes: `WritingEngine`, `StepResult`, `GLOW_TICKS`, `Direction`, `Tile`; `PlayerId`, `PlayerColor`, `PALETTE`, `MAX_PLAYERS`, `PlayerSnapshot`, `PlayerView`, `WorldView`; `InputEvent`, `ServerMsg`.
- Produces:
  - `struct HostState`
  - `HostState::new(host_name: String) -> Self` — legt Host-Spieler (id 0, Farbe 0, Spawn (0,0)) an.
  - `HostState::add_player(&mut self, name: String) -> Result<JoinOutcome, String>` — `Err(reason)` wenn voll (Palette/MAX_PLAYERS erschöpft).
  - `struct JoinOutcome { id: PlayerId, color: PlayerColor, welcome: ServerMsg, joined: ServerMsg }` (`welcome` = `ServerMsg::Welcome` für den Neuen, `joined` = `ServerMsg::PlayerJoined` für die anderen)
  - `HostState::remove_player(&mut self, id: PlayerId) -> Option<ServerMsg>` — gibt Farbe frei, liefert `PlayerLeft`.
  - `HostState::apply_input(&mut self, id: PlayerId, ev: InputEvent) -> Option<ServerMsg>` — liefert `Wrote`/`Erased` zum Broadcast.
  - `HostState::tick_visuals(&mut self)` — tickt alle Engines.
  - `HostState::world_view(&self) -> WorldView` — für das Rendering des Hosts (self_id = 0).
  - `HostState::self_id(&self) -> PlayerId` (= 0)
  - `HostState::local_engine(&self) -> &WritingEngine` (Host-Spieler, für HUD).

- [ ] **Step 1: `HostState` implementieren**

`src/net/server.rs`:
```rust
use std::collections::BTreeMap;

use crate::game::world::{
    PlayerColor, PlayerId, PlayerSnapshot, PlayerView, WorldView, MAX_PLAYERS, PALETTE,
};
use crate::game::writing::{Direction, StepResult, WritingEngine, GLOW_TICKS};
use crate::net::protocol::{InputEvent, ServerMsg};

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
    next_id: PlayerId,
    join_seq: u32,
}

impl HostState {
    pub fn new(host_name: String) -> Self {
        let mut s = Self { players: BTreeMap::new(), next_id: 0, join_seq: 0 };
        // Host always exists as id 0, color index 0, spawn (0,0).
        s.insert_player(HOST_ID, 0, host_name);
        s.next_id = 1;
        s
    }

    fn insert_player(&mut self, id: PlayerId, color_idx: usize, name: String) {
        let seq = self.join_seq as i32;
        self.join_seq += 1;
        let spawn = (seq * 12, seq * 4);
        self.players.insert(
            id,
            Player { engine: WritingEngine::new(spawn), color_idx, name },
        );
    }

    fn free_color_idx(&self) -> Option<usize> {
        (0..MAX_PLAYERS).find(|idx| !self.players.values().any(|p| p.color_idx == *idx))
    }

    pub fn add_player(&mut self, name: String) -> Result<JoinOutcome, String> {
        let color_idx = self
            .free_color_idx()
            .ok_or_else(|| format!("Spiel voll (max {} Spieler)", MAX_PLAYERS))?;
        let id = self.next_id;
        self.next_id += 1;
        self.insert_player(id, color_idx, name.clone());
        let color = PALETTE[color_idx];
        let welcome = ServerMsg::Welcome { your_id: id, color, players: self.snapshot() };
        let joined = ServerMsg::PlayerJoined { id, color, name };
        Ok(JoinOutcome { id, color, welcome, joined })
    }

    pub fn remove_player(&mut self, id: PlayerId) -> Option<ServerMsg> {
        if id == HOST_ID {
            return None; // host leaving ends the session elsewhere
        }
        self.players.remove(&id).map(|_| ServerMsg::PlayerLeft { id })
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
                        .min(u8::MAX as usize) as u8,
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
                Some(ServerMsg::Erased { id, cursor: player.engine.cursor })
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
        WorldView { players, self_id: HOST_ID }
    }

    pub fn self_id(&self) -> PlayerId {
        HOST_ID
    }

    pub fn local_engine(&self) -> &WritingEngine {
        &self.players[&HOST_ID].engine
    }
}
```

In `src/net/mod.rs` `pub mod server;` aktivieren (falls noch auskommentiert).

- [ ] **Step 2: Tests schreiben** (in `src/net/server.rs`):

```rust
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
            ServerMsg::Wrote { glow_len, direction, .. } => {
                assert_eq!(direction, Direction::Up);
                assert_eq!(glow_len, 2);
            }
            _ => panic!("expected Wrote"),
        }
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
```

- [ ] **Step 3: Tests ausführen**

Run: `cargo test --lib net::server`
Expected: alle PASS. `cargo build` warnungsfrei.

- [ ] **Step 4: Commit**

```bash
git add src/net/server.rs src/net/mod.rs
git commit -m "feat(#25): HostState autoritative Spiel-Logik"
```

---

## Task 5: `discovery.rs` — Announce-Paket + Lobby-Dedup (reine Logik)

**Files:**
- Create: `src/net/discovery.rs`
- Modify: `src/net/mod.rs` (`pub mod discovery;` aktivieren)
- Test: in `src/net/discovery.rs`

**Interfaces:**
- Produces:
  - `const DISCOVERY_PORT: u16 = 7778;`, `const TCP_PORT: u16 = 7777;`
  - `struct Announce { name: String, tcp_port: u16 }` (serde)
  - `struct LobbyEntry { addr: std::net::IpAddr, name: String, tcp_port: u16 }`
  - `fn merge_announce(entries: &mut Vec<LobbyEntry>, addr: IpAddr, a: Announce)` — dedup per `addr` (gleiche IP überschreibt).
  - (Socket-Funktionen `spawn_announce`/`discover` werden in Task 9 ergänzt; hier nur reine Logik.)

- [ ] **Step 1: Datei mit Announce + merge_announce anlegen**

`src/net/discovery.rs`:
```rust
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

pub const TCP_PORT: u16 = 7777;
pub const DISCOVERY_PORT: u16 = 7778;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Announce {
    pub name: String,
    pub tcp_port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LobbyEntry {
    pub addr: IpAddr,
    pub name: String,
    pub tcp_port: u16,
}

/// Insert or update a lobby entry keyed by source IP (latest announce wins).
pub fn merge_announce(entries: &mut Vec<LobbyEntry>, addr: IpAddr, a: Announce) {
    if let Some(e) = entries.iter_mut().find(|e| e.addr == addr) {
        e.name = a.name;
        e.tcp_port = a.tcp_port;
    } else {
        entries.push(LobbyEntry { addr, name: a.name, tcp_port: a.tcp_port });
    }
}
```

In `src/net/mod.rs` `pub mod discovery;` aktivieren.

- [ ] **Step 2: Tests schreiben**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn announce_ron_roundtrip() {
        let a = Announce { name: "Hostspiel".into(), tcp_port: TCP_PORT };
        let s = ron::to_string(&a).unwrap();
        let back: Announce = ron::from_str(&s).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn merge_dedups_by_ip() {
        let ip: IpAddr = Ipv4Addr::new(192, 168, 1, 5).into();
        let mut v = Vec::new();
        merge_announce(&mut v, ip, Announce { name: "A".into(), tcp_port: 7777 });
        merge_announce(&mut v, ip, Announce { name: "A2".into(), tcp_port: 7777 });
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "A2");
    }

    #[test]
    fn merge_keeps_distinct_ips() {
        let mut v = Vec::new();
        merge_announce(&mut v, Ipv4Addr::new(192, 168, 1, 5).into(), Announce { name: "A".into(), tcp_port: 7777 });
        merge_announce(&mut v, Ipv4Addr::new(192, 168, 1, 6).into(), Announce { name: "B".into(), tcp_port: 7777 });
        assert_eq!(v.len(), 2);
    }
}
```

- [ ] **Step 3: Tests ausführen**

Run: `cargo test --lib net::discovery`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/net/discovery.rs src/net/mod.rs
git commit -m "feat(#25): Discovery Announce-Paket + Lobby-Dedup"
```

---

## Task 6: Rendering auf `WorldView` umstellen + Roster; `App`-Modi

**Files:**
- Modify: `src/app.rs` (komplette Umstrukturierung auf `Mode`), `src/render/mod.rs` (`draw_world`, neue `draw`-Signatur, Roster)
- Modify: `src/main.rs` (Single-Player-Pfad an neue `App`-API anpassen — nur so weit, dass es baut)
- Test: bestehende Tests bleiben grün; manueller Lauf (`cargo run`).

**Interfaces:**
- Consumes: `WritingEngine`, `WorldView`, `PlayerView`, `PlayerColor`, `Direction`.
- Produces (auf `App`):
  - `enum Mode { Single(WritingEngine), Host(HostState), Client(WorldView) }`
  - `App::new_single() -> App`
  - `App::world_view(&self) -> WorldView`
  - `App::local_engine(&self) -> Option<&WritingEngine>` (Single & Host: Some; Client: None)
  - `App::on_char/on_backspace` arbeiten im Single-Modus weiter wie bisher (Host/Client-Input-Routing kommt in Task 9).

> Diese Task hält Single-Player voll funktionsfähig und entkoppelt das Rendering. Host-/Client-Modi werden hier strukturell vorbereitet, aber erst in Task 8/9 mit Netzwerk befüllt.

- [ ] **Step 1: `App` umstrukturieren**

`src/app.rs` ersetzen durch:
```rust
use crate::game::world::{PlayerId, PlayerView, WorldView};
use crate::game::writing::{Direction, StepResult, WritingEngine};
use crate::net::server::HostState;

pub enum Mode {
    Single(WritingEngine),
    Host(HostState),
    Client(WorldView),
}

pub struct App {
    pub should_quit: bool,
    pub mode: Mode,
    pub day: i64,
    pub last_event: String,
    pub trigger_banner: Option<String>,
    pub trigger_banner_ticks: u32,
    pub debug: bool,
    pub debug_lines: Vec<String>,
}

impl App {
    pub fn new_single() -> Self {
        Self {
            should_quit: false,
            mode: Mode::Single(WritingEngine::new((0, 0))),
            day: 4380,
            last_event: String::from("type to write yourself a path"),
            trigger_banner: None,
            trigger_banner_ticks: 0,
            debug: false,
            debug_lines: Vec::new(),
        }
    }

    pub fn self_id(&self) -> PlayerId {
        match &self.mode {
            Mode::Single(_) => 0,
            Mode::Host(h) => h.self_id(),
            Mode::Client(w) => w.self_id,
        }
    }

    pub fn local_engine(&self) -> Option<&WritingEngine> {
        match &self.mode {
            Mode::Single(e) => Some(e),
            Mode::Host(h) => Some(h.local_engine()),
            Mode::Client(_) => None,
        }
    }

    pub fn world_view(&self) -> WorldView {
        match &self.mode {
            Mode::Single(e) => WorldView {
                self_id: 0,
                players: vec![PlayerView {
                    id: 0,
                    color: crate::game::world::PALETTE[0],
                    name: "you".into(),
                    trail: e.trail.clone(),
                    cursor: e.cursor,
                    direction: e.direction,
                    is_self: true,
                }],
            },
            Mode::Host(h) => h.world_view(),
            Mode::Client(w) => w.clone(),
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
        match &mut self.mode {
            Mode::Single(e) => e.tick_visuals(),
            Mode::Host(h) => h.tick_visuals(),
            Mode::Client(w) => w.tick_visuals(),
        }
    }

    /// Single-player local input. (Host/Client routing added in Task 9.)
    pub fn on_char(&mut self, c: char) {
        if c == ' ' {
            return;
        }
        if let Mode::Single(e) = &mut self.mode {
            let result = e.on_char(c);
            self.last_event = match &result {
                StepResult::Wrote(_) => format!("wrote '{}'", c),
                StepResult::WroteAndTurned(_, d) => format!("turned: {:?}", d),
                StepResult::WroteAndStopped(_) => "paused".into(),
                StepResult::Erased => "erased".into(),
            };
            if let StepResult::WroteAndTurned(_, d) = result {
                self.set_banner(format!("⟹ TURNED: {:?}", d));
            }
            if matches!(result, StepResult::WroteAndStopped(_)) {
                self.set_banner("⟹ STOP — next char overwrites".into());
            }
        }
    }

    pub fn on_backspace(&mut self) {
        if let Mode::Single(e) = &mut self.mode {
            e.on_backspace();
            self.last_event = format!("walked back. doubt: {}", e.doubt);
        }
    }

    pub fn on_enter(&mut self) {}

    fn set_banner(&mut self, msg: String) {
        self.trigger_banner = Some(msg);
        self.trigger_banner_ticks = 90;
    }
}
```

- [ ] **Step 2: `render/mod.rs` auf `WorldView` umstellen**

In `src/render/mod.rs`: `draw` baut die `WorldView` einmal und reicht sie an `draw_world` weiter; `draw_hud` nutzt `app.local_engine()` mit Fallback; neue `draw_roster`-Zeile.

`draw` ändern:
```rust
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(5),
        ])
        .split(f.area());

    let world = app.world_view();
    draw_hud(f, chunks[0], app);
    draw_banner(f, chunks[1], app);
    draw_world(f, chunks[2], &world);
    draw_bottom(f, chunks[3], app, &world);

    if app.debug {
        draw_debug_overlay(f, app);
    }
}
```

`draw_hud` Direction/word: Direction kommt aus der self-`PlayerView`; combo/doubt/word aus `local_engine()` mit Fallback:
```rust
fn draw_hud(f: &mut Frame, area: Rect, app: &App) {
    let world = app.world_view();
    let dir = world
        .players
        .iter()
        .find(|p| p.is_self)
        .map(|p| p.direction)
        .unwrap_or(Direction::Right);
    let arrow = match dir {
        Direction::Up => "↑",
        Direction::Down => "↓",
        Direction::Left => "←",
        Direction::Right => "→",
    };

    let (word_display, word_is_trigger, combo, doubt) = match app.local_engine() {
        Some(e) => (
            if e.current_word.is_empty() { "—".to_string() } else { e.current_word.clone() },
            buffer_ends_with_trigger(&e.current_word),
            e.combo,
            e.doubt,
        ),
        None => ("—".to_string(), false, 0, 0),
    };
    let word_color = if word_is_trigger { Color::LightGreen } else { Color::DarkGray };

    let hud = Paragraph::new(Line::from(vec![
        Span::styled(" PULL REQUEST FROM HELL ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(format!("dir {} ", arrow), Style::default().fg(Color::Yellow)),
        Span::raw("  word: "),
        Span::styled(word_display, Style::default().fg(word_color).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(format!("combo x{}", combo), Style::default().fg(Color::Magenta)),
        Span::raw("  "),
        Span::styled(format!("doubt {}", doubt), Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(format!("day {}", app.day), Style::default().fg(Color::Yellow)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(hud, area);
}
```

`draw_world` Signatur und Body auf `&WorldView` umstellen — alle Spieler zeichnen, Kamera auf self-Cursor, Farbe pro Spieler:
```rust
fn draw_world(f: &mut Frame, area: Rect, world: &WorldView) {
    let block = Block::default().borders(Borders::ALL).title(" /work/repo/career.md ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let w = inner.width as i32;
    let h = inner.height as i32;
    let center = (w / 2, h / 2);

    let self_player = world.players.iter().find(|p| p.is_self);
    let cursor = self_player.map(|p| p.cursor).unwrap_or((0, 0));

    let mut grid: Vec<Vec<Option<(char, Style)>>> = vec![vec![None; w as usize]; h as usize];

    // newest tick across all trails, for fade reference
    let now = world
        .players
        .iter()
        .flat_map(|p| p.trail.iter().map(|t| t.tick))
        .max()
        .unwrap_or(0);

    const FADE_PER_TICK: u64 = 2;
    const MAX_BRIGHTNESS: u64 = 200;
    const MIN_BRIGHTNESS: u64 = 60;

    for player in &world.players {
        for tile in &player.trail {
            let rx = tile.pos.0 - cursor.0 + center.0;
            let ry = tile.pos.1 - cursor.1 + center.1;
            if rx < 0 || ry < 0 || rx >= w || ry >= h {
                continue;
            }
            let style = if tile.glow > 0 {
                Style::default().fg(Color::LightYellow).bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                let age = now.saturating_sub(tile.tick);
                let b = MAX_BRIGHTNESS
                    .saturating_sub(age.saturating_mul(FADE_PER_TICK))
                    .max(MIN_BRIGHTNESS);
                let scale = |c: u8| ((c as u64 * b) / MAX_BRIGHTNESS).min(255) as u8;
                Style::default().fg(Color::Rgb(scale(player.color.r), scale(player.color.g), scale(player.color.b)))
            };
            grid[ry as usize][rx as usize] = Some((tile.ch, style));
        }
    }

    // Cursor markers: self = black-on-yellow; others = arrow in their color.
    for player in &world.players {
        let rx = player.cursor.0 - cursor.0 + center.0;
        let ry = player.cursor.1 - cursor.1 + center.1;
        if rx < 0 || ry < 0 || rx >= w || ry >= h {
            continue;
        }
        let arrow_ch = match player.direction {
            Direction::Up => '▲',
            Direction::Down => '▼',
            Direction::Left => '◀',
            Direction::Right => '▶',
        };
        let style = if player.is_self {
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Rgb(player.color.r, player.color.g, player.color.b))
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        };
        grid[ry as usize][rx as usize] = Some((arrow_ch, style));
    }

    let empty_style = Style::default();
    let lines: Vec<Line> = grid
        .iter()
        .map(|row| {
            let mut spans: Vec<Span> = Vec::with_capacity(row.len());
            for cell in row.iter() {
                match cell {
                    Some((ch, style)) => spans.push(Span::styled(ch.to_string(), *style)),
                    None => spans.push(Span::styled(" ".to_string(), empty_style)),
                }
            }
            Line::from(spans)
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}
```

`draw_bottom` um Roster-Zeile erweitern (Signatur bekommt `&WorldView`):
```rust
fn draw_bottom(f: &mut Frame, area: Rect, app: &App, world: &WorldView) {
    let roster: Vec<Span> = world
        .players
        .iter()
        .flat_map(|p| {
            let label = if p.is_self { format!("{}(du)", p.name) } else { p.name.clone() };
            vec![
                Span::styled(
                    format!("{} ", label),
                    Style::default().fg(Color::Rgb(p.color.r, p.color.g, p.color.b)).add_modifier(Modifier::BOLD),
                ),
            ]
        })
        .collect();

    let inner_lines = vec![
        Line::from(roster),
        Line::from(Span::styled(app.last_event.as_str(), Style::default().fg(Color::DarkGray))),
        Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
            Span::raw(" quit  "),
            Span::raw("triggers: "),
            Span::styled("up down left right back stop", Style::default().fg(Color::Yellow)),
        ]),
    ];

    let p = Paragraph::new(inner_lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
```

Imports in `render/mod.rs` oben ergänzen:
```rust
use crate::game::world::WorldView;
```
(Der bestehende `use crate::game::writing::{buffer_ends_with_trigger, Direction};` bleibt.)

- [ ] **Step 3: `main.rs` Single-Player-Pfad anpassen** (nur damit es baut)

In `src/main.rs` `App::new()` → `App::new_single()` ersetzen:
```rust
let mut app = App::new_single();
```

- [ ] **Step 4: Bauen + bestehende Tests**

Run: `cargo build && cargo test`
Expected: warnungsfrei, alle Tests grün.

- [ ] **Step 5: Manuell verifizieren**

Run: `cargo run`
Expected: Single-Player verhält sich wie zuvor — tippen schreibt eine **grüne** Spur (Palette[0]), Trigger funktionieren, Roster zeigt `you(du)` in Grün.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/render/mod.rs src/main.rs
git commit -m "refactor(#25): Rendering auf WorldView + Roster, App-Modi"
```

---

## Task 7: Host-Netzwerk-Threads + Loopback-Handshake-Test

**Files:**
- Modify: `src/net/server.rs` (Socket-/Thread-Layer unter `HostState`)
- Test: Integrationstest in `tests/host_handshake.rs`

**Interfaces:**
- Consumes: `HostState`, `ClientMsg`, `ServerMsg`, `encode_line`, `decode_line`, `discovery::TCP_PORT`.
- Produces:
  - `enum HostEvent { Hello { conn_id: u64, name: String, write: TcpStream }, Input { conn_id: u64, ev: InputEvent }, Disconnected { conn_id: u64 } }`
  - `fn spawn_listener(listener: TcpListener) -> mpsc::Receiver<HostEvent>` — startet Accept-Thread + pro Verbindung einen Reader-Thread.

- [ ] **Step 1: Socket-Layer implementieren** (in `src/net/server.rs` ergänzen)

```rust
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::net::protocol::{decode_line, ClientMsg, InputEvent};

pub enum HostEvent {
    Hello { conn_id: u64, name: String, write: TcpStream },
    Input { conn_id: u64, ev: InputEvent },
    Disconnected { conn_id: u64 },
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
            if tx.send(HostEvent::Hello { conn_id, name, write }).is_err() {
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
    stream.write_all(crate::net::protocol::encode_line(msg).as_bytes())
}
```

- [ ] **Step 2: Loopback-Handshake-Test schreiben** (`tests/host_handshake.rs`)

```rust
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use prfh::net::protocol::{decode_line, encode_line, ClientMsg, ServerMsg};
use prfh::net::server::{spawn_listener, HostEvent, HostState};

#[test]
fn client_hello_gets_welcome() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let rx = spawn_listener(listener);

    let mut host = HostState::new("Host".into());

    // Client connects and says hello.
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .write_all(encode_line(&ClientMsg::Hello { name: "Bob".into() }).as_bytes())
        .unwrap();

    // Host receives Hello, assigns a player, replies Welcome.
    let event = rx.recv().unwrap();
    match event {
        HostEvent::Hello { name, mut write, .. } => {
            assert_eq!(name, "Bob");
            let outcome = host.add_player(name).unwrap();
            write
                .write_all(encode_line(&outcome.welcome).as_bytes())
                .unwrap();
        }
        _ => panic!("expected Hello"),
    }

    // Client reads the Welcome line.
    let mut reader = BufReader::new(client);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let msg: ServerMsg = decode_line(&line).unwrap();
    match msg {
        ServerMsg::Welcome { your_id, .. } => assert_eq!(your_id, 1),
        _ => panic!("expected Welcome"),
    }
}
```

- [ ] **Step 3: Test ausführen**

Run: `cargo test --test host_handshake`
Expected: PASS. `cargo build` warnungsfrei.

- [ ] **Step 4: Commit**

```bash
git add src/net/server.rs tests/host_handshake.rs
git commit -m "feat(#25): Host-Netzwerk-Threads + Loopback-Handshake-Test"
```

---

## Task 8: Host-Modus in der Render-Schleife (`prfh host`)

**Files:**
- Modify: `src/main.rs` (CLI-Dispatch + Host-Loop), `src/app.rs` (Host-Input-Routing + Broadcast-Hook)
- Test: manuelle Verifikation mit zwei Terminals + bestehende Tests grün.

**Interfaces:**
- Consumes: `spawn_listener`, `HostEvent`, `send_msg`, `HostState`, `discovery` (Announce in Task 9).
- Produces: lauffähiges `prfh host`, das lokalen Input verarbeitet, Clients akzeptiert, Deltas broadcastet.

**Architektur des Host-Loops:** Der Host hält `HostState`, eine `HashMap<u64, TcpStream>` (conn_id → Write-Stream) und eine `HashMap<u64, PlayerId>` (conn_id → Spieler). Pro Frame: (a) Terminal-Input pollen → `host.apply_input(HOST_ID, ev)` → an alle Streams broadcasten; (b) `rx.try_recv()` leeren → `HostEvent` verarbeiten; (c) `render::draw`, `app.tick()`.

- [ ] **Step 1: CLI parsen** — in `src/main.rs` vor dem Terminal-Setup:

```rust
enum Cli {
    Single,
    Host { name: String },
    Join { addr: Option<String>, name: String },
}

fn parse_cli() -> Cli {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name_of = |args: &[String]| -> String {
        args.iter()
            .position(|a| a == "--name")
            .and_then(|i| args.get(i + 1).cloned())
            .unwrap_or_default()
    };
    match args.first().map(|s| s.as_str()) {
        Some("host") => Cli::Host {
            name: { let n = name_of(&args); if n.is_empty() { "Host".into() } else { n } },
        },
        Some("join") => {
            let addr = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .cloned();
            Cli::Join { addr, name: name_of(&args) }
        }
        _ => Cli::Single,
    }
}
```

- [ ] **Step 2: Host-Run-Funktion** — in `src/main.rs`:

```rust
fn run_host<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    name: String,
    debug: bool,
) -> Result<()> {
    use prfh::net::server::{send_msg, spawn_listener, HostEvent, HostState, HOST_ID};
    use prfh::net::protocol::{InputEvent, ServerMsg};
    use prfh::net::discovery::TCP_PORT;
    use std::collections::HashMap;
    use std::net::TcpListener;

    let listener = TcpListener::bind(("0.0.0.0", TCP_PORT))?;
    listener.set_nonblocking(false)?;
    let rx = spawn_listener(listener);
    // Discovery-Announce (Task 9): prfh::net::discovery::spawn_announce(name.clone());

    let mut host = HostState::new(name);
    let mut streams: HashMap<u64, std::net::TcpStream> = HashMap::new();
    let mut conn_player: HashMap<u64, prfh::game::world::PlayerId> = HashMap::new();

    let mut app = App { mode: Mode::Host(host_placeholder()), ..App::new_single() };
    app.debug = debug;

    // We keep `host` as the source of truth and rebuild app.mode's view each frame.
    // Simpler: store HostState in app.mode directly.
    app.mode = Mode::Host(std::mem::replace(&mut host, HostState::new(String::new())));

    while !app.should_quit {
        terminal.draw(|f| render::draw(f, &app))?;

        // (a) local input
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Mode::Host(h) = &mut app.mode {
                        let ev = match key.code {
                            KeyCode::Esc => { app.should_quit = true; None }
                            KeyCode::Char(' ') => None,
                            KeyCode::Char(c) => Some(InputEvent::Char(c)),
                            KeyCode::Backspace => Some(InputEvent::Backspace),
                            _ => None,
                        };
                        if let Some(ev) = ev {
                            if let Some(msg) = h.apply_input(HOST_ID, ev) {
                                broadcast(&mut streams, None, &msg);
                            }
                        }
                    }
                }
            }
        }

        // (b) network events
        while let Ok(ev) = rx.try_recv() {
            if let Mode::Host(h) = &mut app.mode {
                match ev {
                    HostEvent::Hello { conn_id, name, mut write } => {
                        match h.add_player(name) {
                            Ok(outcome) => {
                                let _ = send_msg(&mut write, &outcome.welcome);
                                conn_player.insert(conn_id, outcome.id);
                                streams.insert(conn_id, write);
                                broadcast(&mut streams, Some(conn_id), &outcome.joined);
                            }
                            Err(reason) => {
                                let _ = send_msg(&mut write, &ServerMsg::Reject { reason });
                            }
                        }
                    }
                    HostEvent::Input { conn_id, ev } => {
                        if let Some(&pid) = conn_player.get(&conn_id) {
                            if let Some(msg) = h.apply_input(pid, ev) {
                                broadcast(&mut streams, None, &msg);
                            }
                        }
                    }
                    HostEvent::Disconnected { conn_id } => {
                        if let Some(pid) = conn_player.remove(&conn_id) {
                            streams.remove(&conn_id);
                            if let Some(msg) = h.remove_player(pid) {
                                broadcast(&mut streams, None, &msg);
                            }
                        }
                    }
                }
            }
        }

        app.tick();
    }
    Ok(())
}

fn broadcast(
    streams: &mut std::collections::HashMap<u64, std::net::TcpStream>,
    exclude: Option<u64>,
    msg: &prfh::net::protocol::ServerMsg,
) {
    use prfh::net::server::send_msg;
    let mut dead = Vec::new();
    for (cid, s) in streams.iter_mut() {
        if Some(*cid) == exclude {
            continue;
        }
        if send_msg(s, msg).is_err() {
            dead.push(*cid);
        }
    }
    for cid in dead {
        streams.remove(&cid);
    }
}
```

> **Vereinfachung umsetzen:** Die `host_placeholder()`-Krücke oben **nicht** verwenden. Stattdessen `App` direkt mit `Mode::Host(HostState::new(name))` bauen. Saubere Variante: in `src/app.rs` einen Konstruktor `App::new_with_mode(mode: Mode) -> App` ergänzen und hier `App::new_with_mode(Mode::Host(HostState::new(name)))` aufrufen. Den `std::mem::replace`-Block dann entfernen.

- [ ] **Step 2b: `App::new_with_mode` ergänzen** (in `src/app.rs`):

```rust
impl App {
    pub fn new_with_mode(mode: Mode) -> Self {
        let mut a = App::new_single();
        a.mode = mode;
        a
    }
}
```
Und in `run_host` den Aufbau auf eine Zeile reduzieren:
```rust
let mut app = App::new_with_mode(Mode::Host(HostState::new(name)));
app.debug = debug;
```
(Die lokalen `host`/`host_placeholder`/`mem::replace`-Zeilen entfernen.)

- [ ] **Step 3: `main()` Dispatch verdrahten**

```rust
fn main() -> Result<()> {
    let cli = parse_cli();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let debug = std::env::var("PRFH_DEBUG").is_ok();

    let result = match cli {
        Cli::Single => run(&mut terminal),
        Cli::Host { name } => run_host(&mut terminal, name, debug),
        Cli::Join { addr, name } => run_client(&mut terminal, addr, name, debug), // Task 9
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}
```
> `run_client` wird in Task 9 hinzugefügt. Bis dahin den `Cli::Join`-Arm temporär auf `run(&mut terminal)` zeigen lassen oder Task 8 und 9 zusammen abschließen.

- [ ] **Step 4: Bauen + Tests**

Run: `cargo build && cargo test`
Expected: warnungsfrei, alle Tests grün.

- [ ] **Step 5: Manuell verifizieren (Host allein)**

Run: `cargo run -- host --name Alice`
Expected: Startet wie Single-Player, Roster zeigt `Alice(du)` grün; tippen schreibt grün; `Esc` beendet. (Clients folgen in Task 9.)

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/app.rs
git commit -m "feat(#25): Host-Modus (prfh host) mit Broadcast-Loop"
```

---

## Task 9: Client-Modus + Discovery-Lobby (`prfh join`)

**Files:**
- Modify: `src/net/client.rs` (Threads), `src/net/discovery.rs` (UDP announce/listen), `src/main.rs` (`run_client` + Lobby-Prompt), `src/net/mod.rs` (`pub mod client;` aktiv)
- Test: Loopback-End-to-End-Test `tests/host_client_e2e.rs` + manuelle Zwei-Terminal-Verifikation.

**Interfaces:**
- Consumes: `encode_line`, `decode_line`, `ClientMsg`, `ServerMsg`, `InputEvent`, `WorldView`, `merge_announce`, `Announce`, `TCP_PORT`, `DISCOVERY_PORT`.
- Produces:
  - `fn connect(addr: &str, name: &str) -> anyhow::Result<(WorldView, ClientHandle)>` — verbindet, sendet `Hello`, liest `Welcome` (oder `Reject` → `Err`), startet Reader-Thread, liefert initiale `WorldView` + Handle.
  - `struct ClientHandle { write: TcpStream, rx: Receiver<ServerMsg> }` mit `send_input(&mut self, InputEvent)`.
  - `discovery::spawn_announce(name: String, tcp_port: u16)` — Host-seitiger UDP-Broadcast-Thread.
  - `discovery::discover(timeout: Duration) -> Vec<LobbyEntry>` — sammelt Announces.

- [ ] **Step 1: Client-Threads** — `src/net/client.rs`:

```rust
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use anyhow::{anyhow, Result};

use crate::game::world::WorldView;
use crate::net::protocol::{decode_line, encode_line, ClientMsg, InputEvent, ServerMsg};

pub struct ClientHandle {
    write: TcpStream,
    pub rx: Receiver<ServerMsg>,
}

impl ClientHandle {
    pub fn send_input(&mut self, ev: InputEvent) {
        let _ = self
            .write
            .write_all(encode_line(&ClientMsg::Input(ev)).as_bytes());
    }
}

/// Connect, perform the Hello/Welcome handshake, and start the reader thread.
pub fn connect(addr: &str, name: &str) -> Result<(WorldView, ClientHandle)> {
    let stream = TcpStream::connect(addr)?;
    let mut write = stream.try_clone()?;
    write.write_all(encode_line(&ClientMsg::Hello { name: name.to_string() }).as_bytes())?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Err(anyhow!("Verbindung vom Host geschlossen"));
    }
    let mut world = match decode_line::<ServerMsg>(&line)? {
        ServerMsg::Welcome { your_id, color, players } => {
            let mut w = WorldView::new(your_id);
            w.apply(ServerMsg::Welcome { your_id, color, players });
            w
        }
        ServerMsg::Reject { reason } => return Err(anyhow!(reason)),
        _ => return Err(anyhow!("unerwartete erste Nachricht vom Host")),
    };
    let _ = &mut world;

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => match decode_line::<ServerMsg>(&line) {
                    Ok(msg) => {
                        if tx.send(msg).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                },
                Err(_) => break,
            }
        }
    });

    Ok((world, ClientHandle { write, rx }))
}
```
`pub mod client;` in `src/net/mod.rs` aktivieren.

- [ ] **Step 2: Discovery-Sockets** — in `src/net/discovery.rs` ergänzen:

```rust
use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};

/// Host: periodically broadcast an announce packet on the LAN.
pub fn spawn_announce(name: String, tcp_port: u16) {
    thread::spawn(move || {
        let socket = match UdpSocket::bind(("0.0.0.0", 0)) {
            Ok(s) => s,
            Err(_) => return,
        };
        if socket.set_broadcast(true).is_err() {
            return;
        }
        let payload = ron::to_string(&Announce { name, tcp_port }).unwrap();
        let target = ("255.255.255.255", DISCOVERY_PORT);
        loop {
            let _ = socket.send_to(payload.as_bytes(), target);
            thread::sleep(Duration::from_millis(1000));
        }
    });
}

/// Client: listen for announce packets for `timeout`, return deduped lobby.
pub fn discover(timeout: Duration) -> Vec<LobbyEntry> {
    let mut entries = Vec::new();
    let socket = match UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)) {
        Ok(s) => s,
        Err(_) => return entries,
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(250)));
    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 512];
    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, src)) => {
                if let Ok(a) = ron::from_str::<Announce>(&String::from_utf8_lossy(&buf[..n])) {
                    merge_announce(&mut entries, src.ip(), a);
                }
            }
            Err(_) => continue,
        }
    }
    entries
}
```

- [ ] **Step 3: `run_client` + Lobby-Prompt** — in `src/main.rs`:

```rust
fn run_client<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    addr: Option<String>,
    name: String,
    debug: bool,
) -> Result<()> {
    use prfh::net::client::connect;
    use prfh::net::discovery::{discover, TCP_PORT};
    use prfh::net::protocol::InputEvent;
    use std::time::Duration as StdDuration;

    let target = match addr {
        Some(a) if a.contains(':') => a,
        Some(a) => format!("{}:{}", a, TCP_PORT),
        None => {
            // Discovery lobby on plain terminal (before/outside the loop).
            let found = discover(StdDuration::from_secs(2));
            if found.is_empty() {
                anyhow::bail!("Keine Spiele im LAN gefunden. Nutze `prfh join <ip>`.");
            }
            // Pick the first found game (MVP). Multiple → first wins; log others.
            let e = &found[0];
            format!("{}:{}", e.addr, e.tcp_port)
        }
    };

    let name = if name.is_empty() { "Player".into() } else { name };
    let (world, mut handle) = connect(&target, &name)?;
    let mut app = App::new_with_mode(Mode::Client(world));
    app.debug = debug;

    while !app.should_quit {
        terminal.draw(|f| render::draw(f, &app))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc => app.should_quit = true,
                        KeyCode::Char(' ') => {}
                        KeyCode::Char(c) => handle.send_input(InputEvent::Char(c)),
                        KeyCode::Backspace => handle.send_input(InputEvent::Backspace),
                        _ => {}
                    }
                }
            }
        }

        // Drain server messages.
        let mut host_gone = false;
        loop {
            match handle.rx.try_recv() {
                Ok(msg) => {
                    if let Mode::Client(w) = &mut app.mode {
                        w.apply(msg);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    host_gone = true;
                    break;
                }
            }
        }
        if host_gone {
            app.last_event = "Host getrennt — beende.".into();
            app.should_quit = true;
        }

        app.tick();
    }
    Ok(())
}
```

> **Lobby-Mehrfachauswahl (optional, YAGNI fürs MVP):** Bei mehreren gefundenen Spielen wird das erste genommen. Falls Auswahl gewünscht: VOR `enable_raw_mode()` die Liste mit `println!` ausgeben und eine Zeile von stdin lesen. Für das MVP genügt „erstes Spiel".

- [ ] **Step 4: Discovery-Announce im Host aktivieren** — in `run_host` (Task 8) die auskommentierte Zeile aktivieren:
```rust
prfh::net::discovery::spawn_announce(/* name */ app_host_name.clone(), prfh::net::discovery::TCP_PORT);
```
> Den Host-Namen vor dem `mem`-Aufbau in einer Variablen sichern (`let announce_name = name.clone();`) und `spawn_announce(announce_name, TCP_PORT)` direkt nach dem Binden des Listeners aufrufen.

- [ ] **Step 5: End-to-End-Loopback-Test** — `tests/host_client_e2e.rs`:

```rust
use std::collections::HashMap;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use prfh::game::world::WorldView;
use prfh::net::client::connect;
use prfh::net::protocol::{InputEvent, ServerMsg};
use prfh::net::server::{send_msg, spawn_listener, HostEvent, HostState, HOST_ID};

// Drive the host event loop manually for a short while in a background thread.
#[test]
fn client_sees_host_keystroke() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let rx = spawn_listener(listener);

    // Host thread: processes events + host's own input.
    let host_handle = std::thread::spawn(move || {
        let mut host = HostState::new("Host".into());
        let mut streams: HashMap<u64, std::net::TcpStream> = HashMap::new();
        let mut conn_player: HashMap<u64, u8> = HashMap::new();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut typed = false;
        while Instant::now() < deadline {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    HostEvent::Hello { conn_id, name, mut write } => {
                        let outcome = host.add_player(name).unwrap();
                        send_msg(&mut write, &outcome.welcome).unwrap();
                        conn_player.insert(conn_id, outcome.id);
                        streams.insert(conn_id, write);
                        // After a client joins, host types 'h' once.
                        if !typed {
                            if let Some(msg) = host.apply_input(HOST_ID, InputEvent::Char('h')) {
                                for s in streams.values_mut() {
                                    send_msg(s, &msg).unwrap();
                                }
                            }
                            typed = true;
                        }
                    }
                    HostEvent::Input { conn_id, ev } => {
                        if let Some(&pid) = conn_player.get(&conn_id) {
                            if let Some(msg) = host.apply_input(pid, ev) {
                                for s in streams.values_mut() {
                                    let _ = send_msg(s, &msg);
                                }
                            }
                        }
                    }
                    HostEvent::Disconnected { .. } => {}
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    let (world, handle): (WorldView, _) = connect(&addr.to_string(), "Bob").unwrap();
    assert!(world.players.iter().any(|p| p.is_self)); // got Welcome

    // Wait for the host's 'h' keystroke to arrive.
    let msg = handle.rx.recv_timeout(Duration::from_secs(3)).unwrap();
    match msg {
        ServerMsg::Wrote { id, tile, .. } => {
            assert_eq!(id, HOST_ID);
            assert_eq!(tile.ch, 'h');
        }
        other => panic!("expected Wrote, got {:?}", other),
    }

    drop(handle);
    let _ = host_handle.join();
}
```

- [ ] **Step 6: Bauen + alle Tests**

Run: `cargo build && cargo test`
Expected: warnungsfrei, alle Tests grün (inkl. `host_handshake`, `host_client_e2e`).

- [ ] **Step 7: Manuell verifizieren (zwei Terminals)**

Terminal A: `cargo run -- host --name Alice`
Terminal B: `cargo run -- join 127.0.0.1 --name Bob`
Expected: B sieht Alices grüne Spur erscheinen, während A tippt; A sieht Bobs cyanfarbene Spur. Beide Roster zeigen beide Namen. `Esc` auf B → B beendet; `Esc` auf A → A beendet, B meldet „Host getrennt".

- [ ] **Step 8: Commit**

```bash
git add src/net/client.rs src/net/discovery.rs src/net/mod.rs src/main.rs tests/host_client_e2e.rs
git commit -m "feat(#25): Client-Modus + UDP-Discovery-Lobby"
```

---

## Self-Review-Ergebnis

**Spec-Abdeckung:**
- Host-Client/TCP/autoritativ → Tasks 4, 7, 8. ✅
- Keine Prädiktion → Client rendert nur empfangenen Zustand (Task 9). ✅
- UDP-Discovery + manuelle IP → Task 9 (`discover`/`spawn_announce`, `join <ip>`-Fallback). ✅
- Ko-Präsenz, gemeinsamer Raum, Kamera auf eigenen Cursor → Task 6 (`draw_world`). ✅
- Eigenfarbige Spuren, Host vergibt Palette → Tasks 2 (PALETTE), 4 (`add_player`/`free_color_idx`), 6 (Färbung). ✅
- Max 6, Reject beim 7. → Task 4 (`full_game_rejects_seventh_player`) + Task 8 (`Reject` senden) + Task 9 (Client `Err`). ✅
- WorldView entkoppelt Rendering → Tasks 2/6. ✅
- Trail-Cap → Task 2 (`push_tile`/`TRAIL_CAP`). ✅
- Disconnect-Handling → Task 8 (`Disconnected` → `remove_player` → `PlayerLeft`), Task 9 (Host weg → Exit). ✅
- Newline-RON-Framing → Task 3 (`encode_line`/`decode_line`). ✅
- Roster im HUD → Task 6 (`draw_bottom`). ✅
- Keine neuen Deps → durchgängig nur `std` + `serde`/`ron`. ✅
- Tests: Protokoll-Roundtrip (T3), WorldView-Apply (T3), HostState (T4), Discovery (T5), Loopback-Handshake (T7), E2E (T9). ✅

**Platzhalter-Scan:** Keine TBD/TODO in Code-Schritten. Die als „optional/YAGNI" markierten Punkte (Lobby-Mehrfachauswahl) sind bewusst außerhalb des MVP und nicht erforderlich für grüne Tasks.

**Typ-Konsistenz:** `WorldView`, `PlayerView`, `HostState`, `ServerMsg`/`ClientMsg`, `InputEvent`, `encode_line`/`decode_line`, `spawn_listener`/`HostEvent`/`send_msg`, `connect`/`ClientHandle` über alle Tasks einheitlich benannt und verwendet.
