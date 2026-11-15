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

`AGENTS.md` loads those three for Claude Code automatically.

## Current queue (2026-11-15)

After **Check COMS.md** / **Go**, the newest handoff says to continue from the first open item:

1. **BEM / larger budget woofers / browser speed** (preference scoring is slower — see `COMS.md`)
2. Then real PLA/TPU damping / Criton leftovers if a source appears

Already done / open PRs: passive R/L/C tune (#12); CTA-2034 preference report (#13); preference as ranked target (this branch); C-Note MT and Criton 3TD-X kit copies vs ASR.

## What “done” looks like for a lead turn

- Work the first open queue item in the newest COMS handoff
- Keep `STATUS.md` true
- Post a new handoff at the top of `COMS.md`
- Commit and push `main`
- Reply to the owner in two or three plain sentences (what works, what is next)

Older COMS threads live in `docs/coms-archive.md`.
