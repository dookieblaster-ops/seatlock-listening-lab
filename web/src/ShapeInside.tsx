import { useEffect, useRef, useState } from "react";

import { Graph } from "./Graph";
import type { JobMessage } from "./jobWorker";

/** What the engine's `shape.inside` returns (README section 5.5, "Shaping the inside"). */
type ShapeResult = {
  dx_m: number;
  plastic_m3: number;
  region_m3: number;
  frequencies_hz: number[];
  level_before_db: number[];
  level_after_db: number[];
  uneven_before_db: number;
  uneven_after_db: number;
  runs: number;
  canceled: boolean;
  notes: string[];
};
type Reply = { result: ShapeResult; design: unknown } | { error: string };

/**
 * "Shape the inside": the 3D model places plastic inside the cabinet, cell by cell, where it
 * evens out the level, and offers the result as a design.
 */
export function ShapeInside({ text, onOpen }: { text: string; onOpen: (design: unknown, label: string) => void }) {
  const [cell, setCell] = useState(0.02);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [found, setFound] = useState<{ result: ShapeResult; design: unknown; of: string } | null>(null);
  const worker = useRef<Worker | null>(null);
  useEffect(() => () => worker.current?.terminate(), []);

  function stop() {
    worker.current?.terminate();
    worker.current = null;
    setBusy(false);
    setNote("Stopped.");
  }

  function start() {
    worker.current?.terminate();
    const job = new Worker(new URL("./jobWorker.ts", import.meta.url), { type: "module" });
    worker.current = job;
    setBusy(true);
    setFound(null);
    setNote("Solving the air inside the cabinet in 3D...");
    const of = text;
    job.onmessage = (event: MessageEvent<JobMessage>) => {
      const message = event.data;
      if (message.type === "progress") {
        setNote(`Shaping: step ${message.done} of at most ${message.total}.`);
        return;
      }
      job.terminate();
      worker.current = null;
      setBusy(false);
      if (message.type === "error") {
        setNote(message.message);
        return;
      }
      const reply = JSON.parse(message.json) as Reply;
      if ("error" in reply) {
        setNote(reply.error);
        return;
      }
      setFound({ ...reply, of });
      setNote("");
    };
    job.postMessage({ method: "shape.inside", request: { text, dx_m: cell } });
  }

  const result = found?.result;
  const better = result ? result.uneven_after_db < result.uneven_before_db - 0.005 && result.plastic_m3 > 0 : false;
  return (
    <section data-testid="shape-inside">
      <h2>Shape the inside</h2>
      <p>
        The 3D model works out, block by block, where plastic inside the cabinet evens out the sound: it breaks up the
        echoes between the walls (standing waves) that color the midrange. The result is printed with the cabinet.
      </p>
      <label title="Smaller blocks follow the sound more closely and take several times longer.">
        Block size{" "}
        <select value={cell} onChange={(event) => setCell(Number(event.target.value))} disabled={busy}>
          <option value={0.02}>20 mm (quick)</option>
          <option value={0.01}>10 mm (several minutes)</option>
        </select>
      </label>{" "}
      <button type="button" data-testid="shape-start" disabled={!text || busy} onClick={start}>
        Let the 3D model shape the inside
      </button>{" "}
      {busy ? (
        <button type="button" data-testid="shape-stop" onClick={stop}>
          Stop
        </button>
      ) : null}
      {note ? <p data-testid="shape-note">{note}</p> : null}
      {result && found ? (
        <div data-testid="shape-result">
          {found.of !== text ? <p>The design has changed since this was worked out.</p> : null}
          <p data-testid="shape-summary">
            {better
              ? `Unevenness between ${result.frequencies_hz[0].toFixed(0)} Hz and ${result.frequencies_hz[
                  result.frequencies_hz.length - 1
                ].toFixed(0)} Hz falls from ${result.uneven_before_db.toFixed(2)} dB to ${result.uneven_after_db.toFixed(
                  2,
                )} dB with ${(result.plastic_m3 * 1000).toFixed(2)} L of plastic inside, checked by a fresh 3D run of the finished shape.`
              : `No plastic inside made this cabinet more even (it is at ${result.uneven_before_db.toFixed(
                  2,
                )} dB between ${result.frequencies_hz[0].toFixed(0)} Hz and ${result.frequencies_hz[
                  result.frequencies_hz.length - 1
                ].toFixed(0)} Hz), so nothing is added.`}
          </p>
          <Graph
            title="Level from the inside's shape"
            explanation="Green is the shaped inside, brown the plain one: how the cabinet's inside changes the level, with the average taken out. A flatter line is better."
            freqs={result.frequencies_hz}
            values={result.level_after_db}
            overlay={result.level_before_db}
            unit="dB"
            testId="shape-graph"
            fidelity={`Full 3D enclosure at ${(result.dx_m * 1000).toFixed(0)} mm cells; baffle and room left out. ${result.notes.join(" ")}`}
          />
          {better ? (
            <button
              type="button"
              data-testid="shape-use"
              disabled={found.of !== text}
              onClick={() => onOpen(found.design, "shaped inside")}
            >
              Use this inside
            </button>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
