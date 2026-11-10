/// <reference lib="webworker" />

import { loadWasmEngine } from "./wasmCall";

self.addEventListener("message", async (event: MessageEvent<Record<string, number>>) => {
  try {
    const engine = await loadWasmEngine("/seatlock_wasm.wasm");
    const json = engine.call("fdtd.run", JSON.stringify(event.data));
    self.postMessage({ type: "done", json });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    self.postMessage({ type: "done", json: JSON.stringify({ error: message, elapsed_ms: 0 }) });
  }
});
