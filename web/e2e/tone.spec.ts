import { expect, test } from "@playwright/test";

test("test tone through the real worklet is a 440 Hz sine", async ({ page }) => {
  await page.goto("/");

  const result = await page.evaluate(async () => {
    const sampleRate = 48_000;
    const seconds = 0.5;
    const frames = sampleRate * seconds;
    const wasmBytes = await fetch("/seatlock_rt.wasm").then((response) => response.arrayBuffer());
    const ctx = new OfflineAudioContext(1, frames, sampleRate);
    await ctx.audioWorklet.addModule("/rt-worklet.js");
    const node = new AudioWorkletNode(ctx, "seatlock-rt", {
      processorOptions: { wasmBytes },
    });
    node.connect(ctx.destination);
    const rendered = await ctx.startRendering();
    const samples = rendered.getChannelData(0);

    let energy = 0;
    for (let i = 0; i < samples.length; i++) {
      energy += samples[i] * samples[i];
    }
    const rms = Math.sqrt(energy / samples.length);

    let first = -1;
    let last = -1;
    let crossings = 0;
    for (let i = 1; i < samples.length; i++) {
      if (samples[i - 1] <= 0 && samples[i] > 0) {
        if (first < 0) {
          first = i;
        }
        last = i;
        crossings += 1;
      }
    }
    const hz = crossings >= 2 ? ((crossings - 1) * sampleRate) / (last - first) : 0;
    return { rms, hz, crossings };
  });

  expect(result.rms).toBeGreaterThan(0.05);
  expect(Math.abs(result.hz - 440)).toBeLessThanOrEqual(2);
});

test("numeric coefficients change the kernel output", async ({ page }) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const sampleRate = 48_000;
    const frames = sampleRate;
    const wasmBytes = await fetch("/seatlock_rt.wasm").then((response) => response.arrayBuffer());
    const ctx = new OfflineAudioContext(1, frames, sampleRate);
    await ctx.audioWorklet.addModule("/rt-worklet.js");
    // command 3: 1 channel, 0 biquads, gain 0.25, delay 0, 0 FIRs
    const coeffs = [3, 1, 0, 0.25, 0, 0];
    const node = new AudioWorkletNode(ctx, "seatlock-rt", {
      processorOptions: { wasmBytes, coeffs },
    });
    node.port.postMessage(new Float64Array(coeffs));
    node.connect(ctx.destination);
    const rendered = await ctx.startRendering();
    const samples = rendered.getChannelData(0);
    const start = Math.floor(sampleRate * 0.5);
    let energy = 0;
    for (let i = start; i < samples.length; i++) {
      energy += samples[i] * samples[i];
    }
    const rms = Math.sqrt(energy / (samples.length - start));
    return { rms };
  });
  const expected = (0.15 * 0.25) / Math.sqrt(2);
  expect(Math.abs(result.rms - expected)).toBeLessThan(0.01);
});

test("FIR taps in the worklet change the kernel output", async ({ page }) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const sampleRate = 48_000;
    const frames = sampleRate;
    const wasmBytes = await fetch("/seatlock_rt.wasm").then((response) => response.arrayBuffer());
    const ctx = new OfflineAudioContext(1, frames, sampleRate);
    await ctx.audioWorklet.addModule("/rt-worklet.js");
    // 1 channel, 0 biquads, gain 1, delay 0, 1 FIR of one tap 0.5
    const coeffs = [3, 1, 0, 1, 0, 1, 1, 0.5];
    const node = new AudioWorkletNode(ctx, "seatlock-rt", {
      processorOptions: { wasmBytes, coeffs },
    });
    node.port.postMessage(new Float64Array(coeffs));
    node.connect(ctx.destination);
    const rendered = await ctx.startRendering();
    const samples = rendered.getChannelData(0);
    const start = Math.floor(sampleRate * 0.5);
    let energy = 0;
    for (let i = start; i < samples.length; i++) {
      energy += samples[i] * samples[i];
    }
    const rms = Math.sqrt(energy / (samples.length - start));
    return { rms };
  });
  const expected = (0.15 * 0.5) / Math.sqrt(2);
  expect(Math.abs(result.rms - expected)).toBeLessThan(0.01);
});
