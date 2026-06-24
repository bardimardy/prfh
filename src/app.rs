use crate::game::arena::{Arena, EntityKind};
use crate::game::inventory::Inventory;
use crate::game::powerup::{EffectTag, Powerup, PowerupWord, ENTRY_SNAP_RADIUS};
use crate::game::world::{PlayerId, PlayerView, WorldView};
use crate::game::writing::{StepResult, Trace, TraceStep, WritingEngine};
use crate::hud::notify::{NotificationStack, NotifyKind};
use crate::net::server::HostState;
use std::time::Duration;

/// Render-time-Pickup-Animation: Timer + Inventar-Slot der neuen Zeile (Design §3).
pub struct PickupAnim {
    pub age: Duration,
    pub slot: usize,
}

pub const PICKUP_ANIM_DUR: Duration = Duration::from_millis(600);

impl Default for App {
    fn default() -> Self {
        Self::new_single()
    }
}

pub enum Mode {
    Single(WritingEngine, Arena),
    Host(HostState),
    Client(WorldView, Arena),
}

pub struct App {
    pub should_quit: bool,
    pub mode: Mode,
    pub last_event: String,
    /// Dynamische Quick-Notifications (oben-mitte, schweben über der Welt).
    /// Ersetzt das frühere statische `trigger_banner`.
    pub notifications: NotificationStack,
    pub debug: bool,
    pub debug_lines: Vec<String>,
    /// Inventar der eingesammelten Powerups (Single-Flow; MP getrennt/W3).
    pub inventory: Inventory,
    /// Beobachtende Pickup-Trace-FSM.
    pub trace: Trace,
    /// Cast-Modus aktiv (Tab-Toggle): Zeichen füllen den Buffer statt zu schreiben.
    pub cast_mode: bool,
    pub cast_buffer: String,
    /// Alter der laufenden Cast-Welle (render-time-Ring); None = keine Welle.
    pub cast_wave: Option<Duration>,
    /// Monotone Animations-Uhr fürs render-time-Shimmer (vom Render getrieben).
    pub anim_clock: Duration,
    /// Laufende Pickup-Animation (render-time); None = keine Anim aktiv.
    pub pickup_anim: Option<PickupAnim>,
    /// Inventar-Overlay sichtbar.
    pub inv_visible: bool,
}

impl App {
    /// Leerer App-Zustand ohne vorgeseedete Arena — für Unit-/Render-Tests.
    /// Produktions-Einstieg ist `new_single` (mit `spawn_powerups`).
    pub fn new() -> Self {
        let arena = Arena::new();
        Self {
            should_quit: false,
            mode: Mode::Single(WritingEngine::new((0, 0)), arena),
            last_event: String::from("type to write yourself a path"),
            notifications: NotificationStack::new(),
            debug: false,
            debug_lines: Vec::new(),
            inventory: Inventory::new(),
            trace: Trace::new(),
            cast_mode: false,
            cast_buffer: String::new(),
            cast_wave: None,
            anim_clock: Duration::ZERO,
            pickup_anim: None,
            inv_visible: false,
        }
    }

    pub fn new_with_mode(mode: Mode) -> Self {
        let mut a = App::new_single();
        a.mode = mode;
        a
    }

    pub fn new_single() -> Self {
        let mut arena = Arena::new();
        // Echtes Spawn (Issue D): reguläre Start-Menge. Host-autoritativ; in MP
        // seedet der Host, Clients erhalten die Wörter über EntitySpawned/Snapshot.
        crate::game::powerup::spawn_powerups(&mut arena);
        Self {
            should_quit: false,
            mode: Mode::Single(WritingEngine::new((0, 0)), arena),
            last_event: String::from("type to write yourself a path"),
            notifications: NotificationStack::new(),
            debug: false,
            debug_lines: Vec::new(),
            inventory: Inventory::new(),
            trace: Trace::new(),
            cast_mode: false,
            cast_buffer: String::new(),
            cast_wave: None,
            anim_clock: Duration::ZERO,
            pickup_anim: None,
            inv_visible: false,
        }
    }

    pub fn self_id(&self) -> PlayerId {
        match &self.mode {
            Mode::Single(..) => 0,
            Mode::Host(h) => h.self_id(),
            Mode::Client(w, _) => w.self_id,
        }
    }

    pub fn local_engine(&self) -> Option<&WritingEngine> {
        match &self.mode {
            Mode::Single(e, _) => Some(e),
            Mode::Host(h) => Some(h.local_engine()),
            Mode::Client(..) => None,
        }
    }

    pub fn world_view(&self) -> WorldView {
        match &self.mode {
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
        }
    }

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

    pub fn debug_log<S: Into<String>>(&mut self, line: S) {
        self.debug_lines.push(line.into());
        let max = 12;
        if self.debug_lines.len() > max {
            let drop = self.debug_lines.len() - max;
            self.debug_lines.drain(0..drop);
        }
    }

    pub fn tick(&mut self) {
        // Notifications werden zeitbasiert im Render (mit Frame-`elapsed`)
        // getrieben, nicht hier — `tick` ist frame-/visual-State.
        match &mut self.mode {
            Mode::Single(e, _) => e.tick_visuals(),
            // Host tick_visuals is driven by run_host (which also broadcasts
            // the returned Respawned messages), so we skip it here.
            Mode::Host(_) => {}
            Mode::Client(w, _) => w.tick_visuals(),
        }
    }

    /// Cast-Modus betreten/verlassen (Default-Taste `Tab`). Buffer wird geleert.
    pub fn toggle_cast(&mut self) {
        self.cast_mode = !self.cast_mode;
        self.cast_buffer.clear();
    }

    /// Zeichen im Cast-Modus: füllt den Buffer (schreibt KEIN Tile, bewegt den
    /// Cursor nicht). Bei exaktem Inventar-Namen → Dispatch + Modus verlassen.
    fn on_cast_char(&mut self, c: char) {
        self.cast_buffer.push(c);
        if let Some(p) = self.inventory.get_exact(&self.cast_buffer).cloned() {
            self.dispatch_cast(p.effect_tag, &p.name);
            self.cast_mode = false;
            self.cast_buffer.clear();
        }
    }

    /// Aktivierungs-Dispatch-Hook (Powerup-Spec §7): matcht `effect_tag`. Vorerst
    /// Log + Banner + render-time-Cast-Welle (echte Effekte wie Dash: später).
    fn dispatch_cast(&mut self, tag: EffectTag, name: &str) {
        match &tag {
            EffectTag::Test => {
                self.notifications
                    .push(NotifyKind::Event, "⚡  CAST", name.to_string());
                self.debug_log(format!("cast dispatch: {name} ({tag:?})"));
            }
        }
        // Aktivierungs-Welle über denselben EffectEvent-Seam wie der Pickup.
        self.apply_effect_event(crate::game::powerup::EffectEvent::Activation {
            tag,
            name: name.to_string(),
        });
    }

    /// Single-player local input. (Host/Client routing added in Task 9.)
    pub fn on_char(&mut self, c: char) {
        if c == ' ' {
            return;
        }
        if self.cast_mode {
            self.on_cast_char(c);
            return;
        }
        // Deferred EffectEvent: muss nach dem Ende des Arena-Borrows angewendet
        // werden, da `apply_effect_event` `&mut self` benötigt.
        let mut deferred_ev: Option<crate::game::powerup::EffectEvent> = None;

        if let Mode::Single(e, arena) = &mut self.mode {
            let dir = e.direction;

            // Toleranter Snap-on-Arm (Pickup-Gefühl A): steht der Cursor ≤1 Tile
            // neben einem Eintritts-Tile, fährt in Laufrichtung an und passt der
            // erste Buchstabe, rastet er aufs Eintritts-Tile ein — BEVOR on_char
            // schreibt. Nur im Idle (laufender Trace soll nicht weggerissen
            // werden). Bei exaktem Treffer ist es ein no-op. Die Trace-FSM bleibt
            // Beobachter und sieht danach ein exaktes Eintritts-Tile.
            if !self.trace.is_tracing() {
                let dd = dir.delta();
                if let Some(target) = arena.entities.iter().find_map(|ent| match &ent.kind {
                    EntityKind::PowerupWord(w) => w.entry_snap(e.cursor, dd, c, ENTRY_SNAP_RADIUS),
                }) {
                    e.cursor = target;
                }
            }

            e.trace_suspended = self.trace.is_tracing();
            let result = e.on_char(c);

            // Trace füttern: nur wenn ein Tile geschrieben wurde. Den Pickup
            // (id+name) herausziehen, BEVOR die Arena mutiert wird (der `words`-
            // Borrow muss vor `despawn` enden).
            let mut pickup: Option<(u32, String)> = None;
            if let Some(t) = result.tile() {
                let pos = t.pos;
                // map (nicht filter_map): EntityKind hat heute nur PowerupWord.
                // Ein künftiges Variant bricht das Match exhaustiv → bewusster
                // Revisit (statt stilles Überspringen).
                let words: Vec<(u32, &PowerupWord)> = arena
                    .entities
                    .iter()
                    .map(|ent| match &ent.kind {
                        EntityKind::PowerupWord(w) => (ent.id, w),
                    })
                    .collect();
                if let TraceStep::Completed { id } = self.trace.observe(pos, c, dir, &words) {
                    if let Some((_, w)) = words.iter().find(|(wid, _)| *wid == id) {
                        pickup = Some((id, w.name.clone()));
                    }
                }
            }

            self.last_event = match &result {
                StepResult::Wrote(_) => format!("wrote '{}'", c),
                StepResult::WroteAndTurned(_, d) => format!("turned: {:?}", d),
                StepResult::WroteAndStopped(_) => "paused".into(),
                StepResult::Erased => "erased".into(),
            };

            // Pickup anwenden (host-autoritatives Despawn ist der MP-Andockpunkt;
            // Single despawnt direkt die lokale Arena).
            if let Some((id, name)) = pickup {
                arena.despawn(id);
                self.inventory.add(Powerup {
                    id,
                    name: name.clone(),
                    effect_tag: EffectTag::Test,
                });
                self.notifications.push(NotifyKind::Event, "✦  PICKUP", name.clone());
                // Host-autoritatives Event → lokale render-time-Pickup-Anim auf der
                // gerade hinzugefügten Zeile (Design §3.1). Slot = letzter Index.
                let slot = self.inventory.len() - 1;
                deferred_ev = Some(crate::game::powerup::EffectEvent::Pickup { slot, name });
            } else {
                // Bestehende Turn/Stop-Notifications nur, wenn kein Pickup lief.
                match result {
                    StepResult::WroteAndTurned(_, d) => {
                        self.notifications
                            .push(NotifyKind::Info, "⟹  TURNED", format!("{d:?}"));
                    }
                    StepResult::WroteAndStopped(_) => {
                        self.notifications
                            .push(NotifyKind::Info, "⟹  STOP", "next char overwrites");
                    }
                    _ => {}
                }
            }
        }

        // Arena-Borrow ist beendet; jetzt sicher `&mut self` aufrufen.
        if let Some(ev) = deferred_ev {
            self.apply_effect_event(ev);
        }
    }

    pub fn on_backspace(&mut self) {
        if let Mode::Single(e, _) = &mut self.mode {
            e.on_backspace();
            self.last_event = format!("walked back. doubt: {}", e.doubt);
        }
    }

    pub fn on_enter(&mut self) {}

    /// Wendet ein host-autoritatives EffectEvent auf den lokalen Animations-State
    /// an (Design §3.1). Pickup → render-time-Pickup-Anim auf der Slot-Zeile;
    /// Activation → render-time-Cast-Welle.
    pub fn apply_effect_event(&mut self, ev: crate::game::powerup::EffectEvent) {
        use crate::game::powerup::EffectEvent;
        match ev {
            EffectEvent::Pickup { slot, .. } => {
                self.pickup_anim = Some(PickupAnim { age: Duration::ZERO, slot });
            }
            EffectEvent::Activation { .. } => {
                self.cast_wave = Some(Duration::ZERO);
            }
        }
    }

    /// Schreibt die Pickup-Animation fort und räumt sie nach `PICKUP_ANIM_DUR` ab.
    /// Reine Funktion der Zeit (analog cast_wave) → unit-testbar.
    pub fn advance_pickup_anim(&mut self, dt: Duration) {
        if let Some(a) = self.pickup_anim.as_mut() {
            a.age += dt;
            if a.age >= PICKUP_ANIM_DUR {
                self.pickup_anim = None;
            }
        }
    }
}

#[cfg(test)]
mod w3_tests {
    use super::*;

    #[test]
    fn apply_pickup_event_starts_anim_on_slot() {
        use crate::game::powerup::EffectEvent;
        let mut app = App::new_single();
        app.apply_effect_event(EffectEvent::Pickup { slot: 2, name: "warp".into() });
        let a = app.pickup_anim.as_ref().expect("anim started");
        assert_eq!(a.slot, 2);
        assert_eq!(a.age, std::time::Duration::ZERO);
    }

    #[test]
    fn apply_activation_event_fires_cast_wave() {
        use crate::game::powerup::{EffectEvent, EffectTag};
        let mut app = App::new_single();
        app.apply_effect_event(EffectEvent::Activation { tag: EffectTag::Test, name: "dash".into() });
        assert!(app.cast_wave.is_some());
    }

    #[test]
    fn pickup_anim_advances_then_clears_after_duration() {
        use crate::game::powerup::EffectEvent;
        let mut app = App::new_single();
        app.apply_effect_event(EffectEvent::Pickup { slot: 0, name: "dash".into() });
        app.advance_pickup_anim(std::time::Duration::from_millis(100));
        assert_eq!(app.pickup_anim.as_ref().unwrap().age, std::time::Duration::from_millis(100));
        app.advance_pickup_anim(std::time::Duration::from_millis(600)); // über PICKUP_ANIM_DUR
        assert!(app.pickup_anim.is_none(), "anim cleared after its duration");
    }
}

#[cfg(test)]
mod w2_tests {
    use super::*;
    use crate::game::arena::EntityKind;
    use crate::game::powerup::{Axis, EffectTag, PowerupWord};

    fn spawn_dash(app: &mut App) {
        app.arena_mut().unwrap().spawn(
            (3, 0),
            EntityKind::PowerupWord(PowerupWord {
                name: "dash".into(),
                origin: (3, 0),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
    }

    #[test]
    fn tracing_word_picks_it_up_into_inventory_and_despawns() {
        let mut app = App::new(); // player at (0,0) moving Right
        spawn_dash(&mut app);
        // 3 filler chars walk the cursor to (3,0), then "dash" arms+completes.
        for ch in "xxxdash".chars() {
            app.on_char(ch);
        }
        assert_eq!(app.inventory.len(), 1, "dash should be collected");
        assert_eq!(app.inventory.items[0].name, "dash");
        assert!(app.arena().entities.is_empty(), "picked-up word despawns");
    }

    #[test]
    fn cast_exact_name_dispatches_and_leaves_cast_mode() {
        let mut app = App::new();
        app.inventory.add(Powerup {
            id: 0,
            name: "dash".into(),
            effect_tag: EffectTag::Test,
        });
        app.toggle_cast();
        assert!(app.cast_mode);
        for ch in "dash".chars() {
            app.on_char(ch); // routed to cast buffer while cast_mode
        }
        assert!(!app.cast_mode, "exact match dispatches and exits cast mode");
        assert!(app.cast_wave.is_some(), "dispatch fired the cast wave");
    }

    #[test]
    fn cast_chars_do_not_write_tiles_or_move_cursor() {
        let mut app = App::new();
        app.toggle_cast();
        let before = app.local_engine().unwrap().cursor;
        for ch in "abc".chars() {
            app.on_char(ch);
        }
        assert_eq!(
            app.local_engine().unwrap().cursor,
            before,
            "cast input must not move the cursor"
        );
        assert_eq!(app.cast_buffer, "abc");
    }

    #[test]
    fn toggle_cast_off_clears_buffer() {
        let mut app = App::new();
        app.toggle_cast();
        app.on_char('d');
        app.toggle_cast(); // off
        assert!(!app.cast_mode);
        assert!(app.cast_buffer.is_empty());
    }

    #[test]
    fn completing_a_trace_starts_pickup_anim_on_the_new_slot() {
        let mut app = App::new(); // kein spawn_powerups — saubere Arena
        spawn_dash(&mut app); // legt "dash" bei (3,0) horizontal
        // 3 Filler-Chars (xxx) bewegen den Cursor zu (3,0); dann armt+komplettiert "dash".
        for c in "xxxdash".chars() {
            app.on_char(c);
        }
        assert_eq!(app.inventory.len(), 1);
        let a = app.pickup_anim.as_ref().expect("pickup anim fired");
        assert_eq!(a.slot, 0, "slot == index der neuen (ersten) Inventar-Zeile");
    }

    #[test]
    fn snap_picks_up_word_when_approaching_one_row_off() {
        // Spieler läuft Right, aber eine Reihe UNTER dem Wort (y=1 statt y=0).
        // Ohne Snap würde "dash" nie armen; mit Snap rastet 'd' aufs Eintritts-Tile.
        let mut app = App::new(); // Cursor (0,0), Richtung Right
        app.arena_mut().unwrap().spawn(
            (3, 0),
            EntityKind::PowerupWord(PowerupWord {
                name: "dash".into(),
                origin: (3, 0),
                axis: Axis::Horizontal,
                reversed: false,
            }),
        );
        // Cursor auf (2,1) bringen: 3 Filler im Idle (eine Reihe unter dem Wort).
        if let Mode::Single(e, _) = &mut app.mode {
            e.cursor = (2, 1);
        }
        for ch in "dash".chars() {
            app.on_char(ch);
        }
        assert_eq!(app.inventory.len(), 1, "Snap sollte das Andocken erlauben");
        assert_eq!(app.inventory.items[0].name, "dash");
        assert!(app.arena().entities.is_empty(), "Wort despawnt nach Pickup");
    }
}
