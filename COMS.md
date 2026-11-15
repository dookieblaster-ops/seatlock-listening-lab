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

### 2026-11-15 — Cursor — Handoff: start here. Preference-as-target or BEM / woofers next

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/cta2034-preference-e85b` (stacks on passive R/L/C PR). Level 3.

**Done this session:** CTA-2034 / Olive preference rating on every listening-window run (early reflections, relative SP, PIR, score on the page). Criton ASR: LFX 42.5 Hz, preference 4.31 vs published 4.68 (formula with published terms recovers 4.68). Passive shortfall still ranks. Passive R/L/C tune was the prior item (PR #12).

**Open, in order:** (1) Use preference as the ranked score / redefine the target. (2) BEM; larger budget woofers; browser speed; non-box page fields; multi-source generator harden. (3) Real PLA/TPU damping — do not invent. (4) Criton leftovers only if a source appears.

**Pitfalls:** Page sets `cta2034: true` (sound power + ~46 ER directions). Scores/optimizer leave it off so candidates stay fast. Do not invent damping. Do not digitize graphs. Do not undo Q12.

**Last check:** `preference` 3 passed; `window` 3 passed; clippy `-D warnings` on sim/wasm. Re-running cloud-check after gating CTA off the optimizer path.

**Next step:** item 1 — preference as the ranked target, or item 2 (BEM / woofers / speed).

### 2026-11-15 — Cursor — Handoff: start here. Research plan or real damping next

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` (via PR). Level 3. Passive R/L/C value search is in.

**Done this session:** Optimize R/L/C toward a score. When a design has `dsp.passive`, the cost panel shows "Tune the passive board": Nelder–Mead on R/L/C values only (cabinet and wiring fixed, no digital correction). Mis-tuned first-order two-way shortfall 8.34 → 5.66 in 40 evals, seed-repeatable. README §7 / §11.3 / §12 F / §14 updated.

**Open, in order:** (1) Research plan: CTA-2034 preference score, BEM, larger budget woofers, browser speed, non-box page fields. (2) Real PLA/TPU damping — do not invent. (3) Criton leftovers only if a source appears: 0.47 µF nodes, port flare, lining resistivity, maker FRD. (4) Optional: price R/L/C parts in the cost model; lock inductor DCR resistors out of the search.

**Pitfalls:** Passive search uses `score_as_is` (correction filters are ignored under a passive board). Boards with many parts (Criton ~18) need more evals; the page asks for 60. Do not digitize graphs. Do not undo Q12.

**Last check:** `tools/cloud-check.sh` passed (engine 232, browser 41) on stable 1.100.

**Next step:** item 1 — research plan (README 2.6), or item 2 if the owner prefers walls.

### 2026-11-15 — Cursor — Handoff: start here. Optimize passive R/L/C next

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`. Level 3. Owner chose the CSS Criton 3TD-X over the Skyline kit because ASR published a Klippel measurement.

**Done this session:** Criton cabinet from the drawing and assembly guide. Then a pass over YouTube and forums for the leftovers. No 3TD-X assembly video. CSS clips are the 1TD-X crossover and a 2-vs-3 depth remark (13.5 in, already used). ASR thread confirms the front sealed box and that fancy caps do not measure differently. The 0.47 µF is on the parts list and on the tweeter/mid board photo, but neither the schematic text nor the photo shows its two nodes. No flare size, lining resistivity, or brace-hole model. Design unchanged by that pass.

**Open, in order:** (1) Optimize R/L/C toward a score. (2) Research plan: CTA-2034 preference score, BEM, larger budget woofers, browser speed. (3) Real PLA/TPU damping — do not invent. (4) Criton leftovers only if a source appears: 0.47 µF nodes, port flare, lining resistivity, maker FRD.

**Pitfalls:** 6 in chamber depth is the short side of braces C and D (6.75 in from the outside front). The 13.5 in floor also matches two 6 in denim pads plus a 3/4 in brace. The 9 in callout is taken as the top of the 3 in port, not a centerline. LD25X sensitivity 90 dB is from SoundImports. Do not digitize graphs. Do not undo Q12.

**Last check:** `cargo test -p seatlock-sim --test criton` 2 passed after the cabinet change. Full `tools/cloud-check.sh` not re-run.

**Next step:** optimize R/L/C toward a score.

### 2026-11-15 — Cursor — Handoff: start here. Optimize passive R/L/C next

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`. Level 3. Owner asked to skip the laptop check and do the other Level 0–2 quality items, plus more drivers and vents.

**Where the product stands (one screen):** Levels 0–2 complete. Level 3 in progress. Passive boards edit on the page and play. A design may have any number of vents and drivers; Play follows up to 16 drivers. ND25FA-4, RST28F-4, and RS225-4 now have mounting sizes from their drawings. No R/L/C optimizer yet.

**Done this session:** (1) Removed the four-opening cap in the quick and detailed models. (2) Page button "Add another driver" (opening + channel). Play kernel raised from 4 to 16 channels. (3) "Add a driver in series" for a dual-woofer tower (Skyline-style: two woofers, one current, plus a mid and a tweeter). (4) Mounting from the three Dayton drawings. (5) README: R/L/C value search is Level 3. The Skyline kit's own drivers and crossover drawing are not copied in.

**Open, in order (do the first without a new owner answer):**

1. **Optimize R/L/C** toward a score (Level 3 optimizer track).
2. **Research plan (README 2.6):** CTA-2034 preference rating as the lab's score; BEM; multi-source generator harden; larger budget woofers; browser speed; non-box page fields.
3. **Wall remainder:** real PLA/TPU damping figures (tap test / DMA — do not invent). Imported mesh flats, MDF/plywood, braces, partitions already vibrate.
4. **Optional:** 1/2 in MDF stock for C-Note; dimensioned baffle holes; inductor DCR; shrink LCM 3D self-check; partition coupling inside the waveguide tree. Laptop `./check.sh` skipped at the owner's request.

**Owner questions waiting (ask only when they start to matter):** 2 (playback hardware), 6 (panel material, wall distance, in-room tilt), 8 (depth-scanning phone). Q11 and Q12 answered — do not re-ask. Do not undo Q12. Do not digitize FR graphs.

**Pitfalls:** ND25FA cutout is the drawn Ø45 rear body (a Ø56 ring is also on the sheet; bolt-circle diameter is not printed). RST28F cutout is the drawn Ø72 body; Ø90 is the hole circle. Play stops at 16 drivers; the simulator does not. When `passive` is set, digital radiator blocks are left out. Cloud check: `tools/cloud-check.sh`.

**Last check:** `tools/cloud-check.sh` passed (engine 227, browser 41) before the series-woofer tests. After that: series unit test and the four-driver tower sim test passed; vitest `PassiveEditor` 3; `tsc` ok. Laptop check skipped.

**Next step:** item 1 — optimize R/L/C toward a score.

## Resolved

- 2026-11-15: CTA-2034 / Olive preference rating on listening-window runs (page + PIR graph), done by Cursor (PR on branch). Criton ASR preference 4.31 vs pub 4.68.
- 2026-11-15: Passive R/L/C value optimizer ("Tune the passive board"), done by Cursor (PR on branch). Mis-tuned first-order shortfall 8.34 → 5.66.
- 2026-11-15: CSS Criton 3TD-X kit copy vs ASR CTA-2034 (mid/treble ~2 dB RMS; bass relatively high), done on `main` by Cursor. Skyline skipped for lack of a Klippel measurement.
- 2026-11-15: Any number of vents, add-a-driver on the page, Play up to 16 drivers; three Dayton mounting drawings filled in. Laptop check skipped. Done on `main` by Cursor.
- 2026-11-15: Passive page netlist editor (ohm / mH / µF), done on `main` by Cursor.
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
