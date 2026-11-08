# Handoff

Seatlock Listening Lab, native engine. Read this before writing DSP.

## Locked decisions

- Rust is the engine. Kotlin is the Android shell. C++ is allowed only to bind a solver this repo does not own.
- The browser lab is the oracle for filter parity. It is not the product, and it must not be wrapped again.
- The WebView package `com.seatlock.listeninglab` 1.0.0 is retired as the product. Do not ship it as the listening lab.
- A driver enters `seatlock-catalog` only with a public source, the date it was read, and the numbers that source printed. No curves from a closed database. No numbers typed from memory.
- Helmholtz does not score a bend. Novel internal geometry is drawn and exported before it is simulated. If the model cannot hear it, the app says not simulated.
- Auto EQ cannot boost below 150 Hz or from 3 to 5 kHz, and it turns off when the result does not improve.
- Do not start a later step while the current step's test is still a stub that only proves the crate builds.

## State

Scaffold only. `cargo test` passes because each crate refuses to claim a result. `seatlock_engine::renders_audio()` is false. The catalog is empty. There is no Android app in this repo.

The oracle still lives in the web workspace, not in this repo:

- `src/lib/engine.ts` — the filter graph to match
- `src/lib/dsp.ts` — the biquads
- `src/lib/drivers.ts` — sheet numbers that must be read again before they move here
- `src/lib/geometry.ts` and `src/lib/cabinet.ts` — the box and solid the native geometry has to replace

Do not copy those numbers across until the sheet is open.

## Next step

Step 1 only.

1. Add an Android app that starts, opens an output stream, and calls into `seatlock-engine`.
2. The phone plays silence generated in Rust. Not a tone, not the lab's loop, not the WebView.
3. The test for this step is that the call crosses the bridge and the callback runs. It is not a listening test.

Do not port biquads in that change. Filter parity is step 2, and it fails if one fixed input diverges from the browser engine.

## After that

2. Port the crossover, delay, and waveshaper. One shared input. Fail on sample divergence.
3. Box math. First proof: the RS125 vented alignment whose published knee is 54 Hz in 6.8 L, re-read from the sheet before the number is encoded.
4. Parametric solid. Change a wall, a throat, or a fold. Export STL. Read the volume and the port length back. Resimulate only what the box model is allowed to hear.
5. Active crossover per driver, then auto EQ, each with an off switch.
6. A limited mesh pass for panel modes and one bend. Not a boundary-element claim.

## Do not

- Re-wrap the browser lab or the WebView APK and call it native.
- Invent a frequency response for a shape the box model cannot see.
- Fill the catalog from the web crate without the source open.
- Commit `*.keystore` or `keystore-password.txt`. The retired APK's key is not this app's key.
- Claim `cargo test` means the lab sounds right. Today it means the scaffold builds.
