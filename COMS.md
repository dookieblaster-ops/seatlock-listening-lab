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

### 2026-11-25 — Cursor — Handoff: start here. Real exciters next (PR #33 merged)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` @ `76036d4` (PR #33 merged on owner local-merge go-ahead). Level 3.

**Done this session:** (1) Owner said yes to merge on local check. Fast-forwarded `main` with PR #33 (3D wave views + wow-pass; BEM aperture vs skin; Generate e2e path). PR #33 **merged**. (2) Soft-TPU / foamed-filament loss **skipped**: colorFabb varioShore TDS and Siraya Roamr TDS have no DMA / tan δ / loss factor; Irska Z-SEMIFLEX already rejected (Tα near room temp). Keep Irvine rubber 0.10 labeled. Do not invent.

**Open, in order:**
1. **Real exciters** in `catalog/exciters/` from Dayton DAEX (or other) sheets opened in-session — replace `test-exciter-a` for the panel speaker (README 9.1 / 12).
2. Optional BEM high-ka lit-gap only if it blocks.
3. Criton leftovers only if a source appears.
4. Level 3 remainder (STATUS / README 12): 3D losses / fill / loops / loft / FIR / … as capacity allows; Fable-held items stay held if still available.
5. Optional: fix GitHub Actions billing so Check runs again; second backup remote / Pro protect-`main` (`docs/backups.md`).

**Pitfalls:** Do not invent damping or exciter numbers. Do not weaken CHK-BEM-*. Do not cut wall vibration as open BEM mouths. Do not dump README physics into the wiki. Do not put tokens in chat. Do not digitize graphs. Do not undo Q12.

**Last check:** Prior local re-verify on the merged tip (fmt, clippy sim/fdtd/wasm, wavefield 3, bem_page_repro, WaveView 5). Actions still billing-blocked on this account.

**Next step:** source real exciters from open data sheets, or the next Level 3 remainder item that does not invent.

### 2026-11-25 — Cursor — Handoff: Owner billing or local-merge for PR #33 (superseded)

- **To:** next lead session (Claude or Cursor) + owner
- **Status:** handoff
- **Context:** PR #33. Superseded: owner allowed local-merge; #33 on `main`.

**Next step:** none (superseded).

### 2026-11-25 — Cursor — Handoff: Merge PR #33 (Actions billing block), then soft-TPU (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** PR #33 `cursor/wave-field-viz-741a` @ `9f2c018`. Superseded by lead-review handoff above.

**Next step:** none (superseded).

### 2026-11-25 — Cursor — Handoff: Soft-TPU / lit-gap after wow views (superseded)

- **To:** next lead session
- **Status:** handoff
- **Context:** PR #33. Superseded by merge-first wrap handoff above.

**Next step:** none (superseded).

### 2026-11-25 — Cursor — Handoff: Wow the 3D sound views (superseded)

- **To:** next lead session
- **Status:** handoff
- **Context:** PR #33. Superseded.

**Next step:** none (superseded).

### 2026-11-24 — Cursor — Handoff: Soft-TPU / lit-gap after wave views (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/wave-field-viz-741a` (PR #33). Superseded by wow-views handoff above.

**Done this session:** Level 3 D wave views; BEM+walls fix; WaveView paint fix; Generate Actions path. Superseded.

**Next step:** none (superseded).

### 2026-11-23 — Cursor — Handoff: Soft-TPU / lit-gap; owner backup leftovers (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` (Check: rustfmt + shape wall-gap bound). Level 3. Superseded by wave-views handoff above.

**Done this session:** Actions already on. Rustfmt fixed. Shape wall-gap bound derived. Superseded.

**Next step:** none (superseded).

### 2026-11-23 — Cursor — Handoff: Owner backup clicks, then soft-TPU / lit-gap (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` (wiki agent-duty README update). Level 3. Superseded by rustfmt/Actions handoff above.

**Done this session:** README 1.3 / 1.4 wiki duty. Superseded.

**Next step:** none (superseded).

### 2026-11-22 — Cursor — Handoff: Owner backup clicks, then soft-TPU / lit-gap (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` (PR #32 Check workflow + backups docs merged). Level 3. Superseded by wiki handoff above.

**Done this session:** Version-control backups package. Superseded.

**Next step:** none (superseded).

### 2026-11-22 — Cursor — Handoff: Soft-TPU DMA / lit-gap / Criton next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` (PR #31 sourced PLA damping merged). Level 3. Superseded by backups handoff above.

**Done this session:** Printed PLA η = 0.075; varioShore Young’s 40 MPa. Superseded.

**Next step:** none (superseded).

### 2026-11-22 — Cursor — Handoff: PLA damping / lit-gap next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` (PR #30 remesh reviewed and merged). Level 3. Superseded by sourced PLA damping above.

**Done this session:** BEM remesh for re-entrant shells (PR #30). Superseded.

**Next step:** none (superseded).

### 2026-11-22 — Cursor — Handoff: Remesh / PLA damping next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` (PR #29 CHK-BEM-10 reviewed and merged). Level 3. Superseded by remesh handoff above.

**Done this session:** Reviewed and merged PR #29 (CHK-BEM-10 true open mouths). Superseded.

**Next step:** none (superseded).

### 2026-11-22 — Cursor — Handoff: Remesh / PLA damping next (superseded; PR #29)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/bem-open-mouths-596c` (PR #29). **Merged to `main` by lead 2026-11-22.**

**Done this session:** CHK-BEM-10 true open mouths. Superseded by the review handoff above.

**Next step:** none (merged).

### 2026-11-22 — Cursor — Handoff: True openings / PLA damping next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` (PR #28 CHK-BEM-9 reviewed and merged). Level 3. Superseded by CHK-BEM-10 above.

**Next step:** none (superseded).

### 2026-11-22 — Cursor — Handoff: True openings / PLA damping next (superseded; PR #28)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/bem-mutual-mouths-641f` (PR #28). **Merged to `main` by lead 2026-11-22.**

**Done this session:** CHK-BEM-9 mutual BEM mouth loads. Superseded by the review handoff above.

**Next step:** none (merged).

### 2026-11-22 — Cursor — Handoff: Mutual mouths / PLA damping next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` (PR #27 page offer merged). Level 3. Superseded by CHK-BEM-9 above.

**Next step:** none (superseded).

### 2026-11-22 — Cursor — Handoff: Page offer of bem / PLA damping next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`, after review merge of PR #26 (CHK-BEM-8). Level 3. Superseded by the page-offer handoff above.

**Next step:** none (superseded).

### 2026-11-22 — Cursor — Handoff: PLA/TPU damping next (merged)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/bem-join-network-ff91` (PR #26). **Merged to `main` by lead 2026-11-22.**

**Done this session:** CHK-BEM-8 — join BEM to the driver network. Superseded by the review handoff above.

**Next step:** none (merged).

### 2026-11-19 — Cursor — Handoff: Join BEM to network next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`, after CHK-BEM-7 on `cursor/bem-curved-sphere-8bb5`. Level 3. Superseded by CHK-BEM-8 join above.

**Next step:** none (superseded).

### 2026-11-19 — Cursor — Handoff: Curved BEM / join network next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`, after merge of PR #25. Superseded by CHK-BEM-7 finding above.

**Next step:** none (superseded).

### 2026-11-18 — Cursor — Handoff: Curved BEM / join network next (merged)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/bem-linear-elements-8787` (PR #25). **Merged to `main` by lead 2026-11-19.**

**Done this session:** Discontinuous linear CBIE (CHK-BEM-6). Superseded by the review handoff above.

**Next step:** none (superseded).

### 2026-11-18 — Cursor — Handoff: Higher-order BEM / join network next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main` after PR #24. Superseded by the linear-BEM handoff above.

**Next step:** none (superseded).

### 2026-11-18 — Cursor — Handoff: Higher-order / join network next (merged)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/generator-harden-8787` (PR #24). **Merged to `main` by lead 2026-11-18.**

**Done this session:** Multi-source generator harden. Superseded by the review handoff above.

**Next step:** none (superseded).

### 2026-11-18 — Cursor — Handoff: Higher-order / join network / generator harden (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`, after review of PR #23. Superseded by the generator-harden handoff above.

**Done this session:** Reviewed PR #23; CHIEF on `main`.

**Open, in order:** superseded.

**Next step:** none (superseded).

### 2026-11-17 — Cursor — Handoff: Higher-order elements / join network / generator harden next (merged)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/bem-burton-miller-e8db` (PR #23). Level 3. **Merged to `main` by lead 2026-11-18.**

**Done this session:** (1) Merged outer-shape mesh to `main`. (2) Irregular-frequency fix: CHIEF interior points on the default CBIE solve (CHK-BEM-5). At the discrete peak `ka ≈ 3.2` on 320 faces, CHIEF within 0.5 dB while plain CBIE is >2 dB off; CHK-BEM-1…4 still pass. (3) Burton–Miller prototype kept as `BurtonMillerSystem` (monopole-ok) but not the radiation default — constant discontinuous elements break CHK-BEM-3 under BM. Higher-order/continuous elements come before production BM.

**Open, in order:** superseded by the lead review handoff above.

**Next step:** none (superseded).

### 2026-11-17 — Cursor — Handoff: Burton–Miller / higher-order / generator harden next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/bem-outer-shape-mesh-e8db` (merged to `main`). Level 3.

**Done this session:** True outer-shape meshing for `bem` (AABB seed → `contains_cabinet`). Merged to `main`.

**Open, in order:** superseded by the CHIEF handoff above.

**Next step:** none (superseded).

### 2026-11-16 — Cursor — Handoff: start here. BEM shape meshing next (field sign fixed)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`, after review of `cursor/bem-radiation-wire-up-027d` (PR #20). Level 3.

**Done this session:** Reviewed the helper's `bem` radiation member against README 2.6 / 3.2 / 3.3 / 5.6. Two defects, both fixed before accepting it. (1) The exterior field integral had the opposite sign: level within 0.15 dB of the pulsating sphere, phase 180° off. With the outward normal, Green's identity is `∫ (p ∂G/∂n − G q)`. CHK-BEM-4 now requires phase within 10° (measured 0.07° / 0.34° / 1.09° at ka 0.5 / 1 / 2). (2) `seatlock-wasm` did not compile: `full_3d` lists every `SimulateRequest` field and omitted `bem_subdivisions`. The worker now passes that field. Accepted as-is: box AABB mesh, one LU per frequency, baffle hand-over, sealed-test within 2 dB of baffle from 30 to 400 Hz. Sound power still uses the baffle integral. The page does not offer `bem` yet.

**Open, in order:** (1) BEM: true outer-shape meshing (not just the bounding box); higher-order elements / Burton–Miller; join the driver network; multi-source generator harden. (2) Real PLA/TPU damping — do not invent. (3) Criton leftovers only if a source appears. (4) Optional: owner may change `preference_min` (default 5.0).

**Pitfalls:** Do not invent damping. Do not weaken CHK-BEM-2/3/4 — phase is part of CHK-BEM-4, and a magnitude-only check hid the sign error. Dense O(N²) — a worker when the page is wired. Openings remain rigid patches. Do not digitize graphs. Do not undo Q12.

**Last check:** `tools/cloud-check.sh` re-run 2026-11-17 at owner request (laptop check stand-in; Mint not linked): engine 257, browser 44, stable 1.100. CHK-BEM-4 phase 0.07° / 0.34° / 1.09° at ka 0.5 / 1 / 2.

**Next step:** mesh real outer shapes for `bem`, or higher-order / Burton–Miller, or harden the multi-source generator.

### 2026-11-16 — Cursor — Handoff: BEM shape meshing / generator harden next (reviewed)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/bem-radiation-wire-up-027d` (PR onto `main`). Level 3.

**Done this session:** (1) Earlier on `main`: CHK-BEM-3; high-ka rigid-sphere limit placed. (2) CHK-BEM-4 pulsating-sphere exterior field at 1 m within 0.5 dB. (3) Registered `bem` radiation: AABB box mesh (`bem_subdivisions` default 4), LU reuse per frequency, representation formula at listeners, baffle hand-over above ~6 elements/wavelength; sound power still baffle. Sealed-test front driver within 2 dB of baffle 30–400 Hz.

**Open, in order:** (1) BEM: true outer-shape meshing (not just AABB); higher-order elements / Burton–Miller; join network; multi-source generator harden. (2) Real PLA/TPU damping — do not invent. (3) Criton leftovers only if a source appears. (4) Optional: owner may change `preference_min` (default 5.0).

**Pitfalls:** Do not invent damping. Do not weaken CHK-BEM-2/3/4 to claim high-ka rigid-sphere series. Dense O(N²) — worker when page-wired. Openings remain rigid patches. Do not digitize graphs. Do not undo Q12.

**Last check:** `cargo test -p seatlock-bem --release` 12 passed; `cargo test -p seatlock-sim --release --test bem` 2 passed; clippy `-D warnings` on seatlock-bem + seatlock-sim. Full `tools/cloud-check.sh` not re-run this turn.

**Next step:** mesh real outer shapes for `bem`, or higher-order / Burton–Miller, or harden the multi-source generator.

### 2026-11-16 — Cursor — Handoff: BEM radiation wire-up / higher-order elements / generator harden next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** `main`. Level 3.

**Done this session:** (1) Earlier: merged helper stack #12–#19; CHK-BEM-2 low-ka rigid sphere. (2) CHK-BEM-3 oscillating-sphere dipole (non-uniform Neumann): rel L1 < 0.03 at ka 0.5/1/2. (3) Diagnosed ka≳1 rigid-sphere plane-wave gap: not irregular-frequency (refine 80→5120 faces converges to ~2.6 dB lit error vs smooth series); plane-wave CBIE identity holds with free term +½; radiation uses −½ and matches monopole/dipole. Constant-element collocation vs smooth sphere is the limit — do not weaken checks.

**Open, in order:** (1) BEM: higher-order surface elements (or Burton–Miller for irregular freqs near ka=π); register as radiation; join network; multi-source generator harden. (2) Real PLA/TPU damping — do not invent. (3) Criton leftovers only if a source appears. (4) Optional: owner may change `preference_min` (default 5.0).

**Pitfalls:** Do not invent damping. Do not weaken CHK-BEM-2/3 to claim high-ka rigid-sphere series. Dense O(N²) — worker when wired. Do not digitize graphs. Do not undo Q12.

**Last check:** `cargo test -p seatlock-bem --release` 8 passed; clippy `-D warnings` on seatlock-bem. Full `tools/cloud-check.sh` not re-run this turn.

**Next step:** register BEM as a radiation member, or higher-order elements for high-ka rigid sphere, or harden the multi-source generator.

### 2026-11-16 — Cursor — Handoff: BEM radiation wire-up / generator harden next (merged to main)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/bem-exterior-scaffold-e85b` (stacks on non-box fields #18 → … → #12). Level 3. **Merged to `main` by lead 2026-11-16.**

**Done this session:** Exterior BEM scaffold crate `seatlock-bem`. Constant triangles, collocation CBIE, Duffy self-term, icosphere. CHK-BEM-1: pulsating sphere |p| within 0.05 dB (ka 0.5/1/2, 320 faces). Prior: non-box page fields (#18).

**Open, in order:** (1) BEM next: register as radiation / match rigid-sphere CHK-RAD-3 / Burton–Miller / join network; multi-source generator harden. (2) Real PLA/TPU damping — do not invent. (3) Criton leftovers only if a source appears.

**Pitfalls:** Do not invent damping. Irregular frequencies above `ka ≈ π` without Burton–Miller/CHIEF. Dense O(N²) solve — keep in a worker when wired to the page.

**Last check:** `tools/cloud-check.sh` passed (engine + browser 44 in 7.9m) on stable 1.100. CHK-BEM-1 ≤0.05 dB.

**Next step:** wire BEM as a radiation member or harden the multi-source generator.

### 2026-11-16 — Cursor — Handoff: BEM / generator harden next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/non-box-page-fields-e85b` (stacks on budget woofers #17 → speed #16 → … → #12). Level 3.

**Done this session:** Non-box page fields. `OuterFields` edits tube / revolved / sloped / prism / sheet_box / rounded / chamfered with the right meters; boxes keep width/height/depth. Prior: budget 8 in woofers (#17).

**Open, in order:** (1) BEM; multi-source generator harden; more browser speed if needed. (2) Real PLA/TPU damping — do not invent. (3) Criton leftovers only if a source appears. (4) Optional: owner may change `preference_min`.

**Pitfalls:** Do not invent damping. Do not digitize FR graphs. Do not undo Q12. BEM is the big Level 3 physics item (README 2.6 / 12 D).

**Last check:** `tools/cloud-check.sh` passed (engine + browser 43, 1 flaky import retry) in 8.2m on stable 1.100.

**Next step:** item 1 — BEM or multi-source generator harden.

### 2026-11-16 — Cursor — Handoff: BEM / non-box fields / generator harden next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/budget-8in-woofers-e85b` (stacks on speed #16 → preference #15 → CTA #13 → passive #12). Level 3.

**Done this session:** Two budget 8 in woofers in the catalog: Dayton DS215-8 and DC200-8 (Parts Express PDF + product page, read 2026-11-16). Generator pool includes them. Example `ds215-sealed` reaches f6 71 Hz (TCP115 mdf-box 92 Hz). Prior: preference scoring speed (#16), preference ranking (#15), CTA (#13), passive (#12).

**Open, in order:** (1) BEM; non-box page fields; multi-source generator harden; more browser speed if needed. (2) Real PLA/TPU damping — do not invent. (3) Criton leftovers only if a source appears. (4) Optional: owner may change `preference_min`.

**Pitfalls:** DC200-8 has no published bolt circle (screw holes left to the builder). Do not invent damping. Do not digitize FR graphs. Do not undo Q12.

**Last check:** `tools/cloud-check.sh` passed (engine + browser 41 in 8.0m) on stable 1.100.

**Next step:** item 1 — BEM, non-box page fields, or multi-source generator harden.

### 2026-11-16 — Cursor — Handoff: BEM / woofers / non-box fields next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/speed-preference-score-e85b` (stacks on preference-as-target #15 → CTA #13 → passive #12). Level 3.

**Done this session:** Faster preference scoring. Early-reflection angles corrected to CTA-2034 bounce groups (26 dirs, was ~47 with invented 5°/rear-arc extras). Listening window + ER share one `transfer_many`; baffle outlines built once per batch; sound-power directions batched the same way. Single sealed-test score ~0.54 s release (was ~0.93 s). Prior stack: preference ranking (#15), CTA report (#13), passive R/L/C (#12).

**Open, in order:** (1) BEM; larger budget woofers; non-box page fields; multi-source generator harden; more browser speed if still needed. (2) Real PLA/TPU damping — do not invent. (3) Criton leftovers only if a source appears. (4) Optional: owner may change `preference_min`.

**Pitfalls:** Preference scoring still runs sound power (48 sphere dirs) + ~28 unique off-axis points. Do not invent damping. Do not digitize graphs. Do not undo Q12.

**Last check:** Browser e2e 41 passed in 7.8m (optimizer done ~123 s). Import e2e hardened for canvas/volume races; `cloud-check.sh` now fails when e2e reports failures. Clippy `-D warnings` on core/acoustics/sim earlier.

**Next step:** item 1 — BEM, larger woofers, or non-box page fields.

### 2026-11-15 — Cursor — Handoff: BEM / woofers / speed next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/preference-as-target-e85b` (stacks on CTA PR #13 → passive #12). Level 3.

**Done this session:** Preference is the ranked quality score. Targets carry `preference_min` (default 5.0). Shortfall = preference deficit + clean-level deficit. Scores, optimizer, and generator ask for `cta2034`. Cost table shows Preference first. Passive tune still improves (9.87 → 7.54). Prior: CTA report (PR #13), passive R/L/C (PR #12).

**Open, in order:** (1) BEM; larger budget woofers; browser speed (preference scoring is slower); non-box page fields; multi-source generator harden. (2) Real PLA/TPU damping — do not invent. (3) Criton leftovers only if a source appears. (4) Optional: owner may change `preference_min` in `targets/hifi-default.json`.

**Pitfalls:** Every scored candidate runs CTA early reflections (~46 directions) — cost-quality and generate e2e need long timeouts. Do not invent damping. Do not digitize graphs. Do not undo Q12.

**Last check:** `tools/cloud-check.sh` passed (engine + browser 41 in 9.7m) on stable 1.100.

**Next step:** item 1 — BEM, larger woofers, or browser speed (preference path is the new bottleneck).

### 2026-11-15 — Cursor — Handoff: preference-as-target next (superseded)

- **To:** next lead session (Claude or Cursor)
- **Status:** handoff
- **Context:** branch `cursor/cta2034-preference-e85b` (stacks on passive R/L/C PR #12). Level 3.

**Done this session:** CTA-2034 / Olive preference rating behind `cta2034` request flag (page on; scores/optimizer off). Early reflections, relative SP, PIR, score on the page. Criton ASR: LFX 42.5 Hz, preference 4.31 vs published 4.68 (formula with published terms recovers 4.68). Passive shortfall still ranks. Passive R/L/C tune was the prior item (PR #12).

**Open, in order:** (1) Use preference as the ranked score / redefine the target. (2) BEM; larger budget woofers; browser speed; non-box page fields; multi-source generator harden. (3) Real PLA/TPU damping — do not invent. (4) Criton leftovers only if a source appears.

**Pitfalls:** Page sets `cta2034: true` (sound power + ~46 ER directions). Scores/optimizer leave it off so candidates stay fast. Do not invent damping. Do not digitize graphs. Do not undo Q12.

**Last check:** `tools/cloud-check.sh` passed (engine + browser 41) on stable 1.100 after gating.

**Next step:** item 1 — preference as the ranked target.

### 2026-11-15 — Cursor — Handoff: CTA-2034 gated; cloud-check pending (superseded)

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

- 2026-11-25: PR #33 (3D wave views + wow-pass) merged to `main` on owner local-merge go-ahead; soft-TPU filament DMA skipped (no source).
- 2026-11-24: 3D wave views (inside FDTD / outside BEM / room bass grid) on the page with animated standing wave and pulse.
- 2026-11-23: GitHub Actions confirmed on; rustfmt fixed; shape wall-gap bound derived (Actions 8.05 % gap); protect-`main` still Pro-gated; optional backup URL still open.
- 2026-11-23: Living software wiki at `docs/wiki/` (structure articles; README remains the spec).
- 2026-11-22: GitHub Check workflow + backup inspection docs landed (Actions later confirmed on).
- 2026-11-22: PR #31 sourced PLA damping (η = 0.075) and varioShore Young’s (40 MPa) merged to `main`.
- 2026-11-22: Printed PLA loss factor sourced (Rothe 2021 η = 0.075); varioShore Young’s from maker TDS (40 MPa foamed).
- 2026-11-22: PR #30 BEM remesh for re-entrant shells reviewed and merged to `main`.
- 2026-11-22: BEM remesh for re-entrant shells (`mesh_outer_shape` occupancy path when not star-convex).
- 2026-11-22: PR #29 CHK-BEM-10 (true open mouths) reviewed and merged to `main`.
- 2026-11-22: CHK-BEM-10 true open mouths (exact-area aperture disks cut into the outer BEM mesh).
- 2026-11-22: PR #28 CHK-BEM-9 (mutual BEM mouth loads) reviewed and merged to `main`.
- 2026-11-22: CHK-BEM-9 mutual BEM mouth loads joined to the waveguide network.
- 2026-11-22: Page offer of `bem` ("Run the boundary-element outside" compare overlay); everyday radiation stays `baffle`.
- 2026-11-22: PR #26 CHK-BEM-8 (BEM mouth self-`Z_rad` joined to waveguide) reviewed and merged to `main`.
- 2026-11-19: CHK-BEM-7 spherical-triangle BEM; high-ka lit gap placed as not geometry-only.
- 2026-11-19: PR #25 discontinuous linear BEM (CHK-BEM-6) reviewed and merged to `main`.
- 2026-11-18: Discontinuous linear BEM (CHK-BEM-6); high-ka rigid-sphere gap placed as faceted geometry.
- 2026-11-18: PR #24 multi-source generator harden reviewed and merged to `main`.
- 2026-11-18: Multi-source generator harden (stacked / folded-line / dual-woofer throw-outs cut sharply).
- 2026-11-18: PR #23 CHIEF irregular-frequency fix (CHK-BEM-5) reviewed and merged to `main`; Burton–Miller kept experimental on constant elements.
- 2026-11-17: CHIEF irregular-frequency fix (CHK-BEM-5); BM on constant elements deferred.
- 2026-11-17: `bem` true outer-shape meshing (AABB seed → `contains_cabinet`) merged to `main`.
- 2026-11-16: Helper `bem` radiation (PR #20) reviewed; exterior field sign corrected and CHK-BEM-4 phase held within 10°; worker passes `bem_subdivisions`.
- 2026-11-16: CHK-BEM-3 oscillating-sphere dipole on `main`; high-ka rigid-sphere vs smooth series placed as constant-element limit (not irregular-frequency).
- 2026-11-16: CHK-BEM-2 low-ka rigid-sphere surface pressures on `main` by Cursor lead (ka 0.25/0.5 within 0.75 dB; higher ka still open).
- 2026-11-16: Helper stack #12–#19 (passive R/L/C tune → CTA preference → preference target → speed → 8 in woofers → non-box fields → BEM scaffold) reviewed and merged to `main` by Cursor lead; cloud-check re-run green (engine 247, browser 44).
- 2026-11-15: CTA-2034 / Olive preference rating on listening-window runs (page + PIR graph), merged to `main`. Criton ASR preference 4.31 vs pub 4.68.
- 2026-11-15: Passive R/L/C value optimizer ("Tune the passive board"), merged to `main`. Mis-tuned first-order shortfall 8.34 → 5.66.
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
