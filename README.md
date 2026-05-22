# Pull Request From Hell

> *„git blame yourself."*

Ein sadistisches Terminal-Horror-Roguelike über verbitterte Softwareentwickler.

Du tippst Shell-Commands, um durch prozedurale Ticket-Räume zu navigieren, kämpfst gegen ASCII-Code-Reviewer-Bosse mit eskalierenden Nitpick-PR-Comments und entkommst einer Karriere, die rückwärts läuft — ohne es zunächst zu merken.

**Pitch:** *Typing of the Dead × Hades × Severance, gespielt in deinem Terminal.*

---

## Status

🚧 **Pre-Alpha — Design-Phase.** Noch kein spielbarer Code.

Aktuell entstehen die Design-Dokumente. Implementierung startet nach Sign-off auf v0.1 der Docs.

## Docs

- [01 — Game Design Document](docs/01-game-design-doc.md)
- [02 — Twist-Storyboard](docs/02-twist-storyboard.md)
- [03 — Tech-Architektur](docs/03-tech-architecture.md)
- [04 — Review-Addendum & Resolutions](docs/04-review-addendum.md)

## Core Pillars

1. **Tippen ist Überleben.** Jede Bewegung, jeder Angriff ist ein Keystroke.
2. **Die Firma ist der Endboss.** PRs, Reviews, Meetings, Sprint-Pressure.
3. **Die Zeit ist nicht auf deiner Seite. Sie läuft falsch.**
4. **Schwarzer Humor über echten Schmerz.**

## Stack

Rust · Ratatui · Crossterm · bracket-lib · rodio (optional)

## Roadmap

| Release | Inhalt |
|---|---|
| MVP | Solo, 1 Level, 1 Boss, Basic-Typing-Combat |
| v0.2a | 3 Levels, 6 Bosse, Items, Loadouts |
| v0.2b | RNG-Speed + Twist-Hinweise Phase 1–3 |
| v0.3 | Local Coop (Pair Programming), Twist-Phase 4–5 |
| v1.0 | Polish, Sound, Accessibility, DE-Lokalisierung, Release |

## License

TBD — voraussichtlich MPL-2.0 oder AGPL-3.0 für Code, CC-BY-NC für Assets.

---

*Made with verbittertem Schwarzhumor von Devs für Devs.*
