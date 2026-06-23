# Design: Welt-Base-Engine (Arena-Substrat)

- **Datum:** 2026-06-23
- **Status:** Entwurf (zur Freigabe)
- **Scope:** Das *geteilte, host-autoritative Welt-Substrat* als Fundament — ein
  Koordinatenraum mit vorplatzierten Entitäten, der über das bestehende
  Multiplayer-Modell synct. Powerups sind der **erste Konsument**. Battle-Royale
  (schrumpfende Zone) und prozedurale Landschaft sind **designed-for-later**,
  werden hier aber **nicht gebaut**.
- **Supersedet:** Den „Welt-Modell"-Teil der älteren Powerup-Spec
  (`2026-06-22-powerup-inventory-effects-design.md`), die *vor* dem
  Multiplayer-Merge entstand und annahm, `src/game/world.rs` sei frei.

Dieses Dokument wurde gegen die echte Codebase (`protocol.rs`, `app.rs`,
`server.rs`, `render/mod.rs`) und mit einem Review-Subagent gegengeprüft.

---

## 1. Ziel

Heute hat das Spiel keine **Welt** im eigentlichen Sinn: gerendert wird nur der
eigene Trail (Single-Player) bzw. die fremden Trails (Multiplayer), Cursor-zentriert
über einem unendlichen `(i32, i32)`-Raum. Es gibt **keine vorplatzierten Tiles**,
keine Entitäten, die *vor* dem Erreichen auf der Map liegen.

Die Welt-Base-Engine schafft genau das: eine **`Arena`** — eine geteilte,
host-autoritative Sammlung platzierter Entitäten. Sie ist das Fundament, auf dem
Powerups (Pickup-Wörter), später Items/Hindernisse, eine Battle-Royale-Zone und
prozedurale Generierung aufsetzen — **ohne Rework**, weil der Schnitt von Anfang
auf additives Wachstum ausgelegt ist.

## 2. Kontext & der zentrale Befund

| Was existiert | Wo |
|---|---|
| Base-Typing-Mechanik | `src/game/writing.rs` |
| **Multiplayer (host-autoritativ)** | `src/net/` + `WorldView`/`PlayerView` in `src/game/world.rs` |
| Effekt-Layer (tachyonfx) | `src/effects/` + Render-Hook |
| Frameless HUD + Overlay-Framework + Notifications | `src/hud/`, `src/render/` |

**Kollision, die diese Spec auflöst:** `src/game/world.rs` ist **bereits belegt** —
als **Render-Modell** (`WorldView`/`PlayerView`: Trails, Farben, Pace, Delta-Apply).
Die alte Powerup-Spec wollte dort ein neues Map-Modell anlegen. Das würde Sim- und
Render-Modell vermischen und den Client-Delta-Pfad (`world.rs::apply`) brechen.

**Auflösung:** Die Sim-Welt lebt in einer **eigenen** Datei `src/game/arena.rs`,
strikt getrennt vom Render-`WorldView`. (Kein Umbenennen von `world.rs` — das wäre
nur Churn + Merge-Risiko.)

## 3. Kern-Entscheidungen

| Frage | Entscheidung | Begründung |
|---|---|---|
| Welt-Repräsentation | **Sparse Entity-Layer** (kein Terrain-Grid) | Powerups brauchen nur „Entität an Position". Terrain/Zone haben heute **null Konsumenten**. |
| Struktur | **Eine `Arena`-Struct, additiv wachsend** | Erweiterbarkeit kommt aus der *Struktur* (eigene Struct + generischer Sync), nicht aus Platzhalter-Feldern. |
| BR-Zone jetzt? | **Nein.** Kein `bounds`-Feld, kein `Bounds`-Typ. | Ungenutztes Feld + ungenutzter Typ = toter Code → verstößt gegen CLAUDE.md („keine Warnungen, toten Code entfernen"). Zone ist später ein additiver Diff. |
| Autorität | **Host besitzt die `Arena`** | Spiegelt das bestehende autoritative Trail-Modell; eine Quelle der Wahrheit. |
| Sync | **Neue `ServerMsg`-Deltas + `Welcome`-Snapshot** | 1:1-Muster zum bestehenden `Wrote`/`Erased` + `Welcome { players }`. |
| Sim vs. Render | **`Arena` getrennt von `WorldView`** | Verhindert Vermischung; Client-Delta-Pfad bleibt intakt. |
| Single-Player | **Nicht** mit Host vereinheitlichen (jetzt) | MP-Pfad ist fragil (#37 war frische MP-Merge-Regression). Alle Modi referenzieren dieselbe `Arena`, bleiben aber getrennt. |
| Dependencies | **Keine neuen** | `serde`+`ron` (vorhanden) für Snapshot/Delta-Serialisierung. |

## 4. Datenmodell (`src/game/arena.rs`)

```rust
pub type EntityId = u32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id:   EntityId,
    pub pos:  (i32, i32),     // gleicher Koordinatenraum wie Trails/Cursor
    pub kind: EntityKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntityKind {
    PowerupWord(PowerupWord),   // erster (und vorerst einziger) Konsument
    // additiv erweiterbar: Item(...), Obstacle(...), ...
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Arena {
    pub entities: Vec<Entity>,
    next_id: EntityId,          // monoton, vergibt EntityIds
}
```

> `Arena` hält **nur** `entities` (+ ID-Vergabe). **Kein** `bounds`, **kein**
> `terrain` — diese kommen später als additive Felder, *wenn* sie einen Konsumenten
> haben. Die `next_id`-Vergabe lebt im Host; Clients übernehmen IDs aus Deltas.

**Mutatoren (alle unit-testbar, ohne Sockets):**

- `spawn(pos, kind) -> EntityId` — vergibt ID, fügt Entität ein.
- `despawn(id)` — entfernt Entität.
- `entity_at(pos) -> Option<&Entity>` — Lookup für Pickup/Kollision.
- `snapshot() -> Vec<Entity>` / `from_snapshot(Vec<Entity>)` — Voll-Zustand für
  Late-Join. `ArenaSnapshot` = `Vec<Entity>`; `next_id` wird **nicht** übertragen
  (nur der Host vergibt IDs, Clients übernehmen sie aus Deltas).
- `apply_spawned(Entity)` / `apply_despawned(EntityId)` — Client-seitiges Anwenden
  der `EntitySpawned`/`EntityDespawned`-Deltas (analog zu `WorldView::apply`, das
  die `ServerMsg`-Varianten anwendet). Idempotent gegen Duplikate.

`PowerupWord`-Layout (Origin/Achse/Reversed + Keystroke→Tile-Mapping) übernimmt die
Detailregeln aus der Powerup-Spec §5 — gehört aber in W2 (`powerup.rs`), nicht ins
Substrat. Im Substrat ist `PowerupWord` nur ein opaker `EntityKind`-Payload.

## 5. Sync-Modell (host-autoritativ)

Das Welt-Substrat synct **parallel zum bestehenden Trail-Sync** und im selben
Muster (`src/net/protocol.rs`).

```rust
// Neue Delta-Varianten (Host → Clients)
pub enum ServerMsg {
    // ... bestehend: Welcome, PlayerJoined/Left, Wrote, Erased, Died, Respawned
    EntitySpawned   { entity: Entity },
    EntityDespawned { id: EntityId },
}

// Welcome bekommt den Welt-Snapshot fürs Late-Join dazu:
ServerMsg::Welcome { your_id, color, players: Vec<PlayerSnapshot>, arena: ArenaSnapshot }
```

- **Host** (`src/net/server.rs` / `HostState`): hält die autoritative `Arena`,
  mutiert sie (Spawn beim Welt-Aufbau, Despawn beim Pickup in W2) und broadcastet
  `EntitySpawned`/`EntityDespawned`. Der Welt-Snapshot wandert ins `Welcome`.
- **Client** (`src/net/client.rs`): pflegt eine **lokale Kopie** der `Arena` aus
  `Welcome`-Snapshot + eingehenden Deltas (`apply_delta`).
- **Single** (`src/app.rs` `Mode::Single`): hält die `Arena` **direkt**, ohne
  Netz-Umweg.

**Zone-Andockpunkt (designed-for, nicht gebaut):** Ein späteres
`ServerMsg::ZoneUpdate { bounds }` steht additiv neben den Entity-Deltas — gleicher
Kanal, kein Transport-Umbau.

## 6. Modi-Verdrahtung (`src/app.rs`)

`Mode::Single | Host | Client` bleiben getrennt; jeder Modus **referenziert/hält
dieselbe `Arena`-Struct**:

- `Mode::Single(engine)` → zusätzlich eine eigene `Arena`.
- `Mode::Host(host)` → die autoritative `Arena` in `HostState`.
- `Mode::Client(world_view)` → die aus Deltas gepflegte `Arena`-Kopie.

`App` exponiert die aktuelle `&Arena` fürs Rendering (analog zu `world_view()`).

## 7. Rendering (`src/render/mod.rs`)

`draw_world` (heute `render/mod.rs:187`, Cursor-zentriert, `(i32,i32)`→Screen)
bekommt zusätzlich `&Arena` und zeichnet die Entitäten mit **derselben Transform**
**vor** den Trails (Trails liegen optisch oben). Nicht eingesammelte Powerup-Wörter
werden dezent/ghost-styled gezeichnet (genaues Styling: W3).

Die sparse Entity-Schicht passt verlustfrei in die bestehende Transform-Schleife —
kein Rendering-Umbau, nur eine zusätzliche Zeichen-Passage.

## 8. Erweiterungspfad (designed-for-later — hier NICHT gebaut)

Die Struktur garantiert additive Erweiterung ohne Rework:

| Spätere Erweiterung | Additiver Diff |
|---|---|
| Neue Entitätsarten (Items, Hindernisse) | `+ EntityKind::Item(..)` — Sync/Render tragen es automatisch. |
| **Battle-Royale-Zone** | `+ bounds: Option<Bounds>` an `Arena`, `+ struct Bounds`, `+ ServerMsg::ZoneUpdate`, `+ Shrink-Tick`. |
| **Prozedurale Landschaft** | `+ terrain: Option<Terrain>` an `Arena`, Generator platziert Entitäten/Terrain; Chunk-Sync als eigene Delta-Varianten. |
| Single/Host-Unifikation | Wenn BR es erzwingt: Single wird „Host mit 1 Spieler". |

## 9. Testbarkeit (TDD wo sinnvoll)

Unit-testbar (Pflicht, ohne Sockets/Rendering):

- `Arena`-Mutatoren: `spawn` vergibt monotone IDs, `despawn` entfernt, `entity_at`
  findet/verfehlt.
- `snapshot`/`from_snapshot`-Roundtrip (Voll-Zustand identisch).
- `apply_spawned`/`apply_despawned`: fügt ein bzw. entfernt; idempotent gegen
  Duplikate (doppeltes `EntitySpawned` derselben ID erzeugt keine Dublette).
- **Protokoll-Roundtrip** für die neuen `ServerMsg`-Varianten (serialize→deserialize).
- **Integration über Loopback** (`127.0.0.1`): Host spawnt Entität → Client sieht sie
  via Delta; Late-Join-Client bekommt sie via `Welcome`-Snapshot.

Nicht unit-testbar: das Entitäts-Rendering in `draw_world` (visuell) — höchstens
„baut ohne Panik"-Smoke-Test.

`cargo build` + `cargo test` müssen warnungs- und fehlerfrei grün bleiben.

## 10. Issue-Schnitt & Sequencing

Ein Design-Doc (dieses), daraus **drei** Issues. Reihenfolge minimiert
Merge-Kollisionen (`app.rs`/`render/mod.rs`/`protocol.rs` sind die Hotspots):

```
W1  →  W2  →  W3
```

| Issue | Inhalt | Berührt v.a. | Hängt ab |
|---|---|---|---|
| **W1 — Arena-Substrat** | `src/game/arena.rs` (`Arena`/`Entity`/`EntityKind` + Mutatoren + Tests); `protocol.rs` (`EntitySpawned`/`EntityDespawned` + `Welcome`-Snapshot); Host/Client/Single halten `Arena`; `draw_world` zeichnet Entitäten. | `arena.rs` (neu), `protocol.rs`, `server.rs`, `client.rs`, `app.rs`, `render/mod.rs` | — |
| **W2 — Powerup/Inventar-Engine** | `powerup.rs`/`inventory.rs` (Layout/Prefix-Match), Trace-FSM in `writing.rs` (Beobachter), Cast-Modus + Dispatch-Hook. `PowerupWord` als `EntityKind`. + Test-Powerup hinter `PRFH_DEBUG`. | `powerup.rs`/`inventory.rs` (neu), `writing.rs`, `app.rs` | W1 |
| **W3 — HUD + Spawn/Generierung** | Inventar-Overlay, Shadow-Autocomplete-Highlight, Pickup-/Wellen-Animationen verdrahtet; echtes Spawnen von Powerup-Entitäten in die `Arena`. | `render/mod.rs`, `app.rs` | W1, W2 |

**Mapping zu Alt-Issues:** W1 = der „Welt-Modell"-Teil von #30 + der nötige Sync;
W2 = Rest von #30 (Trace/Cast) + #32 (Test-Powerup); W3 = #31 (HUD) + echtes Spawn.
Alt-Issues #30–#33 werden als „superseded" geschlossen.

**Kollisions-Vermeidung (zwei Claude-Instanzen):** W1 landet ein kleines
`App`/`Arena`-Skelett (Felder + Stub-Methoden), das W2/W3 nur **erweitern**, statt
dieselbe Methode mehrfach umzuschreiben. Details aus der Powerup-Spec §6–§8
(Trace-FSM, Cast-Modus, Overlay) gelten unverändert für W2/W3.

## 11. Offene Detailpunkte (im Plan/Issue zu fixieren)

- Konkrete `EntityId`-Breite (`u32` vorgeschlagen) und ID-Vergabe-Reset bei
  Host-Neustart.
- Ghost-Styling der nicht eingesammelten Map-Entitäten (W3).
- Wo der initiale Welt-Aufbau (Spawn der Start-Entitäten) lebt — Host-Init vs.
  separater Welt-Generator-Hook (Andockpunkt für spätere prozedurale Gen).
- Verhalten bei Pickup im MP: Despawn ist host-autoritativ; Race zweier Spieler auf
  dieselbe Entität → erster gewinnt (Host entscheidet), `EntityDespawned` an alle.
