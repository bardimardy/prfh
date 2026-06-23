---
name: net-sync
description: Wissen über das host-autoritative Sync-Modell in prfh (src/net/ + Arena/WorldView). Use IMMER bevor du das Netz-Protokoll anfasst — neue ServerMsg-Varianten/-Felder, Welcome-Snapshot, Broadcast, Sim-vs-Render-Trennung. Kennt das Delta+Snapshot-Muster, die per-Task-grün-Disziplin beim Enum-Ripple und das Loopback-Test-Muster. Triggert auf ServerMsg, protocol.rs, Welcome, broadcast, Sync, host-autoritativ, EntitySpawned, EntityDespawned, Late-Join, WorldView, Arena-Sync.
---

# Host-autoritatives Sync-Modell (prfh)

Verbindliches Wissen für jede Arbeit an `src/net/` (protocol/server/client) oder am
Sync von Welt-Zustand. Eingeführt/verifiziert mit W1 (#42, PR #45). Quelle:
`docs/superpowers/specs/2026-06-23-world-base-engine-design.md` §5–§7 + gegen die
echte Codebase verifiziert.

## Das Muster: Delta-Varianten + Welcome-Snapshot

Aller geteilter Zustand synct host-autoritativ nach **einem** Muster:

1. **Laufende Änderungen** = eine `ServerMsg`-Delta-Variante, die der Host an alle
   Clients **broadcastet**. Beispiele: `Wrote`, `Erased`, `Respawned`,
   `EntitySpawned`, `EntityDespawned`.
2. **Late-Join** = der Voll-Zustand reist im `Welcome` mit (ein Snapshot-Feld pro
   Subsystem: `players: Vec<PlayerSnapshot>`, `arena: ArenaSnapshot`).

**Broadcast-Konvention:** Ein State-Mutator auf `HostState` **gibt die zu sendende
`ServerMsg` zurück**, der Aufrufer broadcastet sie. Nie im Mutator selbst senden.
So sieht jeder neue Sync-Punkt aus:

```rust
// HostState:
pub fn spawn_entity(&mut self, pos, kind) -> ServerMsg {
    let id = self.arena.spawn(pos, kind.clone());
    ServerMsg::EntitySpawned { entity: Entity { id, pos, kind } }
}
// run_host (main.rs): broadcast(&mut streams, None, &msg);
```

Das spiegelt `apply_input → Some(ServerMsg::Wrote{..}) → broadcast` 1:1. Ein neues
Delta, das nicht diesem Muster folgt, ist fast immer ein Fehler.

## ⚠️ Sim ≠ Render: NIE vermischen

- **`Arena`** (`src/game/arena.rs`) ist die **Sim-Welt** (platzierte Entitäten).
- **`WorldView`/`PlayerView`** (`src/game/world.rs`) ist das **Render-Modell**
  (Trails, Farben, Pace). **NICHT umbenennen.**

`WorldView::apply` ist die Client-Delta-FSM für Spieler-/Trail-Zustand. Entity-Deltas
gehören dort **nicht** hin — sie sind ein No-Op-Arm:

```rust
// world.rs WorldView::apply
ServerMsg::EntitySpawned { .. } | ServerMsg::EntityDespawned { .. } => {}
```

Stattdessen routet der **Client-Loop** (`main.rs::run_client`) Entity-Deltas an die
Arena-Kopie, alles andere an die `WorldView`:

```rust
if let Mode::Client(w, arena) = &mut app.mode {
    match msg {
        ServerMsg::EntitySpawned { entity } => arena.apply_spawned(entity),
        ServerMsg::EntityDespawned { id }   => arena.apply_despawned(id),
        other => w.apply(other),
    }
}
```

Und im `Welcome`-Handshake (`client.rs::connect`) wird der Arena-Snapshot
**aktiv vom WorldView ferngehalten**, indem `apply` ein leeres Arena-Feld bekommt und
der echte Snapshot in `Arena::from_snapshot` wandert:

```rust
w.apply(ServerMsg::Welcome { your_id, color, players, arena: Vec::new() });
(w, Arena::from_snapshot(arena))
```

## ⚠️ Enum-Ripple: so bleibt `main` PRO TASK grün

`ServerMsg` zu ändern rippelt durch protocol/server/client/world + Tests. Die
Reihenfolge entscheidet, ob jeder Commit für sich kompiliert:

**Neue Variante hinzufügen** (z. B. `EntityDespawned`): bricht nur **erschöpfende
`match`** — in der Praxis genau `WorldView::apply`. Ein No-Op-Arm + Roundtrip-Test,
fertig. **Eine** in sich grüne Task.

**Feld zu bestehender Variante hinzufügen** (z. B. `Welcome.arena`): bricht **jede
Konstruktion UND jede erschöpfende Destrukturierung** auf einmal:
- `server.rs::add_player` (Konstruktion) — braucht die Datenquelle, **also muss das
  Feld-Backing zuerst existieren** (erst `HostState.arena`, dann `Welcome.arena`).
- `client.rs::connect` (destrukturiert `Welcome` **ohne** `..` → bricht; hier den
  Snapshot extrahieren).
- Protokoll-Roundtrip-Test + `world.rs`-Test (Konstruktionen).
- `world.rs::apply` nutzt `Welcome { your_id, players, .. }` **mit `..`** → tolerant,
  bricht **nicht**. (Bewusst so: das ist die Sollbruchstelle-Vermeidung.)
→ All das in **einer** Task fixen, sonst ist ein Zwischen-Commit rot.

Merke: `..` in einem `match`-Arm macht ihn tolerant gegen neue Felder; eine
erschöpfende Destrukturierung ohne `..` ist die Stelle, die garantiert bricht (und
oft genau die, wo du das neue Feld auslesen willst — z. B. `connect`).

## ID-Vergabe & Idempotenz

`EntityId = u32`, **monoton nur vom Host** vergeben (`Arena.next_id`, privat). Clients
vergeben **nie** selbst IDs — `from_snapshot` setzt `next_id: 0` und übernimmt IDs aus
Snapshot/Deltas. `apply_spawned` ist **idempotent** (dedupe auf ID), damit ein
doppeltes Delta keine Dublette erzeugt. Beim MP-Pickup (W2): erster gewinnt, Host
entscheidet, `EntityDespawned` an alle.

## Loopback-Integrationstest-Muster

Sync testet man über echtes TCP + echte RON-Serialisierung (`tests/world_sync_e2e.rs`,
`tests/host_client_e2e.rs` als Vorlage):

- `TcpListener::bind("127.0.0.1:0")` → freier Port; `spawn_listener` liefert den
  `Receiver<HostEvent>`.
- Host-Loop in einem Background-Thread mit `Instant::now() + Duration`-**Deadline**
  manuell treiben (Hello→`add_player`→`send_msg(welcome)`; dann `spawn_entity` +
  `send_msg` an die Streams).
- Vom Haupt-Thread `connect(&addr.to_string(), name)` → `(WorldView, Arena,
  ClientHandle)`.
- **Delta-Pfad:** erst connecten, dann Host spawnen → `handle.rx.recv_timeout(..)` auf
  das `EntitySpawned` warten und `arena.apply_spawned` anwenden.
- **Late-Join-Pfad:** Host **vor** dem Accept spawnen → der `Welcome`-Snapshot trägt
  die Entität, `connect` liefert eine vorbefüllte Arena.

Deadlines großzügig (3 s) wählen; ein echter Fail ist Logik, nicht Timing — Zeit
**nicht** hochdrehen, um Flakiness zu „fixen".

## Skeleton-Hooks (W1 → W2/W3)

W1 landete bewusst `pub`-Hooks ohne Produktiv-Aufrufer, die spätere Issues nur
**erweitern**: `HostState::spawn_entity`, `App::arena()`/`arena_mut()`. Auf einem
**Library-Crate** lösen ungenutzte `pub`-Methoden **keine** dead-code-Warnung aus —
daher kein `#[allow]` nötig. Beim Erweitern: den Hook aufrufen, nicht die Signatur
umschreiben.
