# Handoff

Seatlock Listening Lab, native engine. Read this before writing DSP.

## Locked decisions

- Rust is the engine. Kotlin is the Android shell. C++ is allowed only to bind a solver this repo does not own.
- The browser lab is the oracle for filter parity. It is not the product, and it must not be wrapped again.
- The WebView package that played the browser lab is retired. `com.seatlock.listeninglab` version 0.1.0 is the native shell. It plays silence from Rust, not that WebView.
- A driver enters `seatlock-catalog` only with a public source, the date it was read, and the numbers that source printed. No curves from a closed database. No numbers typed from memory.
- Helmholtz does not score a bend. Novel internal geometry is drawn and exported before it is simulated. If the model cannot hear it, the app says not simulated.
- Auto EQ cannot boost below 150 Hz or from 3 to 5 kHz, and it turns off when the result does not improve.
- Do not start a later step while the current step's test is still a stub that only proves the crate builds.

## State

Step 1 is in the tree. `seatlock_engine::fill_silence` writes zeros and increments `frames_filled`. `seatlock-jni` exposes that to `com.seatlock.listeninglab.Engine`. The Kotlin activity opens an `AudioTrack` at 48 kHz, mono, float, and pulls buffers on a thread named `seatlock-audio`.

`renders_audio()` is still false. The catalog is empty. DSP, box, and geometry crates still refuse to claim a result.

The host test is `silence_crosses_the_engine_and_is_counted`. It does not need a phone. The phone check is the frame counter on the screen, which only moves if the stream is calling Rust.

The oracle still lives in the web workspace, not in this repo:

- `src/lib/engine.ts` — the filter graph to match
- `src/lib/dsp.ts` — the biquads
- `src/lib/drivers.ts` — sheet numbers that must be read again before they move here
- `src/lib/geometry.ts` and `src/lib/cabinet.ts` — the box and solid the native geometry has to replace

Do not copy those numbers across until the sheet is open.

## Next step

Step 2 only.

Port the crossover, delay, and waveshaper into `seatlock-dsp`. Feed one fixed input to the browser engine and to Rust. Fail if the samples diverge. Do not put that path in the phone callback until the test passes. The phone keeps playing silence until then.

## After that

3. Box math. First proof: the RS125 vented alignment whose published knee is 54 Hz in 6.8 L, re-read from the sheet before the number is encoded.
4. Parametric solid. Change a wall, a throat, or a fold. Export STL. Read the volume and the port length back. Resimulate only what the box model is allowed to hear.
5. Active crossover per driver, then auto EQ, each with an off switch.
6. A limited mesh pass for panel modes and one bend. Not a boundary-element claim.

## Do not

- Re-wrap the browser lab or the WebView APK and call it native.
- Invent a frequency response for a shape the box model cannot see.
- Fill the catalog from the web crate without the source open.
- Commit `*.keystore`, `keystore-password.txt`, or `android/local.properties`. The retired APK's key is not checked in here.
- Claim `cargo test` means the lab sounds right. The engine test only proves silence is written and counted.
