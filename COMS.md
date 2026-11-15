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

### 2026-11-15 — Cursor — Handoff: start here. Imported-cabinet walls vibrate

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`. Level 3. Continued standing queue item 0.

**Done this session:** (1) Imported `mesh` cabinets: air AABB from the solid, each axis-aligned outer face a plate with thickness measured through the plastic, layers from `build.material` / `build.wall`. (2) Sand-filled imported `sealed-test` matches the same parametric box within 0.06 dB; six faces; fidelity says walls vibrate. (3) README 5.11 / Level 3 C / open-items updated. Curved/slanted skin and per-part wall materials still out.

**Last check:** `cargo test -p seatlock-sim --test walls --test imported`: walls 10 passed; imported 7 passed. fmt; clippy `-D warnings` on sim tests. Full `tools/cloud-check.sh` not re-run.

**Open, in order:** (1) Standing queue: real PLA/TPU damping figures (tap test / DMA — do not invent). (2) Research plan: CTA-2034 preference score; BEM; multi-source generator; larger woofers; browser speed; non-box fields. (3) Optional: shrink LCM 3D self-check ~25 %; partition coupling in the waveguide tree; per-part materials for imported wall stacks.

**Pitfalls:** do not digitize Erin's FR. Do not undo Q12. Do not invent PLA/TPU loss factors. Mesh walls are axis-aligned flats only. Waveguide + partitions → lumped only. Walls off in engine default and in `correction_plan` on purpose.

**Next step:** CTA-2034 preference score (product-visible), or real damping figures if a sourced number is open in the session.

### 2026-11-15 — Cursor — Handoff: start here. Fresh lead session (Level 3)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` at `5128556`. Level 3. Wall-vibration track advanced; research-plan and Level 3 remainder still open. Older COMS threads are in `docs/coms-archive.md`. Owner paste for a new chat: `docs/new-chat.md`.

**Where the product stands (one screen):** Level 0–2 complete. Level 3 in progress: room, exterior 3D, DML panels, folded lines, generator (incl. multi-way), imports (3MF/STL), novel materials, wall vibration for printed cabinets (solid / filled / two-filament), divided cabinets, foam-board / MDF / plywood sheet boxes, tube end caps, **braces**, and **partitions**. LCM import + Erin CTA-2034 curves + TEBM independent data + Q12 acoustic level scale + 3D self-check ~25 %; leftover bass-mid tilt **placed** (not trimmed). Laptop: `./start.sh` runs; `./check.sh` not run there.

**Done on 2026-11-15 (Cursor cloud, this stretch):** MDF/plywood density & stiffness (Wood Handbook Table 12-1); LCM tilt cause placed; braces (`min_m`/`max_m`) take air/print/FDTD and split wall spans; partitions as solid plates with mutual Y in the lumped network (waveguide falls back when partitions active).

**Open, in order (do the first you can without a new owner answer):**

1. **Wall vibration remainder:** imported-cabinet walls; real damping figures for PLA and TPU cores (tap test / DMA — do not invent).
2. **Research plan (README 2.6):** CTA-2034 preference rating as the lab's score; generator for any number of sources (already partly there — harden); boundary-element exterior joined to the driver network; larger budget woofers; browser speed; page fields for non-box cabinets.
3. **Inside-shaping earn more:** folded line / ported box, listener-through-baffle objective, real 3D losses so the assumed loss can go.
4. **Fast 3D leftovers:** smaller exterior block (one-way boundary); WebGPU on the owner's laptop (integrated graphics only).
5. **Level 3 physics held for Fable if still available:** losses in the 3D model; 3D wave views; room mapping (STATUS Q8).
6. **Optional:** shrink LCM 3D self-check ~25 % further; partition coupling inside the waveguide tree (today: lumped only).

**Owner questions waiting (ask only when they start to matter):** 2 (playback hardware), 6 (panel material, wall distance, in-room tilt), 8 (depth-scanning phone). Q11 and Q12 are answered — do not re-ask. Do not undo Q12. Do not digitize Erin's FR graphs.

**Pitfalls:** Wood Handbook already used for MDF/plywood — do not invent stock figures. Brace cuts near a plate edge are ignored (sliver guard). Partition stack is solid print material, not the outer wall build. Waveguide + partitions → quick network. Imported mesh cabinets still rigid walls. Cloud check: `tools/cloud-check.sh` (Playwright Chromium install on Cursor if `/opt/pw-browsers` missing). Stable Rust ~1.100; do not edit catalog/Rust while that script embeds the catalog. Walls off in engine default and in `correction_plan` on purpose.

**Last check (this stretch):** targeted `walls` / `network` / `lcm` / `independent` / `imported` tests + fmt + clippy `-D warnings` on touched crates. Full `tools/cloud-check.sh` not re-run on the latest commits — run it after a large change or before claiming Level 3 milestones.

**Next step:** queue item 1 (imported-cabinet walls) or item 2 (CTA-2034 preference score). Prefer product-visible sound/score work over cleanup. Superseded by the handoff above.

## Resolved

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
