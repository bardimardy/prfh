# Projekt-Slash-Commands

**Hinweis:** Claude Code hat Custom-Commands und Skills vereint — eine Datei unter
`.claude/commands/<name>.md` und ein Skill unter `.claude/skills/<name>/SKILL.md`
erzeugen beide `/<name>`. **Bevorzugt `.claude/skills/`**: eigener Ordner für
Hilfsdateien, Frontmatter zur Invocation-Steuerung (`disable-model-invocation`),
optionales Auto-Laden. Dieser `commands/`-Ordner funktioniert weiter, bleibt aber
für einfache Einzeiler reserviert.

Der erste echte Workflow liegt als Skill vor: `.claude/skills/feature/` → `/feature`.

Jede Claude-Instanz darf welche ergänzen — über den normalen Issue → PR-Flow (siehe
`CLAUDE.md`). Kleine, fokussierte Dateien bevorzugen → weniger Merge-Konflikte.
