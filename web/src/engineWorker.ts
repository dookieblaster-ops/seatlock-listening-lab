/// <reference lib="webworker" />

import { loadWasmEngine } from "./wasmCall";
import type { Issue } from "./generated/Issue";

export type WorkerRequest =
  | { type: "init" }
  | { type: "call"; id: number; method: string; request: string }
  | { type: "design.check"; id: number; text: string }
  | {
      type: "simulate";
      id: number;
      text: string;
      revision: number;
      frequencies: number[];
      /** Defaults to `baffle` (everyday path). Pass `bem` for the boundary-element compare. */
      radiation?: string;
      bem_subdivisions?: number;
    }
  | { type: "geometry"; id: number; text: string }
  | { type: "correct"; id: number; text: string }
  | {
      type: "listen";
      id: number;
      text: string;
      correction: boolean;
      impulseLen: number;
      sampleRate: number;
    }
  | { type: "catalog.driver"; id: number; driverId: string };

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

    if (msg.type === "correct") {
      const eng = await ensureEngine();
      const json = eng.call("correct", JSON.stringify({ text: msg.text }));
      self.postMessage({ type: "result", id: msg.id, json } satisfies WorkerResponse);
      return;
    }

    if (msg.type === "catalog.driver") {
      const eng = await ensureEngine();
      const json = eng.call("catalog.driver", JSON.stringify({ text: msg.driverId }));
      self.postMessage({ type: "result", id: msg.id, json } satisfies WorkerResponse);
      return;
    }

    if (msg.type === "listen") {
      const eng = await ensureEngine();
      const json = eng.call(
        "listen",
        JSON.stringify({
          text: msg.text,
          correction: msg.correction,
          impulse_len: msg.impulseLen,
          sample_rate_hz: msg.sampleRate,
        }),
      );
      self.postMessage({ type: "result", id: msg.id, json } satisfies WorkerResponse);
      return;
    }

    if (msg.type === "geometry") {
      const eng = await ensureEngine();
      const request = JSON.stringify({ text: msg.text });
      const json = eng.call("geometry", request);
      self.postMessage({ type: "result", id: msg.id, json } satisfies WorkerResponse);
      return;
    }

    if (msg.type === "simulate") {
      const eng = await ensureEngine();
      const request = JSON.stringify({
        text: msg.text,
        revision: msg.revision,
        frequencies_hz: msg.frequencies,
        // The detailed model: as fast as the quick one, and it knows lines and tapers.
        enclosure: "waveguide",
        // Everyday path stays `baffle`. The page's boundary-element compare passes `bem`.
        radiation: msg.radiation ?? "baffle",
        bem_subdivisions: msg.bem_subdivisions,
        // Evenness is judged a little off to the sides too (README 11.1).
        listening_window: true,
        // CTA-2034 curves and Olive preference rating (README 2.6).
        cta2034: true,
        // The air's loss at duct walls (README 5.4).
        losses: true,
        // Cabinet walls vibrate and are heard (README 5.11).
        walls: true,
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
