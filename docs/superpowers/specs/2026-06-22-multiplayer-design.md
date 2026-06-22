# Spec: Lokaler-Netzwerk-Multiplayer mit eigenfarbigen Buchstaben-Spuren

- **Issue:** [#25](https://github.com/bardimardy/prfh/issues/25)
- **Datum:** 2026-06-22
- **Status:** Design freigegeben, bereit für Implementierungsplan

## Ziel

Mehrere Spieler im selben LAN bewegen sich gleichzeitig über ein gemeinsames
Spielfeld und ziehen je eine **eigenfarbige** Buchstaben-Spur hinter sich her.
Jeder sieht die anderen in Echtzeit, eingefärbt nach Spieler. Dies ist das
**Netzwerk-Fundament** auf der bestehenden Base-Typing-Mechanik
(`src/game/writing.rs`) — echtes Gameplay (Kollision, PvP, Ziele) kommt als
eigene Folge-Issues *darauf*.

## Nicht-Ziele (bewusst außerhalb des MVP)

- Keine Kollision / kein PvP / keine spielmechanische Interaktion zwischen Spielern.
- Keine client-seitige Prädiktion (im LAN nicht spürbar nötig, siehe unten).
- Keine Reconnect-/Resync-Logik nach Verbindungsabbruch.
- Kein Internet-Play / NAT-Traversal — ausschließlich ein LAN-Segment.

## Kern-Entscheidungen

| Frage | Entscheidung | Begründung |
|---|---|---|
| Topologie | **Host-Client** | Ein Spieler hostet (`prfh host`), Rest joint. Einfachste robuste LAN-Coop-Variante; Host ist natürlicher Autoritäts- und Farbvergabe-Punkt. |
| Transport | **TCP** | Tastenanschläge sind diskrete, reihenfolge-kritische Events in Tipp-Geschwindigkeit — TCPs Zuverlässigkeit/Ordnung passt; UDP wäre unnötige Eigenbau-Komplexität. |
| Sync-Modell | **Autoritativer Host** | Host simuliert alle Engines, Clients senden Input und rendern Zustand. Single Source of Truth, konsistent. |
| Latenz | **Keine Prädiktion** | LAN-Round-Trip ~1–5 ms liegt unter einem 16-ms-Frame und damit unter der Tipp-Reaktionszeit → eigene Spur wirkt sofort. Architektur lässt Prädiktion später nachrüsten. |
| Discovery | **UDP-Broadcast** + manuelle IP als Fallback | Dependency-frei (`std::net::UdpSocket`), genügt im LAN-Segment. `mdns-sd` wäre „proper", aber Dependency-Zuwachs fürs MVP nicht gerechtfertigt. |
| Welt-Semantik | **Ko-Präsenz, keine Kollision** | Gemeinsamer Koordinatenraum + Ursprung; jede Kamera auf eigenen Cursor zentriert; fremde Spuren überlagern (zuletzt geschrieben gewinnt optisch), aber keine Kollision. |
| Farbvergabe | **Host vergibt aus fester Palette** | Deterministisch, konfliktfrei, null UI. Farbe wird bei Disconnect wieder frei. |
| Max. Spieler | **6** | Größe der gut unterscheidbaren Terminal-Farbpalette. |
| Dependencies | **Keine neuen** | `std::net` + `std::thread` + `std::sync::mpsc`, Serialisierung über vorhandenes `serde`+`ron`. Kein tokio/clap. Passt zu CLAUDE.md. |

## Architektur

### Autoritatives Modell

- **Host** hält eine `WritingEngine` **pro Spieler** (inkl. sich selbst), jeweils
  mit zugewiesener Farbe. Der Host ist die einzige Instanz, die Trigger-Logik rechnet.
- **Clients** sind dünn: erfassen lokale Tastenanschläge → senden sie an den Host;
  empfangen Zustands-Deltas → pflegen ein reines Render-Modell und zeichnen es.
- Der **Host-eigene Spieler** speist seinen Input direkt in seine lokale Engine
  (kein Netzwerk-Umweg).

### Render-Entkopplung (zentraler Code-Umbau)

Neue Struktur **`WorldView`** ist die *einzige* Eingabe fürs Rendering. Das
Rendering hängt damit **nie** von Engines oder vom Netzwerk ab.

```rust
// src/game/world.rs
pub struct PlayerView {
    pub id: PlayerId,
    pub color: PlayerColor,   // Basis-RGB des Spielers
    pub name: String,
    pub trail: Vec<Tile>,     // wiederverwendet aus writing.rs
    pub cursor: (i32, i32),
    pub direction: Direction,
    pub is_self: bool,
}

pub struct WorldView {
    pub players: Vec<PlayerView>,
    pub self_id: PlayerId,
}
```

Drei Wege füllen `WorldView`:

- **Single-Player** (Default-Modus): 1-Eintrag-`WorldView` aus der einen Engine,
  jeden Frame gebaut. Bestehendes Verhalten bleibt erhalten.
- **Host**: N-Einträge, aus den Engines des Hosts gebaut.
- **Client**: `WorldView` wird inkrementell aus empfangenen Deltas gepflegt.

**Fade & Farbe:** Die bisherige Grau-Fade-Rechnung (`Rgb(b,b,b)`) wird auf den
Spieler-Farbton verallgemeinert: Basis-RGB des Spielers × Helligkeitsfaktor.
Eigener Cursor bleibt der gelbe Marker; fremde Cursor = Richtungspfeil in ihrer
Farbe. Glow (Trigger) bleibt hervorgehoben.

### Threading & Loop-Integration

Die Render-/Input-Schleife bleibt single-threaded, bekommt aber einen
**Netzwerk-Thread** daneben, verbunden über zwei `mpsc`-Kanäle
(eingehend / ausgehend). Pro Frame:

1. Lokalen Tastatur-Input pollen (non-blocking, wie heute).
2. Eingangskanal vom Netz-Thread leeren und verarbeiten.
3. `WorldView` aktualisieren, zeichnen, `tick()`.

Kein Blockieren auf Netzwerk-I/O in der Render-Schleife.

**Host-Netz-Thread:** besitzt den `TcpListener`; pro akzeptiertem Client ein
Reader-Thread, der geparste Nachrichten in den Eingangskanal schiebt; eine
Writer-Seite liest den Ausgangskanal und fächert Broadcasts an alle
`TcpStream`s. Außerdem läuft der UDP-Broadcast-Announce hier.

**Client-Netz-Thread:** verbindet sich, schiebt eingehende Nachrichten in den
Eingangskanal der Main-Loop, sendet lokale Anschläge aus dem Ausgangskanal an
den Host.

## Protokoll

Wire-Format: **newline-getrenntes, kompaktes RON** über TCP (eine Nachricht pro
Zeile). Reuse der vorhandenen `serde`+`ron`-Dependencies; kompaktes RON enthält
keine eigenen Newlines, daher ist `\n` ein sicheres Frame-Trennzeichen.

```rust
// src/net/protocol.rs
pub enum ClientMsg {
    Hello { name: String },
    Input(InputEvent),       // Char(char) | Backspace
    Bye,
}

pub enum ServerMsg {
    Welcome { your_id: PlayerId, color: PlayerColor, snapshot: Vec<PlayerSnapshot> },
    PlayerJoined { id: PlayerId, color: PlayerColor, name: String },
    PlayerLeft { id: PlayerId },
    Delta { id: PlayerId, tile: Tile, cursor: (i32, i32), direction: Direction },
}
```

- **Late-Join:** Auf `Hello` antwortet der Host mit `Welcome`, das einen
  Voll-`snapshot` aller bestehenden Spuren enthält — Neulinge sehen den Bestand.
- **Delta** wird nach jedem Engine-Schritt eines Spielers an alle Clients
  geschickt (das Tile, das geschrieben wurde, plus neue Cursor-Position und
  Richtung). Backspace erzeugt ebenfalls ein Delta-Äquivalent (entferntes Tile).

## Discovery & CLI

CLI ohne `clap`, manuell aus `std::env::args()` geparst:

| Aufruf | Wirkung |
|---|---|
| `prfh` | Single-Player (Default, unverändertes Verhalten) |
| `prfh host [--name N]` | Hosten; lauscht auf TCP-Port (fest, z.B. 7777) + UDP-Announce |
| `prfh join [ip] [--name N]` | Ohne IP: Lobby-Liste per UDP-Broadcast gefundener Spiele. Mit IP: direkter Connect (Fallback) |

- ⚙️ Fester TCP-Port (z. B. 7777), UDP-Announce-Port (z. B. 7778).
- Namen: Default `P{n}` (n = Beitritts-Reihenfolge), via `--name` überschreibbar.
- Kleine **Roster-Anzeige** im HUD: Farbe + Name jedes verbundenen Spielers.

## Robustheit

- **Disconnect (Client):** Host erkennt TCP-Close → entfernt die Engine, gibt die
  Farbe wieder frei, broadcastet `PlayerLeft`.
- **Disconnect (Host):** Client erkennt geschlossene Verbindung → sauberer Exit
  mit klarer Meldung (kein Hängenbleiben).
- **Trail-Cap:** Spuren werden auf die letzten *N* Tiles begrenzt (begrenzt
  Speicher und Snapshot-Größe). Heute wächst `trail` unbegrenzt. Konkretes *N*
  im Implementierungsplan festzulegen (Größenordnung: einige tausend Tiles).

## Modul-Struktur

Viele kleine, fokussierte Dateien (CLAUDE.md-konform, weniger Merge-Konflikte):

```
src/net/mod.rs        — Modul-Deklaration
src/net/protocol.rs   — Message-Enums + serde, Frame-(De)Serialisierung
src/net/server.rs     — Host: Listener, per-Client-Threads, Engines, Broadcast
src/net/client.rs     — Join: connect, send input, recv state
src/net/discovery.rs  — UDP-Broadcast announce/listen + Lobby-Liste
src/game/world.rs     — WorldView / PlayerView / PlayerColor (Render-Modell)
src/render/mod.rs      — generalisiert auf WorldView + Roster (Umbau)
src/main.rs            — CLI-Dispatch + Netzwerk-Thread/Kanäle (Umbau)
```

## Testbarkeit

- **Protokoll:** Roundtrip-Tests (serialize → deserialize) für alle Message-Typen.
- **WorldView:** Aufbau und Delta-Anwendung unit-testbar ohne Sockets.
- **Engine:** bestehende `WritingEngine`-Tests bleiben unangetastet (grün).
- **Integration:** Host + Client über Loopback (`127.0.0.1`) in einem Test —
  Hello/Welcome/Delta-Fluss verifizieren.
- `cargo build` + `cargo test` müssen warnungs- und fehlerfrei grün bleiben.

## Offene Detailpunkte für den Plan

- Konkretes Trail-Cap *N*.
- Exakte Palette (RGB-Werte der 6 Farben).
- Feste Portnummern.
- Verhalten, wenn das 7. Spieler-Slot (Palette voll) beitreten will → Ablehnung
  mit Meldung.
