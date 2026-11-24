/// <reference lib="webworker" />

import { loadWasmEngine } from "./wasmCall";

export type WaveRequest = {
  text: string;
  domain: "inside" | "outside" | "room";
  frequencyHz?: number;
  dxM?: number;
  pulse?: boolean;
};

export type WaveMessage =
  | { type: "progress"; steps: number; maxSteps: number }
  | { type: "result"; json: string; elapsedMs: number }
  | { type: "error"; message: string };

let lastPost = 0;

self.addEventListener("message", async (event: MessageEvent<WaveRequest>) => {
  try {
    const engine = await loadWasmEngine("/seatlock_wasm.wasm", (steps, maxSteps) => {
      const now = Date.now();
      if (now - lastPost > 200) {
        lastPost = now;
        self.postMessage({ type: "progress", steps, maxSteps } satisfies WaveMessage);
      }
      return true;
    });
    const started = performance.now();
    const json = engine.call(
      "simulate.wavefield",
      JSON.stringify({
        text: event.data.text,
        domain: event.data.domain,
        frequency_hz: event.data.frequencyHz,
        dx_m: event.data.dxM,
        pulse: event.data.pulse ?? true,
      }),
    );
    self.postMessage({
      type: "result",
      json,
      elapsedMs: performance.now() - started,
    } satisfies WaveMessage);
  } catch (error) {
    self.postMessage({
      type: "error",
      message: error instanceof Error ? error.message : String(error),
    } satisfies WaveMessage);
  }
});
