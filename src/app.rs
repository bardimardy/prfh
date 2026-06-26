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
    /// Engine-`tick` beim Cast-Eintritt — markiert die im Cast geschriebenen
    /// Trail-Tiles fürs render-time-Tinten.
    pub cast_start_tick: Option<u64>,
    /// Alter der laufenden Cast-Welle (render-time-Ring); None = keine Welle.
    pub cast_wave: Option<Duration>,
    /// Monotone Animations-Uhr fürs render-time-Shimmer (vom Render getrieben).
    pub anim_clock: Duration,
    /// Laufende Pickup-Animation (render-time); None = keine Anim aktiv.
    pub pickup_anim: Option<PickupAnim>,
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
            cast_start_tick: None,
            cast_wave: None,
            anim_clock: Duration::ZERO,
            pickup_anim: None,
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
            cast_start_tick: None,
            cast_wave: None,
            anim_clock: Duration::ZERO,
            pickup_anim: None,
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

    /// Cast-Modus betreten/verlassen (Default-Taste `Tab`). Buffer wird geleert;
    /// `cast_start_tick` merkt sich beim Eintritt den Engine-`tick`, damit der
    /// Render die im Cast geschriebenen Trail-Tiles tinten kann.
    pub fn toggle_cast(&mut self) {
        self.cast_mode = !self.cast_mode;
        self.cast_buffer.clear();
        self.cast_start_tick = if self.cast_mode {
            self.local_engine().map(|e| e.tick)
        } else {
            None
        };
    }

    /// Debug-Overlay ein-/ausblenden (Default-Taste `F1`). Default: versteckt,
    /// unabhängig von `PRFH_DEBUG` (der Env-Var steuert nur das Sammeln der
    /// Log-Zeilen, nicht die Sichtbarkeit).
    pub fn toggle_debug(&mut self) {
        self.debug = !self.debug;
    }

    /// Zeichen im Cast-Modus: lenkneutral ins Trail schreiben (kein Bewegungs-
    /// Trigger, aber Tile + Cursor laufen wie beim normalen Schreiben — Cast
    /// „passiert im Trail", #44). Bei exaktem Inventar-Namen → Dispatch + Modus
    /// verlassen.
    fn on_cast_char(&mut self, c: char) {
        // Auto-Abort: sobald `cast_buffer + c` kein Präfix mehr eines
        // Inventar-Worts ist, ist kein exakter Match je wieder erreichbar
        // (subsumiert „Buffer länger als längster Name"). Cast verlassen und das
        // Zeichen normal durch `on_char` schicken — inkl. Bewegungs-Triggern, ein
        // nahtloser Übergang zurück ins normale Schreiben. Leeres Inventar matcht
        // nie → der erste Cast-Char droppt sofort zurück.
        let mut candidate = self.cast_buffer.clone();
        candidate.push(c);
        if self.inventory.prefix_matches(&candidate).is_empty() {
            self.cast_mode = false;
            self.cast_buffer.clear();
            self.cast_start_tick = None;
            self.notifications
                .push(NotifyKind::Info, "✗  no spell", candidate);
            self.on_char(c);
            return;
        }

        if let Mode::Single(e, _) = &mut self.mode {
            e.trace_suspended = true;
            e.on_char(c);
        }
        self.cast_buffer.push(c);
        if let Some(p) = self.inventory.get_exact(&self.cast_buffer).cloned() {
            self.dispatch_cast(p.effect_tag, &p.name);
            self.cast_mode = false;
            self.cast_buffer.clear();
            self.cast_start_tick = None;
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
            EffectTag::Dash => {
                self.notifications
                    .push(NotifyKind::Event, "⚡  DASH", name.to_string());
                self.debug_log(format!("cast dispatch: {name} ({tag:?}) — Aim-Mode folgt in Task 2"));
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
                let effect_tag = crate::game::skill::skill_def(&name)
                    .map(|d| d.effect_tag.clone())
                    .unwrap_or(EffectTag::Test);
                self.inventory.add(Powerup {
                    id,
                    name: name.clone(),
                    effect_tag,
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
        if self.cast_mode {
            self.cast_buffer.pop();
        }
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

    #[test]
    fn toggle_debug_flips_visibility() {
        let mut app = App::new_single();
        assert!(!app.debug, "Debug-Overlay startet versteckt");
        app.toggle_debug();
        assert!(app.debug, "F1 zeigt das Overlay");
        app.toggle_debug();
        assert!(!app.debug, "F1 erneut versteckt es wieder");
    }
}

#[cfg(test)]
mod w2_tests {
    use super::*;
    use crate::game::arena::EntityKind;
    use crate::game::powerup::{Axis, EffectTag, PowerupWord};

    fn add_inv(app: &mut App, name: &str) {
        app.inventory.add(Powerup {
            id: 0,
            name: name.into(),
            effect_tag: EffectTag::Test,
        });
    }

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
    fn cast_aborts_when_prefix_no_longer_matches() {
        // Inventar nur "dash". 'u' kann nie Präfix werden → Auto-Abort, und das
        // 'u' läuft normal durch on_char; ein folgendes 'p' lässt dann "up" als
        // Bewegungs-Trigger feuern (nahtloser Übergang in den Normalmodus).
        use crate::game::writing::Direction;
        let mut app = App::new();
        add_inv(&mut app, "dash");
        app.toggle_cast();
        app.on_char('u'); // bricht Cast ab, schreibt 'u' normal
        assert!(!app.cast_mode, "no possible prefix → cast aborts");
        assert!(app.cast_buffer.is_empty());
        app.on_char('p'); // "up" feuert jetzt regulär
        assert_eq!(
            app.local_engine().unwrap().direction,
            Direction::Up,
            "aborted char is processed normally, so triggers fire again"
        );
    }

    #[test]
    fn cast_with_empty_inventory_drops_on_first_char() {
        // Leeres Inventar: prefix_matches ist immer leer → der erste Cast-Char
        // droppt sofort zurück in den Normalmodus.
        let mut app = App::new();
        app.toggle_cast();
        assert!(app.cast_mode);
        app.on_char('d');
        assert!(!app.cast_mode, "empty inventory → first cast char drops out");
        assert!(app.cast_buffer.is_empty());
    }

    #[test]
    fn cast_keeps_running_while_buffer_is_a_valid_prefix() {
        let mut app = App::new();
        add_inv(&mut app, "dash");
        app.toggle_cast();
        app.on_char('d');
        app.on_char('a');
        assert!(app.cast_mode, "valid prefix keeps cast mode alive");
        assert_eq!(app.cast_buffer, "da");
    }

    #[test]
    fn cast_chars_write_into_trail_and_move_cursor() {
        // Cast „passiert im Trail" (#44): Tile + Cursor laufen wie beim normalen
        // Schreiben, aber lenkneutral (kein Bewegungs-Trigger).
        use crate::game::writing::Direction;
        let mut app = App::new(); // player at (0,0), Right
        add_inv(&mut app, "abcde"); // "abc" bleibt Präfix → kein Auto-Abort
        app.toggle_cast();
        for ch in "abc".chars() {
            app.on_char(ch);
        }
        let e = app.local_engine().unwrap();
        assert_eq!(e.cursor, (3, 0), "cast input advances the cursor");
        assert_eq!(e.trail.len(), 3, "cast input writes trail tiles");
        assert_eq!(e.direction, Direction::Right, "cast stays steering-neutral");
        assert_eq!(app.cast_buffer, "abc");
    }

    #[test]
    fn cast_does_not_fire_movement_triggers() {
        // „up" wäre normal ein Turn — im Cast lenkneutral (trace_suspended).
        use crate::game::writing::Direction;
        let mut app = App::new();
        add_inv(&mut app, "upgrade"); // "up" bleibt Präfix → kein Auto-Abort
        app.toggle_cast();
        for ch in "up".chars() {
            app.on_char(ch);
        }
        let e = app.local_engine().unwrap();
        assert_eq!(e.direction, Direction::Right, "cast must not steer");
        assert_eq!(e.trail.len(), 2);
        assert_eq!(app.cast_buffer, "up");
    }

    #[test]
    fn cast_backspace_corrects_buffer_and_trail() {
        let mut app = App::new();
        add_inv(&mut app, "daxyz"); // "dax" bleibt Präfix → kein Dispatch, kein Abort
        app.toggle_cast();
        for ch in "dax".chars() {
            app.on_char(ch);
        }
        app.on_backspace();
        assert_eq!(app.cast_buffer, "da");
        let e = app.local_engine().unwrap();
        assert_eq!(e.cursor, (2, 0), "backspace walks the cursor back one tile");
        assert_eq!(e.trail.len(), 2, "backspace erases one trail tile");
    }

    #[test]
    fn toggle_cast_off_clears_buffer() {
        let mut app = App::new();
        add_inv(&mut app, "dash"); // "d" bleibt Präfix → kein Auto-Abort
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

    #[test]
    fn picking_up_dash_stores_the_dash_effect_tag() {
        let mut app = App::new();
        spawn_dash(&mut app);
        for ch in "xxxdash".chars() {
            app.on_char(ch);
        }
        assert_eq!(app.inventory.items[0].effect_tag, EffectTag::Dash);
    }
}
