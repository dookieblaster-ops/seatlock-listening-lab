# Agent communications

The handoff board between sessions (README section 1.4). Every chat starts with no memory of earlier chats, so whatever the next session needs is written here. The owner does not need to read this file.

Read this file at the start of every session, immediately after `README.md` and `STATUS.md`. Keep messages short, dated, and actionable.

## Standing rules: what to do on "Check COMS.md"

The owner starts a session by typing **Check COMS.md** (or **Go** after that). Every instruction lives in this file. The paste for a brand-new chat is in `docs/new-chat.md`.

**Lead session (Claude or Cursor cloud), every time:**

1. Fetch every branch and pull `main`. A fresh clone may fetch `main` only, so first run `git config remote.origin.fetch '+refs/heads/*:refs/remotes/origin/*'`, then `git fetch origin`.
2. If a helper's handoff is waiting for review on its branch: review it against the README, rerun the checks, merge it into `main` or not, and post the review here.
3. Otherwise take the newest lead handoff at the top of the active thread and continue its work queue from the first item not done. Do not ask the owner whether to continue. Ask only the product questions that handoff names, when they start to matter.
4. Before stopping for any reason: make `STATUS.md` true, post a new handoff at the top of the active thread (what is done, what is not, the next queue, the tail of the checks), commit, and push to `main`.
5. Questions only the owner can answer go in the chat reply, one at a time with a recommendation, and into `STATUS.md`.

**Helper, every time** (only when the lead has posted an assignment for one):

1. Fetch and pull `main`. Read README, STATUS, and this file. Find the newest assignment addressed to you.
2. If it holds work you have not done: post an ack, then work on a new branch from current `main`. Push commits as you go.
3. Before you stop for any reason, push your branch and post a handoff at the top of the active thread on that branch. List every item as done, partly done, or not done. Give the tail of the checks and the machine they ran on.
4. If your handoff is waiting for review, or nothing new is addressed to you: change nothing, and tell the owner "Nothing new for the helper; waiting for the lead."
5. Never merge into `main` and never delete branches. The lead merges.
6. If the README is silent or wrong on something that affects more than one track, post a message with Status `question` or `blocked`, then keep going on work that does not depend on the answer.
7. In `STATUS.md`, add lines or change only the lines your work affects. Never rewrite or delete its history.
8. End your chat reply with two or three plain sentences for the owner: what now works, and what happens next.

## How to write here

Append new entries **at the top** of the thread below (newest first). Use this block:

```
### YYYY-MM-DD — Author — Subject

- **To:** next lead session | helper | both
- **Status:** question | answer | handoff | FYI | blocked
- **Context:** one line (branch, level, track if relevant)

Body: what you need or what you did. End with explicit next step for the other agent, or "none".
```

When a thread is done, add one line under **Resolved** at the bottom with the date and one-sentence outcome. Do not delete old messages; when the board grows long, move older threads unchanged to `docs/coms-archive.md`.

## Active thread

### 2026-11-15 — Cursor — Handoff: start here. Passive page editor next

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`. Level 3. Passive listen-mode landed this session.

**Where the product stands (one screen):** Levels 0–2 complete. Level 3 in progress. `dsp.passive` drives graphs, scores, and listen mode (network in the convolver impulses; leftover digital radiator blocks not played; filter text says loads are frozen at sim Z). Kit copy `designs/imports/c-note/` vs ASR. No page netlist editor; no R/L/C optimizer yet.

**Done this session:** (1) `listen_package` / `impulse_responses`: keep `dsp.passive` in the acoustic IR; skip per-radiator digital blocks in worklet channels when passive is set; label frozen loads. (2) Test `listen_impulses_carry_the_passive_network` on the first-order two-way fixture. (3) README §7 / §10 / Level 3 F wording.

**Open, in order (do the first without a new owner answer):**

1. **Passive page editor** for the netlist (add/edit R/L/C/driver nodes).
2. **Optimize R/L/C** toward a score (Level 3 optimizer track).
3. **Research plan (README 2.6):** CTA-2034 preference rating as the lab's score; BEM; multi-source generator harden; larger budget woofers; browser speed; non-box page fields.
4. **Wall remainder:** real PLA/TPU damping figures (tap test / DMA — do not invent). Imported mesh flats, MDF/plywood, braces, partitions already vibrate.
5. **Optional:** 1/2 in MDF stock for C-Note; dimensioned baffle holes; inductor DCR; shrink LCM 3D self-check; partition coupling inside the waveguide tree.

**Owner questions waiting (ask only when they start to matter):** 2 (playback hardware), 6 (panel material, wall distance, in-room tilt), 8 (depth-scanning phone). Q11 and Q12 answered — do not re-ask. Do not undo Q12. Do not digitize FR graphs.

**Pitfalls:** when `passive` is set, digital blocks on radiators are ignored for graphs and for listen playback (label says so). Amp is an ideal voltage source. No mutual inductance / transformers. Listen impulses freeze Z(f) from the sim; they do not re-solve the network per audio block. Cloud check: `tools/cloud-check.sh`. Stable Rust ~1.100.

**Last check:** `cargo test -p seatlock-sim --test passive`: 3 passed. `listen_package` check ok. fmt; clippy `-D warnings` on sim/acoustics. Full `tools/cloud-check.sh` not re-run.

**Next step:** item 1 — passive page netlist editor.

## Resolved

- 2026-11-15: Passive listen-mode (network in listen impulses; digital radiator blocks skipped), done on `main` by Cursor.
- 2026-11-15: C-Note MT kit copy with `dsp.passive` vs ASR CTA-2034 (mid/treble ~2 dB RMS), done on `main` by Cursor (PR #11).
- 2026-11-15: Passive R/L/C crossover for graphs and scores (`dsp.passive`), done on `main` by Cursor; editor/optimize still open.
- 2026-11-15: Imported mesh cabinets' axis-aligned flat walls vibrate (thickness measured from the solid), done on `main` by Cursor.
- 2026-11-15: Board archived to `docs/coms-archive.md` for a fresh lead session; handoff package in `docs/new-chat.md`.
- 2026-11-15: Partitions between chambers vibrate (mutual admittance in lumped network), done on `main` by Cursor.
- 2026-11-15: Brace solids take air/print and split wall spans, done on `main` by Cursor.
- 2026-11-15: MDF/plywood sheet boxes vibrate (Wood Handbook figures); LCM leftover tilt cause placed; Q12 stands, done on `main` by Cursor.
- 2026-11-15: LCM 3D self-check 249 % traced to mouth-area PortSpec and total-volume Helmholtz compare; fixed to ~25 % at 5 mm, done on `main` by Cursor.
- 2026-11-15: STATUS question 12 answered yes; independent acoustic sensitivity scales BMR piston level while Vas keeps compliance, done on `main` by Cursor.
- 2026-11-14: Erin's LCM CTA-2034 curves kept under CC BY-NC-SA; bass-to-midrange step placed as BMR vs woofer level, done on `main` by Cursor.
- 2026-11-14: Full check completed on the LCM commit; Clippy for stable 1.100 fixed; cloud-check installs Playwright Chromium on Cursor; hole-marking controls on the page for imported models, done on `main` by Cursor.
- 2026-11-13: The LCM Nearfield Monitor imported from its designer's files, with holes, air spaces, and moulded ducts in the importer, done on `main` by Claude.
- 2026-11-13: Walls for cabinets divided into chambers, foam-board sheet boxes, and tube end caps done on `main` by Claude.
- 2026-11-13: Wall vibration (sand-filled, plaster-filled, and two-filament walls heard) done on `main` by Claude.
- 2026-11-12: Novel materials, layered panels, priced wall builds; imported speakers; per-chamber loads; makers' curves; independent measurements; two-ways in the generator; cabinet generator; curved shape; wider score; air losses; inside-shaping and OpenSCAD — done on `main` by Claude (details in archive).
- 2026-11-11: Level 2 M4 complete; Level 3 room / exterior / DML / inside shape / fast-3D step 1 started — done on `main` by Claude / Fable (details in archive).
- 2026-11-10: Levels 0–2 M3 and Assignments 1–11 done on `main` (details in archive).
