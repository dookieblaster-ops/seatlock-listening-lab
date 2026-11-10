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
