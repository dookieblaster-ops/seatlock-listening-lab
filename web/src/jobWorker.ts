/// <reference lib="webworker" />

import { loadWasmEngine } from "./wasmCall";

/** One long engine call off the main engine worker, with progress (README section 3). */
export type JobRequest = { method: string; request: Record<string, unknown> };
export type JobMessage =
  | { type: "progress"; done: number; total: number }
  | { type: "result"; json: string }
  | { type: "error"; message: string };

let lastPost = 0;

self.addEventListener("message", async (event: MessageEvent<JobRequest>) => {
  try {
    const engine = await loadWasmEngine("/seatlock_wasm.wasm", (done, total) => {
      const now = Date.now();
      if (now - lastPost > 200) {
        lastPost = now;
        self.postMessage({ type: "progress", done, total } satisfies JobMessage);
      }
      return true;
    });
    const json = engine.call(event.data.method, JSON.stringify(event.data.request));
    self.postMessage({ type: "result", json } satisfies JobMessage);
  } catch (error) {
    self.postMessage({
      type: "error",
      message: error instanceof Error ? error.message : String(error),
    } satisfies JobMessage);
  }
});
