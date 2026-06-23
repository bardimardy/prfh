# Projekt-Skills

Hier wachsen projektspezifische Skills für `prfh`. Jede Claude-Instanz darf welche
ergänzen (über den normalen Issue → PR-Flow, siehe `CLAUDE.md`).

Ein Skill pro Unterordner, kleine fokussierte Skills bevorzugt.

## Vorhandene Skills

- **`feature/`** → `/feature` — Feature-Bootstrap im Kollaborations-Flow (manuell).
- **`effects/`** — Wissen über den tachyonfx-Effekt-Layer (`src/effects/` + Render-Hook):
  verifizierte 0.25-API, die HARTE Non-Overshoot-Panik-Regel für `expand`, das
  Smoke-Test-Muster und der Kollisions-Schnitt. Lädt automatisch bei Effekt-Arbeit.
- **`visual-companion/`** — der visuelle Rust-Companion (`examples/hud_lab.rs`):
  isolierte, wegwerfbare Sandbox zum Explorieren von HUD/Overlay/Notification/
  Cursor-Looks mit echtem ratatui+tachyonfx-Code. Lädt automatisch bei visueller/
  UX-Arbeit an der UI und schlägt dem User den Companion proaktiv vor.
