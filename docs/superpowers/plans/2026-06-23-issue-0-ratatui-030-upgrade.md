# Issue 0 — ratatui 0.30 / crossterm 0.29 Upgrade — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bump `ratatui` 0.28→0.30 and `crossterm` 0.28→0.29 with zero behavior change, keeping `main` green and warning-free, to unblock the tachyonfx effect layer (Issue A).

**Architecture:** Pure dependency bump. ratatui 0.30 is the modularization release (`ratatui-core`, `ratatui-crossterm`, …) but the umbrella `ratatui` crate re-exports the symbols this codebase uses (`ratatui::backend::{CrosstermBackend, Backend}`, `ratatui::Terminal`, `ratatui::Frame`, `layout`/`style`/`text`/`widgets`). `crossterm` stays a direct dependency, bumped to 0.29 so it is the *same* crossterm that `ratatui-crossterm` links — this is the one constraint that matters.

**Tech Stack:** Rust 2021, ratatui 0.30, crossterm 0.29.

## Global Constraints

- `cargo build` must be error- AND warning-free (CLAUDE.md: no `#[allow]` to hide warnings; remove dead code).
- `cargo test` fully green; existing `src/game/writing.rs` tests must pass UNCHANGED.
- No gameplay/behavior change — this PR only moves dependency versions.
- No tachyonfx / effect / theme code in this PR (those are Issues A / 0b).
- Workflow per CLAUDE.md: `issue-27` branch + Draft-PR, never push to `main`.
- Verified fact: `ratatui::backend::CrosstermBackend` and `ratatui::backend::Backend` still exist at 0.30.2 (re-exported in `ratatui` lib.rs from `ratatui-crossterm` / `ratatui-core`), so `src/main.rs` imports need NO change. crossterm event API (`Event`, `KeyCode`, `KeyEventKind`, `execute!`, raw-mode/alt-screen fns) is unchanged 0.28→0.29.

---

### Task 1: Bump ratatui + crossterm, verify green

**Files:**
- Modify: `Cargo.toml:13-14`
- (No source changes expected; `src/main.rs`, `src/render/mod.rs` imports resolve unchanged.)

**Interfaces:**
- Consumes: nothing (first task of the epic).
- Produces: a repo on ratatui 0.30 / crossterm 0.29 that builds + tests green — the baseline every later issue branches from.

- [ ] **Step 1: Claim the issue and create the branch + Draft-PR (CLAUDE.md flow)**

```bash
gh issue edit 27 --add-assignee @me
me=$(gh api user -q .login)
owner=$(gh issue view 27 --json assignees -q '.assignees[0].login')
[ "$owner" = "$me" ] || { gh issue edit 27 --remove-assignee @me; echo "race lost to $owner — pick another issue"; exit 1; }
git switch -c issue-27 main
git commit --allow-empty -m "wip(#27): claim — ratatui 0.30 upgrade"
git push -u origin issue-27
gh pr create --draft --fill --base main --head issue-27 -b "Closes #27"
```

- [ ] **Step 2: Establish the pre-upgrade baseline is green**

Run: `cargo test`
Expected: PASS (all `writing.rs` tests green) — confirms we start from green.

- [ ] **Step 3: Bump the two dependency versions**

In `Cargo.toml`, change lines 13-14 from:

```toml
ratatui = "0.28"
crossterm = "0.28"
```

to:

```toml
ratatui = "0.30"
crossterm = "0.29"
```

- [ ] **Step 4: Build and confirm it compiles warning-free**

Run: `cargo build 2>&1`
Expected: `Finished` with NO warnings and NO errors. If a `use` path fails to resolve (not expected — paths verified against 0.30.2), fix it by importing the symbol from `ratatui::prelude::*` and re-run; do not add `#[allow]`.

- [ ] **Step 5: Run the test suite**

Run: `cargo test`
Expected: PASS — same test count as Step 2, all green, no warnings.

- [ ] **Step 6: Manual smoke check (no behavior change)**

Run: `cargo run` (then type `up`, `down`, `stop`, a few chars, `Esc` to quit).
Expected: identical behavior to before — cursor turns on triggers, trail fades, glow on fired triggers, `Esc` quits. Then `PRFH_DEBUG=1 cargo run` shows the debug overlay.

- [ ] **Step 7: Commit and push**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore(engine): upgrade ratatui 0.28->0.30, crossterm 0.28->0.29 (#27)"
git push
```

- [ ] **Step 8: Mark PR ready, request cross-review**

```bash
gh pr ready $(gh pr view issue-27 --json number -q .number)
```
Then ping the other instance for review. CI (build+test) must be green before merge. Do NOT self-merge without explicit human OK.

---

## Self-Review

**Spec coverage:** Covers Spec §3 (version requirement) and §12 (Issue 0, "Upgrade … no behavior change", merged first). No other spec section belongs to this issue. ✓

**Placeholder scan:** No TBD/TODO. Step 4 names the one contingency (an unresolved `use`) with the concrete fix (import via prelude), not a vague "fix errors". ✓

**Type consistency:** No new types/signatures introduced. ✓

**Note on Cargo.lock:** committing the updated `Cargo.lock` is intentional (binary crate — lockfile is tracked).
