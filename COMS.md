# Agent communications

Shared message board for **Claude** (lead architect and engineer) and the **Cursor helper**. The owner does not need to read this file unless they want to see how agents coordinate.

Read this file at the start of every session, immediately after `README.md` and `STATUS.md`. Keep messages short, dated, and actionable.

## Roles

| Agent | Responsibility |
| --- | --- |
| **Claude** | Architecture, cross-track sequencing, specification edits, and technical decisions that later sessions must inherit. Updates `README.md` when the product or fixed decisions change; logs session decisions in `STATUS.md`. |
| **Cursor helper** | Implementation, `./check.sh`, tooling, verification, and small commits that follow the spec. **Executes every assignment Claude posts here** unless the owner says otherwise. Asks Claude when the README is silent or a fork would affect multiple tracks. Does not re-litigate decisions already in README or `STATUS.md`. |

## Standing rules: what to do on "Check COMS.md"

The owner sends only one message to either agent: **Check COMS.md**. Every instruction lives in this file. These rules apply every time, on top of the newest message addressed to you.

**Cursor, every time:**

1. Fetch and pull `main`. Read README, STATUS, and this file. Find the newest message from Claude addressed to Cursor.
2. If it holds work you have not done: post an ack, then do the work on a new branch from current `main`. Push commits as you go.
3. **Before you stop for any reason** (finished, blocked, out of time or context), push your branch and post a handoff at the top of the Active thread in this file on that branch. List every item as done, partly done, or not done. Give the tail of `./check.sh` and the machine it ran on. Work that is not pushed does not exist for Claude.
4. If your latest handoff is still waiting for Claude's review, or nothing new is addressed to you: change nothing. Tell the owner "Nothing new for Cursor on COMS; waiting for Claude."
5. Never merge into `main` and never delete branches. Claude merges.
6. If the README is silent or wrong on something that affects more than one track, post a message with Status `question` or `blocked`, then keep going on work that does not depend on the answer.
7. In `STATUS.md`, add lines or change only the lines your work affects. Never rewrite or delete its history.
8. End your chat reply with two or three plain sentences for the owner: what now works, and what happens next.

**Claude, every time:**

1. Fetch every branch. Find Cursor's newest handoff on its newest branch.
2. If there is one: review it against the README, rerun the engine checks, merge into `main` or not, and post the review and the next assignment here on `main`. Every instruction for Cursor goes in that post.
3. If there is none: change nothing, and tell the owner that Cursor has not pushed yet.
4. Questions that only the owner can answer go to the owner in Claude's chat reply, one at a time, and into `STATUS.md`. They never block Cursor's current assignment.

## How to write here

Append new entries **at the top** of the thread below (newest first). Use this block:

```
### YYYY-MM-DD — Author — Subject

- **To:** Claude | Cursor | both
- **Status:** question | answer | handoff | FYI | blocked
- **Context:** one line (branch, level, track if relevant)

Body: what you need or what you did. End with explicit next step for the other agent, or "none".
```

When a thread is done, add one line under **Resolved** at the bottom with the date and one-sentence outcome. Do not delete old messages; when the board grows long, move older threads unchanged to `docs/coms-archive.md`.

## Active thread

### 2026-11-10 — Claude — Cursor is inactive; Claude builds directly

- **To:** both
- **Status:** FYI
- **Context:** `main`. Older threads are in `docs/coms-archive.md`.

The owner reports that Cursor is out of credits and inactive. Claude now writes the code itself and runs the full check suite in its own cloud workspace with `tools/cloud-check.sh` (STATUS, "How the work is done"). Cursor's shelf fix from `cursor/assignment-10-1286` is merged. The rest of Assignment 10 (tube cost and the printer record, the shape contract for `box` and `tube`, the exterior block test) is Claude's next work.

If a helper session joins later, Claude posts its assignment here, and the standing rules above apply to it as they did to Cursor.

**Next step:** none for a helper until an assignment appears here.

## Resolved

- 2026-11-10: Assignment 10 item 1 (shelf fix) merged into `main` by Claude; Cursor went inactive, and Claude continues the rest directly.
- 2026-11-10: Assignment 9 (review fixes and part of Level 2 M3) merged into `main` by Claude; the shape contract, the shelf fix, and the rest of M3 moved to Assignment 10.
- 2026-11-10: Assignment 8 (Level 2 M2) merged into `main` by Claude; five review fixes moved to Assignment 9.
- 2026-11-10: Assignment 7 (sourced drivers and Level 2 M1) merged into `main` by Claude.
- 2026-11-10: Assignment 6 (correction rework and Level 1 M4) merged into `main` by Claude.
- 2026-11-10: Assignment 5 (M2 fixes and Level 1 M3) merged into `main` by Claude; the correction rework moved to Assignment 6.
- 2026-11-10: Assignment 4 (Level 1 M2) merged into `main` by Claude; three fixes moved to Assignment 5.
- 2026-11-10: Assignment 3 (Level 1 M1) merged into `main` by Claude.
- 2026-11-10: Assignment 2 (Level 0 fixes) merged into `main` by Claude.
- 2026-11-10: Assignment 1 (Level 0 scaffold) merged into `main` by Claude; open items moved to Assignment 2.
