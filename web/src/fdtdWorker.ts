/// <reference lib="webworker" />

import { loadWasmEngine } from "./wasmCall";

export type FullRequest = {
  text: string;
  cellSizes: number[];
  frequencies: number[];
};
export type FullMessage =
  | { type: "progress"; dx: number; steps: number; maxSteps: number }
  | { type: "result"; dx: number; json: string }
  | { type: "done" }
  | { type: "error"; message: string };

let currentDx = 0;
let lastPost = 0;

function post(message: FullMessage) {
  self.postMessage(message);
}

self.addEventListener("message", async (event: MessageEvent<FullRequest>) => {
  try {
    const engine = await loadWasmEngine(
      "/seatlock_wasm.wasm",
      (steps, maxSteps) => {
        const now = Date.now();
        if (now - lastPost > 200) {
          lastPost = now;
          post({ type: "progress", dx: currentDx, steps, maxSteps });
        }
        return true;
      },
    );
    for (const dx of event.data.cellSizes) {
      currentDx = dx;
      post({ type: "progress", dx, steps: 0, maxSteps: 1 });
      const started = performance.now();
      let json = engine.call(
        "simulate.full3d",
        JSON.stringify({
          text: event.data.text,
          fdtd_dx_m: dx,
          radiation: "baffle",
          frequencies_hz: event.data.frequencies,
        }),
      );
      // The engine has no clock in WebAssembly; time the run here.
      const result = JSON.parse(json) as { fdtd?: { elapsed_ms: number } | null };
      if (result.fdtd) {
        result.fdtd.elapsed_ms = performance.now() - started;
        json = JSON.stringify(result);
      }
      post({ type: "result", dx, json });
    }
    post({ type: "done" });
  } catch (error) {
    post({
      type: "error",
      message: error instanceof Error ? error.message : String(error),
    });
  }
});
