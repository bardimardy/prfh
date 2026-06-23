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
