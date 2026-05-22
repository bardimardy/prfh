# Tech-Architektur
### Pull Request From Hell — v0.1

> *„It works on my terminal."*

---

## 1. Stack-Entscheidungen

| Layer | Wahl | Begründung |
|---|---|---|
| Sprache | **Rust** (stable) | Performance, Cross-Platform, deterministisches Timing für Typing-Latenz |
| TUI-Framework | **Ratatui 0.x** | De-facto-Standard 2026, Immediate-Mode, Diff-Rendering |
| Terminal-Backend | **Crossterm** | Echtes Cross-Platform (Linux/macOS/Windows), Raw-Mode, Mouse, Async-Events |
| Event-Loop | **rat-salsa** | Timer-Events, Async-Application-Events |
| Roguelike-Utils | **bracket-lib** (selective) | FOV, A*, Dice-RNG — nicht das Rendering-Stack |
| RNG | **rand** + Seedable | Reproduzierbare Runs für Debugging |
| Serialization | **serde** + **ron** | RON für Save-Files (lesbar, Rust-native) |
| ASCII-Assets | Statisch (`assets/ascii/*.txt`) | Pre-generated mit `chafa` + manuelles Cleanup |
| CI/Build | **cargo** + GitHub Actions | Cross-Compile via `cross` |

---

## 2. Hochlevel-Architektur

```
┌─────────────────────────────────────────────────────────┐
│                     main.rs                             │
│  - Terminal Setup (Crossterm Raw-Mode)                  │
│  - App-Bootstrap                                        │
└─────────────────────┬───────────────────────────────────┘
                      │
        ┌─────────────┴─────────────┐
        │       app::App            │
        │  - State Machine          │
        │  - Scene-Stack            │
        └─────┬───────────┬─────────┘
              │           │
   ┌──────────┘           └──────────┐
   ▼                                 ▼
┌──────────────────┐         ┌─────────────────────┐
│   input::        │         │   render::          │
│   - KeyEvents    │         │   - Ratatui Frame   │
│   - Routing      │         │   - Scene-Renderer  │
│     (Driver/Nav) │         │   - ASCII-Sprites   │
└──────┬───────────┘         └─────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│                    game::                                │
│  - World (Grid, Rooms, Entities)                         │
│  - Combat (Typing-Engine)                                │
│  - Shell (Command-Parser)                                │
│  - Time (RNG-Speed-Ticker, Reverse-State)                │
│  - Items / Inventory                                     │
│  - AI (Enemy-Spawner, Boss-Pattern)                      │
└──────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│                  persist::                               │
│  - Save/Load (RON-Files in $XDG_DATA_HOME)               │
│  - Meta-Progress („LinkedIn"-Profile)                    │
│  - Run-Telemetry (für Balancing, opt-in)                 │
└──────────────────────────────────────────────────────────┘
```

---

## 3. Module-Layout

```
pull-request-from-hell/
├── Cargo.toml
├── README.md
├── assets/
│   ├── ascii/
│   │   ├── bosses/                # ASCII-Boss-Frames
│   │   ├── ui/                    # Logo, Borders, Decorations
│   │   └── death/                 # Death-Screen-Varianten
│   ├── words/
│   │   ├── nitpicks.txt           # Code-Review-Comment-Pool
│   │   ├── code_snippets.txt      # Combat-Words
│   │   └── lore/                  # Slack-Logs, Commits
│   └── boss_scripts/              # RON: Boss-Phasen + Patterns
├── src/
│   ├── main.rs
│   ├── app.rs                     # State Machine, Scene-Stack
│   ├── input/
│   │   ├── mod.rs
│   │   ├── router.rs              # Solo vs. Coop-Routing
│   │   └── coop.rs                # Driver/Navigator-Split
│   ├── render/
│   │   ├── mod.rs
│   │   ├── grid.rs                # World-Grid-Renderer
│   │   ├── combat.rs              # Typing-Combat-View
│   │   ├── boss.rs                # ASCII-Boss-Renderer (Frame-Sequenz)
│   │   ├── hud.rs                 # Bug-Counter, Combo, „Innocence"
│   │   └── shell.rs               # Shell-Prompt-Renderer
│   ├── game/
│   │   ├── mod.rs
│   │   ├── world.rs               # Grid-Generation, Räume
│   │   ├── procgen.rs             # Procedural Level-Generator
│   │   ├── shell.rs               # Command-Parser (`cd`, `ls`, `grep`...)
│   │   ├── combat.rs              # Typing-Engine, Combo-System
│   │   ├── time.rs                # RNG-Speed-Ticker, Reverse-Flags
│   │   ├── items.rs               # Loot, Inventar
│   │   ├── ai/
│   │   │   ├── enemy.rs
│   │   │   └── boss.rs            # Phase-Machine
│   │   └── champions.rs           # Backend/Frontend/DevOps
│   ├── persist/
│   │   ├── mod.rs
│   │   ├── save.rs
│   │   └── meta.rs                # Meta-Progress „LinkedIn"
│   └── coop/
│       ├── mod.rs
│       ├── socket.rs              # TCP-Coop (cross-platform)
│       └── ipc.rs                 # Lokal-Coop via UDS
└── tests/
    ├── shell_parser.rs
    ├── combat_typing.rs
    └── procgen_smoke.rs
```

---

## 4. Render-Pipeline

**Immediate-Mode-Loop, 60 FPS Target, adaptive Frame-Drop:**

```rust
loop {
    // 1. Drain Input (non-blocking)
    while let Some(ev) = crossterm::event::poll(Duration::ZERO)? {
        app.handle_input(ev);
    }
    
    // 2. Tick Game-Logic (uses RNG-modulated dt)
    let dt = time_ticker.next_dt();
    app.tick(dt);
    
    // 3. Render Frame (Ratatui Diff-Render)
    terminal.draw(|f| app.render(f))?;
    
    // 4. Sleep to target FPS
    spin_sleep::sleep(frame_budget - elapsed);
}
```

**Performance-Constraints:**
- Boss-ASCII-Frames sind pre-rasterized in `Vec<Vec<Cell>>` (kein String-Parse pro Frame)
- Diff-Rendering aus Ratatui übernimmt Redraw-Optimierung
- Grid-Render nur bei State-Change neu berechnen (Dirty-Flag)

---

## 5. Input-Architektur (inkl. Coop)

**Solo:** Standard Crossterm-Event-Stream → AppHandler.

**Coop (revidiert nach Review):**

`evdev` ist Linux-only — daher umgeschwenkt auf **„Co-located Coop via tmux-Socket"**:

| Option | Wann | Wie |
|---|---|---|
| **A: Tmux Split** | Bevorzugt | 2 Player starten je eine Instanz, joinen via `--coop-socket /tmp/prfh.sock`, eine Instanz ist Host, beide rendern unabhängig, synchronisierter State via Unix-Socket |
| **B: Same Terminal** | Fallback | Eine Instanz, geteiltes Terminal, Tastatur-Range-Split (links: Driver, rechts: Navigator) — als „Couch-Mode" labeln |
| **C: TCP-Coop** | Stretch | Selbe Architektur wie A, aber via TCP — ermöglicht LAN-Coop |

**Entscheidung MVP:** Solo zuerst. MP in v0.3 via Option A (tmux/UDS). Option B ist Notlösung für „echtes Couch-Coop"-Feeling.

---

## 6. State Machine

```
                ┌──────────┐
                │  Boot    │ (Loading „career...")
                └────┬─────┘
                     ▼
                ┌──────────┐
                │  Menu    │
                └────┬─────┘
                     ▼
                ┌──────────┐
                │  Day0    │ (Tutorial — diegetisch)
                └────┬─────┘
                     ▼
        ┌──────────────────────┐
        │      RunActive       │◄─────┐
        │  ┌────────────────┐  │      │
        │  │  Grid          │  │      │
        │  │  ↕             │  │      │
        │  │  Combat        │  │      │
        │  │  ↕             │  │      │
        │  │  BossFight     │  │      │
        │  │  ↕             │  │      │
        │  │  Retro (Loot)  │  │      │
        │  └────────────────┘  │      │
        └────────┬─────────────┘      │
                 │                    │
            ┌────┴─────┐              │
            ▼          ▼              │
        ┌──────┐   ┌──────────────┐   │
        │Death │   │ FinalReveal  │   │
        └──┬───┘   └──────┬───────┘   │
           │              │           │
           ▼              ▼           │
        ┌──────────┐   ┌──────────┐   │
        │ MetaProg │   │ Credits  │   │
        └────┬─────┘   └──────────┘   │
             └────────────────────────┘
```

---

## 6.5 Write-to-Move Engine (Primary-Mode)

**Kern-Modul:** `game::writing` — verarbeitet jeden Keystroke als Schritt + Zeichen.

```rust
pub struct WritingEngine {
    pub cursor: (i32, i32),        // World-Grid-Position
    pub direction: Direction,       // Up/Down/Left/Right
    pub trail: Vec<Tile>,           // History (Pos, Char, Style)
    pub current_word: String,       // Buffer für Trigger-Detection
    pub combo: u32,
    pub doubt: u32,
}

pub enum Direction { Up, Down, Left, Right }

impl WritingEngine {
    pub fn on_char(&mut self, c: char) -> StepResult {
        // 1. Word-Boundary erkennen (Space/Punkt/EOF)
        // 2. Trigger-Word check (up/down/left/right/back/stop)
        // 3. Position advancen entlang aktueller Direction
        // 4. Collision-Check (Enemies, Items, Walls)
        // 5. Trail-Update
    }
}
```

**Trigger-Detection:** State-Machine, die `current_word` akkumuliert und beim Boundary-Token prüft, ob ein Direction-Word matched. Match = `direction` wird nach dem Boundary-Char gewechselt.

**Tests:** Property-Tests mit `proptest` — beliebige Strings dürfen die Engine nicht in invaliden Zustand bringen.

---

## 7. Shell-Command-Parser (Sub-Mode)

Per `Tab` aktiviert. Mini-DSL für In-Game-Commands. Eigener Parser (nicht `clap`), damit Fehler-Feedback diegetisch ist.

```rust
enum ShellCommand {
    Cd(Target),           // cd north | cd ../bug42
    Ls,
    Cat(Path),            // cat slack.log
    Grep(Pattern),        // grep enemy | grep "TODO:"
    Rm(EntityRef),        // rm bug42
    Sudo(AbilityName),    // sudo --jetpack
    GitStash,             // panic exit
    Help,
    Invalid(String),      // → diegetisch: „command not found"
}

fn parse(input: &str) -> ShellCommand { /* ... */ }
```

**Diegetische Fehler:** Tippt der Spieler `quit`, antwortet das Spiel: *„There is no escape. Try `git stash` (you'll lose progress)."*

---

## 8. Time-System (RNG-Speed + Reversal)

**Zwei separate Konzepte:**

### 8.1 RNG-Speed (taktisches Pacing)

```rust
struct TimeTicker {
    base_dt: Duration,        // 16.6ms (60 FPS)
    current_modifier: f32,    // 0.5 / 1.0 / 1.5 / 2.0
    next_change_at: Instant,
    rng: ChaCha8Rng,
}
```

- Modifier wechselt alle 30–120s zufällig
- **Fairness-Lock:** Während Boss-Phases mit langen Texten ist Modifier auf ≤1.0 gecapped (Review-Fix)
- Optionaler **Surge-Indikator** in Accessibility-Mode

### 8.2 Reverse-State (Narrativ)

```rust
struct CareerState {
    day: i64,                 // startet hoch, sinkt
    bugs_remaining: i64,      // startet hoch, sinkt
    innocence: f32,           // startet niedrig, steigt
    job_title: JobTitle,      // Staff → Senior → Mid → Junior
}
```

Alle UI-Anzeigen ziehen ihre Werte aus diesem State. Die Reverse-Mechanik ist **rein narrativ** — die Game-Logic läuft normal vorwärts, nur die *Darstellung* ist umgekehrt.

---

## 9. Procedural Generation

**Räume = Multi-Floor-Dungeons** (Auflösung des Jetpack-Raum-Problems aus Review):

- Jeder Sprint ist ein **Bürogebäude** (3–5 Etagen)
- Jede Etage ist ein 2D-Grid von Räumen
- **Jetpack** = vertikaler Etagenwechsel (`sudo --jetpack` → up/down)
- **Treppen** auch verfügbar, aber langsamer + bewacht
- `cd up` / `cd down` für vertikale Navigation

Algorithmus: Pro Etage Wave-Function-Collapse-light (Tile-Set: Open-Space, Cubicle, Meeting-Room, Kitchen, Server-Closet).

---

## 10. Save-Format (RON)

```ron
CareerSave(
    player_name: "Nick",
    current_day: 247,            // sinkt!
    runs_completed: 12,
    unlocks: ["jquery.js", "mech_kb"],
    twist_phase: 2,              // versteckt, persistiert
    settings: Settings(
        accessibility_mode: false,
        word_difficulty: Normal,
        rng_speed_indicator: false,
    ),
)
```

Save-Location: `$XDG_DATA_HOME/pull-request-from-hell/career.ron`
Mid-Run-Save: nur zwischen Sprints (kein Save-Scumming).

---

## 11. Testing-Strategie

- **Unit:** Shell-Parser, Combat-Typing-Validator, Procgen-Determinismus
- **Snapshot:** Render-Output gegen Referenz-Frames (`insta`)
- **Integration:** Volle Runs als Headless-Bot mit gescripteter Eingabe
- **Manual Playtest:** Wöchentliche Runs, Twist-Discovery-Telemetrie

---

## 12. Cross-Platform-Matrix

| OS | Status | Notes |
|---|---|---|
| Linux | Primary | dev-env, evdev-experimente für KB-Splitting |
| macOS | Primary | Test-Target, Terminal.app + iTerm2 + Ghostty |
| Windows | Secondary | Windows Terminal only, ConPTY-Latenz dokumentieren |

---

## 13. Build & Distribution

- `cargo install --locked pull-request-from-hell` (crates.io)
- itch.io: Pre-built Binaries für Linux/macOS/Windows
- Single-Binary, keine externen Deps zur Runtime
- Asset-Embedding via `include_bytes!` für Single-Binary-Distribution

---

*Document Status: v0.1 — Initial Draft*
*Dependencies: 01-game-design-doc.md, 02-twist-storyboard.md*
*Next: 04-review-addendum.md*
