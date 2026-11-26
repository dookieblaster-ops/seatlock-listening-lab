# New chat — handoff package

Paste one of the blocks below into a **new** agent chat. The session has no memory of older chats; everything it needs is in the repository.

## Cursor cloud (lead)

```
Work on my GitHub repository dookieblaster-ops/seatlock-listening-lab. Attach it, then Check COMS.md.
```

After the first turn, **Go** continues the newest handoff without re-asking.

## Claude (lead)

```
Work on my GitHub repository dookieblaster-ops/seatlock-listening-lab. Attach it, then Check COMS.md.
```

Same as Cursor. Lead sessions push to `main` (README 1.4).

## What the new session must read first

1. `README.md` — specification (sections 1–4 and 12 always; then the part you touch)
2. `STATUS.md` — what exists, decisions, next steps summary
3. `COMS.md` — newest handoff at the top of the active thread (start here)
4. `docs/wiki/Home.md` — living software map (read articles for parts you change)

`AGENTS.md` loads those three for Claude Code automatically.

## Current queue (2026-11-26)

After **Check COMS.md** / **Go**, the newest handoff says to continue from the first open item:

1. Level 3 optimizer: build method / stock materials (cap discrete combo count) after free-proportions PR merges
2. Optional high-ka lit-gap only if it blocks; Criton leftovers only if a source appears
3. Optional: second backup remote URL / protect `main` (GitHub Pro on this private repo — skip is fine)

Already in on `main` (or open PR): L3 optimizer driver choice (#53) + free proportions (box/sheet); duct-mounted drivers; passive radiators; loft; waveguide loops; FIR; 3D wave views; Dayton exciters; chamber `fill`; printed PLA η = 0.075; soft TPU loss still Irvine 0.10; preference score; BEM CHK-BEM-1…10; Criton / C-Note; living wiki.

## What “done” looks like for a lead turn

- Work the first open queue item in the newest COMS handoff
- Keep `STATUS.md` true
- Post a new handoff at the top of `COMS.md`
- Commit and push `main`
- Reply to the owner in two or three plain sentences (what works, what is next)

Older COMS threads live in `docs/coms-archive.md`.
