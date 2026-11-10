/// <reference lib="webworker" />

import { loadWasmEngine } from "./wasmCall";
import type { Issue } from "./generated/Issue";

export type WorkerRequest =
  | { type: "init" }
  | { type: "call"; id: number; method: string; request: string }
  | { type: "design.check"; id: number; text: string }
  | { type: "simulate"; id: number; text: string; revision: number; frequencies: number[] };

export type WorkerResponse =
  | { type: "ready"; version: string }
  | { type: "result"; id: number; json: string }
  | { type: "error"; id?: number; message: string };

let engine: Awaited<ReturnType<typeof loadWasmEngine>> | null = null;

async function ensureEngine() {
  if (!engine) {
    engine = await loadWasmEngine("/seatlock_wasm.wasm");
  }
  return engine;
}

self.addEventListener("message", async (event: MessageEvent<WorkerRequest>) => {
  const msg = event.data;
  try {
    if (msg.type === "init") {
      const eng = await ensureEngine();
      const versionJson = eng.call("version", "{}");
      const { version } = JSON.parse(versionJson) as { version: string };
      const reply: WorkerResponse = { type: "ready", version };
      self.postMessage(reply);
      return;
    }

    if (msg.type === "call") {
      const eng = await ensureEngine();
      const json = eng.call(msg.method, msg.request);
      self.postMessage({ type: "result", id: msg.id, json } satisfies WorkerResponse);
      return;
    }

    if (msg.type === "design.check") {
      const eng = await ensureEngine();
      const request = JSON.stringify({ text: msg.text });
      const json = eng.call("design.check", request);
      self.postMessage({ type: "result", id: msg.id, json } satisfies WorkerResponse);
      return;
    }

    if (msg.type === "simulate") {
      const eng = await ensureEngine();
      const request = JSON.stringify({
        text: msg.text,
        revision: msg.revision,
        frequencies_hz: msg.frequencies,
      });
      const json = eng.call("simulate", request);
      self.postMessage({ type: "result", id: msg.id, json } satisfies WorkerResponse);
      return;
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    self.postMessage({
      type: "error",
      id: "id" in msg ? msg.id : undefined,
      message,
    } satisfies WorkerResponse);
  }
});

export type DesignCheckResult = {
  issues: Issue[];
  canonical_json: string;
};
