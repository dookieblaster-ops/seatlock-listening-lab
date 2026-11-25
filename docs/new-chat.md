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

## Current queue (2026-11-25)

After **Check COMS.md** / **Go**, the newest handoff says to continue from the first open item:

1. Owner: fix GitHub Actions billing / spending limit, **or** say the lead may merge on local check → merge **PR #33** (lead review + local re-verify already green)
2. Soft TPU / foamed-filament loss only with a filament-specific DMA or tap test (skip if no source); optional high-ka lit-gap only if it blocks
3. Criton leftovers only if a source appears
4. Optional: second backup remote URL / protect `main` (needs GitHub Pro on this private repo — skip is fine)

Already in on PR #33 (awaiting merge): wow-pass on 3D sound views (inside / outside / room). On `main` already: printed PLA η = 0.075 (Rothe 2021); varioShore Young’s 40 MPa (maker TDS); passive R/L/C tune, CTA-2034 Olive preference as the ranked score, preference scoring speedup, budget 8 in woofers, non-box page fields, BEM scaffold (CHK-BEM-1…10; true open mouths; mouth `Z_rad` matrix joined to waveguide including mutuals; page offer of `bem`; high-ka lit gap not geometry-only; re-entrant occupancy remesh) and `bem` radiation on the true outer shape (AABB seed or remesh), multi-source generator harden; C-Note MT and Criton 3TD-X vs ASR; living wiki under `docs/wiki/`.

## What “done” looks like for a lead turn

- Work the first open queue item in the newest COMS handoff
- Keep `STATUS.md` true
- Post a new handoff at the top of `COMS.md`
- Commit and push `main`
- Reply to the owner in two or three plain sentences (what works, what is next)

Older COMS threads live in `docs/coms-archive.md`.
