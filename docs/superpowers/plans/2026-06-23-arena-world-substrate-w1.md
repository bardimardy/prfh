# Arena-Welt-Substrat (W1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ein geteiltes, host-autoritatives Welt-Substrat (`Arena` aus platzierten Entitäten), strikt getrennt vom Render-`WorldView`, das über das bestehende Multiplayer-Modell synct und in allen drei Modi (Single/Host/Client) gehalten und gerendert wird.

**Architecture:** Neue Sim-Welt `src/game/arena.rs` (`Arena { entities, next_id }`). Host besitzt die autoritative Arena und broadcastet `EntitySpawned`/`EntityDespawned`-Deltas; der `Welcome`-Snapshot trägt die volle Arena fürs Late-Join. Client pflegt eine Kopie aus Snapshot + Deltas; Single hält die Arena direkt. `draw_world` zeichnet Entitäten mit derselben Cursor-zentrierten Transform **vor** den Trails. Sim (`Arena`) bleibt strikt getrennt vom Render (`WorldView`) — kein Vermischen, `world.rs::apply` bleibt intakt.

**Tech Stack:** Rust 2021, Ratatui + Crossterm (TUI), serde + ron (Sync-Serialisierung — bereits vorhanden, **keine neuen Dependencies**), Standard-`std::net`-TCP-Loopback für Integrationstests.

## Global Constraints

- **Keine neuen Dependencies** — nur `serde` + `ron` (vorhanden) für Snapshot/Delta.
- `cargo build` **und** `cargo test` müssen nach **jeder** Task grün **und warnungsfrei** sein. Kein `#[allow]` zum Verstecken; toten Code entfernen.
- `cargo` ist nicht im PATH — Tasks rufen es über `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"` auf.
- **`src/game/world.rs` NICHT umbenennen** — es ist das Render-Modell (`WorldView`/`PlayerView`). Die Sim-Welt lebt ausschließlich in der neuen `src/game/arena.rs`.
- **`EntityKind` bekommt KEIN `bounds`/`terrain`-Feld und keinen `Bounds`-Typ** — ungenutzt = toter Code = Warnung. Erweiterbarkeit kommt aus der Struktur, nicht aus Platzhaltern.
- **Single-Player NICHT mit Host vereinheitlichen** — die drei Modi bleiben getrennt (MP-Pfad ist fragil), referenzieren aber dieselbe `Arena`-Struct.
- `EntityId = u32`, monoton vom Host vergeben. `ArenaSnapshot = Vec<Entity>`; `next_id` wird **nicht** übertragen (Clients übernehmen IDs aus Deltas).
- Rust-Stil: `cargo fmt`-konform, Naming/Kommentardichte wie der umgebende Code (deutsche Kommentare wo idiomatisch im Repo).

---

### Task 1: Arena-Sim-Modul (`src/game/arena.rs`)

**Files:**
- Create: `src/game/arena.rs`
- Modify: `src/game/mod.rs` (Modul registrieren)

**Interfaces:**
- Consumes: nichts (self-contained; nur `serde`).
- Produces:
  - `pub type EntityId = u32;`
  - `pub type ArenaSnapshot = Vec<Entity>;`
  - `pub struct Entity { pub id: EntityId, pub pos: (i32, i32), pub kind: EntityKind }` — `derive(Debug, Clone, PartialEq, Serialize, Deserialize)`
  - `pub enum EntityKind { PowerupWord(PowerupWord) }` — `derive(Debug, Clone, PartialEq, Serialize, Deserialize)`
  - `pub struct PowerupWord { pub word: String }` — opaker Payload für W2; `derive(Debug, Clone, PartialEq, Serialize, Deserialize)`
  - `pub struct Arena { pub entities: Vec<Entity>, next_id: EntityId }` — `derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)`
  - `Arena::new() -> Arena`
  - `Arena::spawn(&mut self, pos: (i32,i32), kind: EntityKind) -> EntityId`
  - `Arena::despawn(&mut self, id: EntityId)`
  - `Arena::entity_at(&self, pos: (i32,i32)) -> Option<&Entity>`
  - `Arena::snapshot(&self) -> ArenaSnapshot`
  - `Arena::from_snapshot(entities: ArenaSnapshot) -> Arena`
  - `Arena::apply_spawned(&mut self, entity: Entity)` — idempotent gegen Duplikat-IDs
  - `Arena::apply_despawned(&mut self, id: EntityId)`

- [ ] **Step 1: Modul registrieren**

In `src/game/mod.rs` die bestehende Liste um `arena` ergänzen (alphabetisch vor `world`):

```rust
pub mod arena;
pub mod world;
pub mod writing;
```

- [ ] **Step 2: Datenmodell + Mutatoren schreiben**

Erzeuge `src/game/arena.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Monotone, host-vergebene Entitäts-ID. Nur der Host alloziert; Clients
/// übernehmen IDs aus den Deltas/dem Snapshot.
pub type EntityId = u32;

/// Voll-Zustand der Arena fürs Late-Join. Trägt bewusst **kein** `next_id`
/// (Clients vergeben nie selbst IDs).
pub type ArenaSnapshot = Vec<Entity>;

/// Eine platzierte Entität im geteilten Koordinatenraum (gleicher Raum wie
/// Trails/Cursor in `world.rs`/`writing.rs`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub pos: (i32, i32),
    pub kind: EntityKind,
}

/// Art der Entität. Additiv erweiterbar (Item, Obstacle, …) — Sync/Render
/// tragen neue Varianten automatisch mit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntityKind {
    PowerupWord(PowerupWord),
}

/// Opaker Powerup-Payload. Im Substrat (W1) nur ein zu tippendes Wort; das
/// Layout (Origin/Achse/Reversed, Keystroke→Tile-Mapping) kommt additiv in
/// W2 (`powerup.rs`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerupWord {
    pub word: String,
}

/// Die Sim-Welt: eine sparse Sammlung platzierter Entitäten + monotone
/// ID-Vergabe. **Kein** `bounds`/`terrain` — diese kommen später additiv,
/// *wenn* sie einen Konsumenten haben. Strikt getrennt vom Render-`WorldView`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Arena {
    pub entities: Vec<Entity>,
    next_id: EntityId,
}

impl Arena {
    pub fn new() -> Self {
        Self::default()
    }

    /// Vergibt eine monotone ID und fügt die Entität ein. Host-Pfad.
    pub fn spawn(&mut self, pos: (i32, i32), kind: EntityKind) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        self.entities.push(Entity { id, pos, kind });
        id
    }

    /// Entfernt die Entität mit dieser ID (No-Op, wenn nicht vorhanden).
    pub fn despawn(&mut self, id: EntityId) {
        self.entities.retain(|e| e.id != id);
    }

    /// Lookup für Pickup/Kollision: erste Entität an dieser Position.
    pub fn entity_at(&self, pos: (i32, i32)) -> Option<&Entity> {
        self.entities.iter().find(|e| e.pos == pos)
    }

    /// Voll-Zustand fürs Late-Join (Welcome-Snapshot).
    pub fn snapshot(&self) -> ArenaSnapshot {
        self.entities.clone()
    }

    /// Baut eine Arena-Kopie aus einem Snapshot. `next_id` bleibt 0 — Clients
    /// vergeben nie selbst IDs, sie übernehmen sie aus Deltas/Snapshot.
    pub fn from_snapshot(entities: ArenaSnapshot) -> Self {
        Self {
            entities,
            next_id: 0,
        }
    }

    /// Client-seitiges Anwenden eines `EntitySpawned`-Deltas. Idempotent:
    /// ein doppeltes Delta derselben ID erzeugt keine Dublette.
    pub fn apply_spawned(&mut self, entity: Entity) {
        if !self.entities.iter().any(|e| e.id == entity.id) {
            self.entities.push(entity);
        }
    }

    /// Client-seitiges Anwenden eines `EntityDespawned`-Deltas.
    pub fn apply_despawned(&mut self, id: EntityId) {
        self.entities.retain(|e| e.id != id);
    }
}
```

- [ ] **Step 3: Unit-Tests schreiben** (ans Ende von `src/game/arena.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn powerup(word: &str) -> EntityKind {
        EntityKind::PowerupWord(PowerupWord { word: word.into() })
    }

    #[test]
    fn spawn_assigns_monotonic_ids() {
        let mut a = Arena::new();
        let id0 = a.spawn((1, 1), powerup("sudo"));
        let id1 = a.spawn((2, 2), powerup("merge"));
        let id2 = a.spawn((3, 3), powerup("rebase"));
        assert_eq!((id0, id1, id2), (0, 1, 2));
        assert_eq!(a.entities.len(), 3);
    }

    #[test]
    fn ids_stay_monotonic_after_despawn() {
        let mut a = Arena::new();
        let id0 = a.spawn((0, 0), powerup("a"));
        a.despawn(id0);
        // Nach Entfernen wird die ID NICHT wiederverwendet.
        let id1 = a.spawn((0, 0), powerup("b"));
        assert_eq!(id1, 1);
    }

    #[test]
    fn despawn_removes_only_the_target() {
        let mut a = Arena::new();
        let keep = a.spawn((1, 0), powerup("keep"));
        let drop = a.spawn((2, 0), powerup("drop"));
        a.despawn(drop);
        assert_eq!(a.entities.len(), 1);
        assert_eq!(a.entities[0].id, keep);
    }

    #[test]
    fn entity_at_finds_and_misses() {
        let mut a = Arena::new();
        a.spawn((5, 7), powerup("hit"));
        assert!(a.entity_at((5, 7)).is_some());
        assert!(a.entity_at((0, 0)).is_none());
    }

    #[test]
    fn snapshot_roundtrip_preserves_entities() {
        let mut a = Arena::new();
        a.spawn((1, 2), powerup("one"));
        a.spawn((3, 4), powerup("two"));
        let rebuilt = Arena::from_snapshot(a.snapshot());
        assert_eq!(rebuilt.entities, a.entities);
    }

    #[test]
    fn apply_spawned_is_idempotent_on_duplicate_id() {
        let mut a = Arena::new();
        let e = Entity {
            id: 42,
            pos: (1, 1),
            kind: powerup("dup"),
        };
        a.apply_spawned(e.clone());
        a.apply_spawned(e); // doppeltes Delta
        assert_eq!(a.entities.len(), 1, "Duplikat-ID darf keine Dublette erzeugen");
    }

    #[test]
    fn apply_despawned_removes_entity() {
        let mut a = Arena::new();
        a.apply_spawned(Entity {
            id: 7,
            pos: (0, 0),
            kind: powerup("x"),
        });
        a.apply_despawned(7);
        assert!(a.entities.is_empty());
    }
}
```

- [ ] **Step 4: Build + Test**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test arena 2>&1 | tail -20
cargo build 2>&1 | tail -5
```
Expected: alle `arena::tests::*` PASS, Build warnungsfrei.

- [ ] **Step 5: Commit**

```bash
git add src/game/arena.rs src/game/mod.rs
git commit -m "feat(#42): Arena-Sim-Modul — Entity/EntityKind + Mutatoren + Tests"
```

---

### Task 2: Entity-Delta-Varianten im Protokoll (`protocol.rs` + `world.rs`)

**Files:**
- Modify: `src/net/protocol.rs` (neue `ServerMsg`-Varianten + Roundtrip-Tests)
- Modify: `src/game/world.rs` (No-Op-Arm in `WorldView::apply`)

**Interfaces:**
- Consumes: `crate::game::arena::{Entity, EntityId}` (Task 1).
- Produces: `ServerMsg::EntitySpawned { entity: Entity }`, `ServerMsg::EntityDespawned { id: EntityId }`.

> **Hinweis:** `Welcome` bleibt in dieser Task **unverändert** — das `arena`-Feld kommt erst in Task 5, sobald der Host eine Arena besitzt. So bleibt jeder Schritt kompilierbar.

- [ ] **Step 1: Roundtrip-Tests schreiben (failing)** — in `src/net/protocol.rs` im `mod tests` ergänzen:

```rust
    #[test]
    fn server_msg_entity_spawned_roundtrip() {
        use crate::game::arena::{Entity, EntityKind, PowerupWord};
        let msg = ServerMsg::EntitySpawned {
            entity: Entity {
                id: 3,
                pos: (12, -4),
                kind: EntityKind::PowerupWord(PowerupWord {
                    word: "rebase".into(),
                }),
            },
        };
        let back: ServerMsg = decode_line(&encode_line(&msg)).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn server_msg_entity_despawned_roundtrip() {
        let msg = ServerMsg::EntityDespawned { id: 9 };
        let back: ServerMsg = decode_line(&encode_line(&msg)).unwrap();
        assert_eq!(msg, back);
    }
```

- [ ] **Step 2: Test ausführen (verify fail)**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test --lib protocol 2>&1 | tail -20
```
Expected: Kompilierfehler — `ServerMsg` hat keine Variante `EntitySpawned`/`EntityDespawned`.

- [ ] **Step 3: Varianten hinzufügen** — in `src/net/protocol.rs`:

Import oben ergänzen (nach den bestehenden `use crate::game::...`-Zeilen):

```rust
use crate::game::arena::{Entity, EntityId};
```

In `enum ServerMsg` (nach `Respawned { … }`, vor der schließenden `}`) ergänzen:

```rust
    EntitySpawned {
        entity: Entity,
    },
    EntityDespawned {
        id: EntityId,
    },
```

- [ ] **Step 4: No-Op-Arm in `WorldView::apply`** — in `src/game/world.rs`, in der `match msg`-Anweisung in `apply` (nach dem `ServerMsg::Respawned`-Arm, vor der schließenden `}` des `match`) ergänzen:

```rust
            // Entity-Deltas betreffen die Sim-Arena, nicht den Render-WorldView.
            // Der Client-Loop routet sie an die Arena-Kopie (s. main.rs::run_client).
            ServerMsg::EntitySpawned { .. } | ServerMsg::EntityDespawned { .. } => {}
```

- [ ] **Step 5: Build + Test**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test --lib 2>&1 | tail -20
cargo build 2>&1 | tail -5
```
Expected: neue Roundtrip-Tests PASS, alle bestehenden Tests grün, warnungsfrei.

- [ ] **Step 6: Commit**

```bash
git add src/net/protocol.rs src/game/world.rs
git commit -m "feat(#42): ServerMsg::EntitySpawned/EntityDespawned + Roundtrip-Tests"
```

---

### Task 3: Host besitzt die autoritative Arena (`server.rs`)

**Files:**
- Modify: `src/net/server.rs` (`HostState`-Feld `arena` + Accessor + `spawn_entity`)

**Interfaces:**
- Consumes: `crate::game::arena::{Arena, EntityId, EntityKind}` (Task 1), `ServerMsg::EntitySpawned` (Task 2).
- Produces:
  - `HostState::arena(&self) -> &Arena`
  - `HostState::spawn_entity(&mut self, pos: (i32,i32), kind: EntityKind) -> ServerMsg` — mutiert die Arena und liefert das zu broadcastende `EntitySpawned`-Delta.

> `Welcome` wird hier **noch nicht** geändert (Task 5). `spawn_entity` ist der Skeleton-Hook, den W3 (Welt-Aufbau) und der Integrationstest nutzen.

- [ ] **Step 1: Import + Feld ergänzen** — in `src/net/server.rs`:

Import oben ergänzen (nach den bestehenden `use crate::game::...`):

```rust
use crate::game::arena::{Arena, EntityId, EntityKind};
```

Im `struct HostState` ein Feld ergänzen:

```rust
pub struct HostState {
    players: BTreeMap<PlayerId, Player>,
    join_seq: u32,
    /// countdown ticks until respawn; absent = alive
    dead_ticks: HashMap<PlayerId, u32>,
    /// Die autoritative Sim-Welt. Host mutiert sie und broadcastet Deltas;
    /// der Welcome-Snapshot trägt sie fürs Late-Join.
    arena: Arena,
}
```

In `HostState::new` das Feld initialisieren (im Struct-Literal `let mut s = Self { … }`):

```rust
        let mut s = Self {
            players: BTreeMap::new(),
            join_seq: 0,
            dead_ticks: HashMap::new(),
            arena: Arena::new(),
        };
```

- [ ] **Step 2: Accessor + `spawn_entity`** — in `impl HostState` (z. B. nach `local_engine`) ergänzen:

```rust
    /// Read-only-Zugriff auf die autoritative Arena (Rendering, Welcome-Snapshot).
    pub fn arena(&self) -> &Arena {
        &self.arena
    }

    /// Spawnt eine Entität in die autoritative Arena und liefert das
    /// `EntitySpawned`-Delta, das der Aufrufer an alle Clients broadcastet.
    pub fn spawn_entity(&mut self, pos: (i32, i32), kind: EntityKind) -> ServerMsg {
        let id = self.arena.spawn(pos, kind.clone());
        ServerMsg::EntitySpawned {
            entity: crate::game::arena::Entity { id, pos, kind },
        }
    }
```

> `EntityId` wird im Import gelistet, weil Task 5/W2 ihn nutzen; falls `cargo build` in **dieser** Task eine unused-import-Warnung für `EntityId` wirft, entferne `EntityId` aus dem `use` und nimm ihn erst in Task 5 wieder auf. (Erst messen, dann anpassen — siehe Step 3.)

- [ ] **Step 3: Build + Test (Warnungen prüfen!)**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo build 2>&1 | tail -15
cargo test --lib server 2>&1 | tail -15
```
Expected: Build **warnungsfrei**. Falls `unused import: EntityId` erscheint → im `use crate::game::arena::{…}` `EntityId` streichen, neu bauen. (`Arena` + `EntityKind` werden von `arena()`/`spawn_entity` genutzt, `ServerMsg`/`Entity` ohnehin.)

- [ ] **Step 4: Unit-Test für `spawn_entity`** — in `src/net/server.rs` `mod tests` ergänzen:

```rust
    #[test]
    fn spawn_entity_adds_to_arena_and_returns_delta() {
        use crate::game::arena::{EntityKind, PowerupWord};
        let mut s = HostState::new("Host".into());
        assert!(s.arena().entities.is_empty());
        let msg = s.spawn_entity(
            (4, 2),
            EntityKind::PowerupWord(PowerupWord { word: "sudo".into() }),
        );
        assert_eq!(s.arena().entities.len(), 1);
        match msg {
            ServerMsg::EntitySpawned { entity } => {
                assert_eq!(entity.id, 0);
                assert_eq!(entity.pos, (4, 2));
            }
            _ => panic!("expected EntitySpawned"),
        }
    }
```

- [ ] **Step 5: Build + Test**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test --lib server 2>&1 | tail -15
cargo build 2>&1 | tail -5
```
Expected: Test PASS, warnungsfrei.

- [ ] **Step 6: Commit**

```bash
git add src/net/server.rs
git commit -m "feat(#42): HostState besitzt autoritative Arena + spawn_entity-Hook"
```

---

### Task 4: App hält die Arena + Rendering der Entitäten (`app.rs` + `render/mod.rs`)

**Files:**
- Modify: `src/app.rs` (`Mode`-Varianten tragen `Arena`, Accessoren, alle Mode-Matches + Konstruktoren)
- Modify: `src/render/mod.rs` (`draw_world` zeichnet Entitäten)
- Modify: `src/main.rs` (`run_client` konstruiert `Mode::Client(world, Arena::new())` — Platzhalter, in Task 5 durch den echten Snapshot ersetzt)

**Interfaces:**
- Consumes: `crate::game::arena::{Arena, EntityKind}` (Task 1), `HostState::arena()` (Task 3).
- Produces:
  - `Mode::Single(WritingEngine, Arena)`, `Mode::Client(WorldView, Arena)`, `Mode::Host(HostState)` (unverändert).
  - `App::arena(&self) -> &Arena`
  - `App::arena_mut(&mut self) -> Option<&mut Arena>` (Some für Single/Client, None für Host — Skeleton für W2/W3, der die Single-Arena befüllt)

- [ ] **Step 1: `Mode` + Konstruktoren + Matches in `app.rs` anpassen**

Import oben in `src/app.rs` ergänzen:

```rust
use crate::game::arena::Arena;
```

`Mode` ändern:

```rust
pub enum Mode {
    Single(WritingEngine, Arena),
    Host(HostState),
    Client(WorldView, Arena),
}
```

`new_single` anpassen:

```rust
    pub fn new_single() -> Self {
        Self {
            should_quit: false,
            mode: Mode::Single(WritingEngine::new((0, 0)), Arena::new()),
            last_event: String::from("type to write yourself a path"),
            notifications: NotificationStack::new(),
            debug: false,
            debug_lines: Vec::new(),
        }
    }
```

`self_id` — `Mode::Single(_)` → `Mode::Single(..)`, `Mode::Client(w)` → `Mode::Client(w, _)`:

```rust
    pub fn self_id(&self) -> PlayerId {
        match &self.mode {
            Mode::Single(..) => 0,
            Mode::Host(h) => h.self_id(),
            Mode::Client(w, _) => w.self_id,
        }
    }
```

`local_engine`:

```rust
    pub fn local_engine(&self) -> Option<&WritingEngine> {
        match &self.mode {
            Mode::Single(e, _) => Some(e),
            Mode::Host(h) => Some(h.local_engine()),
            Mode::Client(..) => None,
        }
    }
```

`world_view` — der `Mode::Single`-Arm:

```rust
            Mode::Single(e, _) => WorldView {
                self_id: 0,
                players: vec![PlayerView {
                    id: 0,
                    color: crate::game::world::PALETTE[0],
                    name: "you".into(),
                    trail: e.trail.clone(),
                    cursor: e.cursor,
                    direction: e.direction,
                    is_self: true,
                    is_dead: false,
                    pace: e.pace,
                }],
            },
            Mode::Host(h) => h.world_view(),
            Mode::Client(w, _) => w.clone(),
```

`tick`:

```rust
        match &mut self.mode {
            Mode::Single(e, _) => e.tick_visuals(),
            Mode::Host(_) => {}
            Mode::Client(w, _) => w.tick_visuals(),
        }
```

`on_char` — die Bedingung:

```rust
        if let Mode::Single(e, _) = &mut self.mode {
```

`on_backspace` — die Bedingung:

```rust
        if let Mode::Single(e, _) = &mut self.mode {
```

- [ ] **Step 2: Arena-Accessoren in `impl App`** (z. B. nach `world_view`) ergänzen:

```rust
    /// Aktuelle Sim-Arena fürs Rendering (analog zu `world_view`).
    pub fn arena(&self) -> &Arena {
        match &self.mode {
            Mode::Single(_, a) => a,
            Mode::Host(h) => h.arena(),
            Mode::Client(_, a) => a,
        }
    }

    /// Mutabler Zugriff auf die lokal gehaltene Arena (Single/Client). Host
    /// mutiert seine Arena über `HostState`. Skeleton-Hook: W2 befüllt die
    /// Single-Arena, W3 verdrahtet Pickup/Despawn.
    pub fn arena_mut(&mut self) -> Option<&mut Arena> {
        match &mut self.mode {
            Mode::Single(_, a) | Mode::Client(_, a) => Some(a),
            Mode::Host(_) => None,
        }
    }
```

- [ ] **Step 3: `main.rs::run_client` Platzhalter anpassen**

In `src/main.rs`, in `run_client`, die `Mode::Client`-Konstruktion (aktuell `App::new_with_mode(Mode::Client(world))`) ändern zu:

```rust
    let (world, mut handle) = connect(&addr, &name)?;
    // Arena-Snapshot wird in Task 5 aus dem Welcome übernommen; vorerst leer.
    let mut app = App::new_with_mode(Mode::Client(world, prfh::game::arena::Arena::new()));
```

- [ ] **Step 4: `draw_world` zeichnet Entitäten** — in `src/render/mod.rs`:

Import oben ergänzen:

```rust
use crate::game::arena::{Arena, EntityKind};
```

In `draw` die `draw_world`-Zeile um die Arena erweitern (die immutable Borrow endet vor `app.notifications.render`):

```rust
    let world = app.world_view();

    draw_world(f, area, &world, app.arena());
    draw_hud(f, area, app, &world);
```

Signatur von `draw_world` erweitern und die Entitäts-Passage **vor** der Tiles-Schleife einfügen. Die Funktion beginnt heute mit der grid-Initialisierung; direkt **nach** `let mut grid: …` und **vor** dem `all_tiles`-Block einfügen:

```rust
fn draw_world(f: &mut Frame, area: Rect, world: &WorldView, arena: &Arena) {
    let w = area.width as i32;
    let h = area.height as i32;
    let center = (w / 2, h / 2);

    let self_player = world.players.iter().find(|p| p.is_self);
    let cursor = self_player.map(|p| p.cursor).unwrap_or((0, 0));

    let mut grid: Vec<Vec<Option<(char, Style)>>> = vec![vec![None; w as usize]; h as usize];

    // Entitäten zuerst zeichnen (Trails liegen optisch darüber). Dieselbe
    // cursor-zentrierte Transform wie die Tiles. Dezentes Ghost-Styling
    // (genaues Look&Feel: W3).
    for e in &arena.entities {
        let rx = e.pos.0 - cursor.0 + center.0;
        let ry = e.pos.1 - cursor.1 + center.1;
        if rx < 0 || ry < 0 || rx >= w || ry >= h {
            continue;
        }
        let ch = match &e.kind {
            EntityKind::PowerupWord(pw) => pw.word.chars().next().unwrap_or('◆'),
        };
        grid[ry as usize][rx as usize] =
            Some((ch, Style::default().fg(theme::TEXT_DIM)));
    }
```

(Der Rest der Funktion — `all_tiles`-Sortierung, Tiles-Schleife, Cursor-Marker, `lines`/`render_widget` — bleibt unverändert.)

- [ ] **Step 5: Build + Test**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo build 2>&1 | tail -15
cargo test 2>&1 | tail -20
```
Expected: alles grün, warnungsfrei. (Bestehende Render-Tests laufen weiter; `App::new()` hat eine leere Arena.)

- [ ] **Step 6: Render-Smoke-Test für Entitäten** — in `src/render/mod.rs` `mod tests` ergänzen:

```rust
    #[test]
    fn draw_world_renders_arena_entity_without_panic() {
        use crate::game::arena::{EntityKind, PowerupWord};
        let mut app = App::new();
        // Entität am Cursor-Ursprung (0,0) → landet in der Bildmitte.
        app.arena_mut().unwrap().spawn(
            (0, 0),
            EntityKind::PowerupWord(PowerupWord { word: "sudo".into() }),
        );
        let out = render_to_string(&mut app);
        assert!(
            out.contains('s'),
            "erster Buchstabe des Powerup-Worts sollte gerendert werden"
        );
    }
```

- [ ] **Step 7: Build + Test**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test 2>&1 | tail -20
```
Expected: neuer Smoke-Test PASS, alles grün.

- [ ] **Step 8: Commit**

```bash
git add src/app.rs src/render/mod.rs src/main.rs
git commit -m "feat(#42): App hält Arena (Single/Client) + draw_world zeichnet Entitäten"
```

---

### Task 5: Welcome-Snapshot (Late-Join) + Client-Arena-Pipeline

**Files:**
- Modify: `src/net/protocol.rs` (`Welcome` bekommt `arena: ArenaSnapshot` + Test-Fix)
- Modify: `src/net/server.rs` (`add_player` packt `arena: self.arena.snapshot()` ins Welcome)
- Modify: `src/game/world.rs` (Test `apply_welcome_*` um `arena: vec![]` ergänzen)
- Modify: `src/net/client.rs` (`connect` liefert zusätzlich `Arena` aus dem Welcome-Snapshot)
- Modify: `src/main.rs` (`run_client`: echten Snapshot übernehmen + Entity-Deltas an die Arena routen)
- Modify: `tests/host_client_e2e.rs` (geänderte `connect`-Signatur)

**Interfaces:**
- Consumes: `Arena::from_snapshot` (Task 1), `Mode::Client(WorldView, Arena)` + `arena_mut` (Task 4).
- Produces:
  - `ServerMsg::Welcome { your_id, color, players, arena: ArenaSnapshot }`
  - `connect(addr, name) -> Result<(WorldView, Arena, ClientHandle)>`

- [ ] **Step 1: `Welcome` um `arena` erweitern** — in `src/net/protocol.rs`:

Import ergänzen (`ArenaSnapshot` zu den vorhandenen `arena`-Imports):

```rust
use crate::game::arena::{ArenaSnapshot, Entity, EntityId};
```

`Welcome`-Variante:

```rust
    Welcome {
        your_id: PlayerId,
        color: PlayerColor,
        players: Vec<PlayerSnapshot>,
        arena: ArenaSnapshot,
    },
```

Den bestehenden Test `server_msg_welcome_roundtrip` fixen (Feld ergänzen):

```rust
    #[test]
    fn server_msg_welcome_roundtrip() {
        let msg = ServerMsg::Welcome {
            your_id: 1,
            color: PALETTE[1],
            players: vec![],
            arena: vec![],
        };
        let back: ServerMsg = decode_line(&encode_line(&msg)).unwrap();
        assert_eq!(msg, back);
    }
```

- [ ] **Step 2: Host packt Snapshot ins Welcome** — in `src/net/server.rs`, `add_player`, die `welcome`-Konstruktion:

```rust
        let welcome = ServerMsg::Welcome {
            your_id: id,
            color,
            players: self.snapshot(),
            arena: self.arena.snapshot(),
        };
```

- [ ] **Step 3: `world.rs`-Test fixen** — in `src/game/world.rs` `mod tests`, der Test `apply_welcome_populates_players_and_self` konstruiert ein `Welcome`; `arena: vec![]` ergänzen:

```rust
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
            arena: vec![],
        });
```

- [ ] **Step 4: Build (verify ripple gefangen)**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo build 2>&1 | tail -20
```
Expected: Kompilierfehler in `src/net/client.rs` (`connect` matched `Welcome` exhaustiv ohne `arena`) und ggf. `tests/host_client_e2e.rs`. Diese werden in Step 5/7 gefixt. (Falls die Tests in `cargo build` nicht erfasst werden, erscheinen sie erst bei `cargo test`.)

- [ ] **Step 5: `connect` liefert die Arena** — in `src/net/client.rs`:

Import ergänzen:

```rust
use crate::game::arena::Arena;
```

Signatur + Welcome-Handling in `connect` anpassen:

```rust
pub fn connect(addr: &str, name: &str) -> Result<(WorldView, Arena, ClientHandle)> {
    let stream = TcpStream::connect(addr)?;
    let mut write = stream.try_clone()?;
    write.write_all(
        encode_line(&ClientMsg::Hello {
            name: name.to_string(),
        })
        .as_bytes(),
    )?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Err(anyhow!("Verbindung vom Host geschlossen"));
    }
    let (world, arena) = match decode_line::<ServerMsg>(&line)? {
        ServerMsg::Welcome {
            your_id,
            color,
            players,
            arena,
        } => {
            let mut w = WorldView::new(your_id);
            w.apply(ServerMsg::Welcome {
                your_id,
                color,
                players,
                arena: Vec::new(), // WorldView ignoriert die Arena (Sim ≠ Render)
            });
            (w, Arena::from_snapshot(arena))
        }
        ServerMsg::Reject { reason } => return Err(anyhow!(reason)),
        _ => return Err(anyhow!("unerwartete erste Nachricht vom Host")),
    };
```

Und das abschließende `Ok((world, ClientHandle { write, rx }))` ändern zu:

```rust
    Ok((world, arena, ClientHandle { write, rx }))
```

- [ ] **Step 6: `main.rs::run_client` — echten Snapshot + Delta-Routing**

Den in Task 4 gesetzten Platzhalter ersetzen:

```rust
    let (world, arena, mut handle) = connect(&addr, &name)?;
    let mut app = App::new_with_mode(Mode::Client(world, arena));
```

Den Empfangs-Loop so anpassen, dass Entity-Deltas an die Arena gehen, der Rest an die `WorldView`:

```rust
        let mut host_gone = false;
        loop {
            match handle.rx.try_recv() {
                Ok(msg) => {
                    if let Mode::Client(w, arena) = &mut app.mode {
                        match msg {
                            ServerMsg::EntitySpawned { entity } => arena.apply_spawned(entity),
                            ServerMsg::EntityDespawned { id } => arena.apply_despawned(id),
                            other => w.apply(other),
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    host_gone = true;
                    break;
                }
            }
        }
```

> `ServerMsg` ist in `run_client` zu importieren — die `use prfh::net::protocol::InputEvent;`-Zeile zu `use prfh::net::protocol::{InputEvent, ServerMsg};` erweitern.

- [ ] **Step 7: `tests/host_client_e2e.rs` an die neue `connect`-Signatur anpassen**

Im Test die `connect`-Destrukturierung anpassen. Suche die Zeile mit `connect(` (Form `let (mut world, mut handle) = connect(...)` o. ä.) und ergänze die Arena-Bindung, z. B.:

```rust
    let (mut world, _arena, mut handle) = connect(&addr, "Bob").unwrap();
```

(Falls der Test die Arena nicht prüft, `_arena` verwenden, um unused-Warnungen zu vermeiden.)

- [ ] **Step 8: Build + Test**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo build 2>&1 | tail -10
cargo test 2>&1 | tail -25
```
Expected: alles grün, warnungsfrei (inkl. `tests/host_client_e2e.rs`, `tests/host_handshake.rs`).

- [ ] **Step 9: Commit**

```bash
git add src/net/protocol.rs src/net/server.rs src/game/world.rs src/net/client.rs src/main.rs tests/host_client_e2e.rs
git commit -m "feat(#42): Welcome trägt Arena-Snapshot (Late-Join) + Client-Delta-Routing"
```

---

### Task 6: Loopback-Integrationstest (`tests/world_sync_e2e.rs`)

**Files:**
- Create: `tests/world_sync_e2e.rs`

**Interfaces:**
- Consumes: `connect` (Task 5), `HostState::spawn_entity` (Task 3), `Arena::{apply_spawned, from_snapshot}` (Task 1), `spawn_listener`/`send_msg`/`HostEvent` (vorhanden).

> Modelliert nach `tests/host_client_e2e.rs` (Host-Loop in einem Background-Thread). Zwei Fälle: **(a) Delta** — Client connectet, Host spawnt danach → Client sieht `EntitySpawned`. **(b) Late-Join** — Host spawnt zuerst → der `Welcome`-Snapshot trägt die Entität.

- [ ] **Step 1: Testdatei schreiben**

```rust
use std::collections::HashMap;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use prfh::game::arena::{Arena, EntityKind, PowerupWord};
use prfh::net::client::connect;
use prfh::net::protocol::ServerMsg;
use prfh::net::server::{spawn_listener, HostEvent, HostState};

fn powerup(word: &str) -> EntityKind {
    EntityKind::PowerupWord(PowerupWord { word: word.into() })
}

/// (a) Delta-Pfad: Host spawnt eine Entität, NACHDEM der Client verbunden ist.
/// Der Client muss das `EntitySpawned`-Delta empfangen und in seine Arena-Kopie
/// anwenden.
#[test]
fn client_sees_entity_spawned_via_delta() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let rx = spawn_listener(listener);

    // Host-Thread: akzeptiert den Client, spawnt dann eine Entität und
    // broadcastet das Delta.
    let host = std::thread::spawn(move || {
        let mut host = HostState::new("Host".into());
        let mut streams: HashMap<u64, std::net::TcpStream> = HashMap::new();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut spawned = false;
        while Instant::now() < deadline {
            while let Ok(ev) = rx.try_recv() {
                if let HostEvent::Hello {
                    conn_id,
                    name,
                    mut write,
                } = ev
                {
                    let outcome = host.add_player(name).unwrap();
                    prfh::net::server::send_msg(&mut write, &outcome.welcome).unwrap();
                    streams.insert(conn_id, write);
                }
            }
            // Sobald ein Client da ist, einmalig spawnen + broadcasten.
            if !spawned && !streams.is_empty() {
                let msg = host.spawn_entity((7, 3), powerup("rebase"));
                for s in streams.values_mut() {
                    let _ = prfh::net::server::send_msg(s, &msg);
                }
                spawned = true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    let (_world, mut arena, handle) = connect(&addr.to_string(), "Bob").unwrap();
    assert!(arena.entities.is_empty(), "frischer Client startet ohne Entitäten");

    // Auf das Delta warten und anwenden.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut got = false;
    while Instant::now() < deadline && !got {
        if let Ok(msg) = handle.rx.recv_timeout(Duration::from_millis(200)) {
            if let ServerMsg::EntitySpawned { entity } = msg {
                arena.apply_spawned(entity);
                got = true;
            }
        }
    }
    assert!(got, "Client hat kein EntitySpawned-Delta empfangen");
    assert_eq!(arena.entities.len(), 1);
    assert_eq!(arena.entities[0].pos, (7, 3));

    drop(handle);
    let _ = host.join();
}

/// (b) Late-Join-Pfad: Host spawnt eine Entität, BEVOR der Client verbindet.
/// Der `Welcome`-Snapshot muss die Entität tragen, sodass `connect` eine
/// vorbefüllte Arena liefert.
#[test]
fn late_join_client_gets_entity_via_welcome_snapshot() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let rx = spawn_listener(listener);

    let host = std::thread::spawn(move || {
        let mut host = HostState::new("Host".into());
        // VOR dem Accept spawnen → landet im Welcome-Snapshot.
        host.spawn_entity((1, 2), powerup("sudo"));
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            while let Ok(ev) = rx.try_recv() {
                if let HostEvent::Hello {
                    name, mut write, ..
                } = ev
                {
                    let outcome = host.add_player(name).unwrap();
                    prfh::net::server::send_msg(&mut write, &outcome.welcome).unwrap();
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    let (_world, arena, _handle): (_, Arena, _) = connect(&addr.to_string(), "Late").unwrap();
    assert_eq!(arena.entities.len(), 1, "Late-Join muss die Entität via Snapshot sehen");
    assert_eq!(arena.entities[0].pos, (1, 2));
    match &arena.entities[0].kind {
        EntityKind::PowerupWord(pw) => assert_eq!(pw.word, "sudo"),
    }

    let _ = host.join();
}
```

- [ ] **Step 2: Build + Test**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo test --test world_sync_e2e 2>&1 | tail -25
```
Expected: beide Tests PASS. (Falls flaky durch Timing: Deadlines sind großzügig (3 s); bei echtem Fail die Logik prüfen, nicht die Zeit erhöhen.)

- [ ] **Step 3: Voller Lauf — alles grün & warnungsfrei**

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo build 2>&1 | tail -5
cargo test 2>&1 | tail -25
cargo build --release 2>&1 | tail -5   # warnungsfrei auch im Release
```
Expected: gesamte Suite grün, keine Warnungen.

- [ ] **Step 4: Commit**

```bash
git add tests/world_sync_e2e.rs
git commit -m "test(#42): Loopback-Integrationstest — Entity-Delta + Late-Join-Snapshot"
```

---

## Abschluss (nach Task 6)

- [ ] **Code-Review** vor `gh pr ready`: Skill `superpowers:requesting-code-review` bzw. `code-reviewer`-Subagent auf den Diff `main..issue-42`. Fokus: Sim/Render-Trennung (Arena ≠ WorldView), keine neuen Warnungen, Sync-Symmetrie zum Trail-Modell (Delta + Welcome-Snapshot), Idempotenz von `apply_spawned`.
- [ ] **Learnings festhalten** (CLAUDE.md-Norm): Falls beim Enum-Ripple (Welcome-Feld) oder beim Loopback-Test eine nicht-offensichtliche Falle auftrat → kurzer Eintrag in `CLAUDE.md` oder ein `.claude/skills/`-Skill, im selben PR.
- [ ] `gh pr ready 45` (PR aus Draft holen), aktuelles `main` reinmergen, CI grün abwarten.

---

## Self-Review (gegen Spec & Issue #42)

**Spec-Coverage:**
- §4 Datenmodell (`Arena`/`Entity`/`EntityKind`/`PowerupWord`, kein `bounds`/`terrain`) → Task 1 ✓
- §4 Mutatoren (`spawn`/`despawn`/`entity_at`/`snapshot`/`from_snapshot`/`apply_spawned`/`apply_despawned`) + Tests → Task 1 ✓
- §5 Sync (`EntitySpawned`/`EntityDespawned` + Welcome-Snapshot, host-autoritativ, Client-Kopie, Single direkt) → Tasks 2,3,5 ✓
- §6 Modi-Verdrahtung (Single/Host/Client getrennt, je dieselbe Arena, `App::arena()`) → Tasks 3,4 ✓
- §7 Rendering (`draw_world` + `&Arena`, Entitäten vor Trails, gleiche Transform) → Task 4 ✓
- §9 Testbarkeit (Unit + Protokoll-Roundtrip + Loopback-Integration) → Tasks 1,2,6 ✓
- §10 Skeleton (App/Arena-Stubs, die W2/W3 erweitern: `arena_mut`, `HostState::spawn_entity`) → Tasks 3,4 ✓

**Issue-#42-Akzeptanzkriterien:**
- `arena.rs` Struktur ohne `bounds`/`terrain` → Task 1 ✓
- Mutatoren unit-getestet → Task 1 ✓
- `protocol.rs` Deltas + Welcome-Snapshot + Roundtrip-Tests → Tasks 2,5 ✓
- Host besitzt Arena, broadcastet, Snapshot ins Welcome; Client-Kopie; Single direkt; Modi getrennt → Tasks 3,4,5 ✓
- `draw_world` zeichnet Entitäten vor Trails → Task 4 ✓
- Loopback: Delta + Late-Join → Task 6 ✓
- Skeleton für W2/W3 → Tasks 3,4 ✓
- Spec-Datei im Branch committed → bereits erledigt (Commit vor Task 1) ✓
- `cargo build`/`cargo test` grün & warnungsfrei → jede Task Step "Build + Test" ✓

**Placeholder-Scan:** keine TBD/TODO/„handle edge cases"-Platzhalter; jeder Code-Step zeigt vollständigen Code.

**Typ-Konsistenz:** `EntityId = u32`, `ArenaSnapshot = Vec<Entity>`, `Arena::from_snapshot`, `apply_spawned`/`apply_despawned`, `HostState::arena()`/`spawn_entity()`, `App::arena()`/`arena_mut()`, `connect -> (WorldView, Arena, ClientHandle)` — über alle Tasks identisch verwendet.
