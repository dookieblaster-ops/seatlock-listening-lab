import { useRef, useState } from "react";

import type { JobMessage } from "./jobWorker";

/** A design's score after correction, beside its cost (README sections 9.2 and 11). */
export type Score = {
  f6_hz: number;
  flatness_db: number;
  max_spl_db: number;
  cost_usd: number;
  cost_complete: boolean;
  cost_summary: string;
  meets_target: boolean;
  shortfall: number;
  problems: string[];
  /** Olive preference rating when CTA-2034 curves were formed (README 2.6 / 11.2). */
  preference?: number | null;
};

type Variant = { label: string; score: Score; design: unknown; notes: string[] };
type MethodRow = Variant | { label: string; error: string };
type Reference = { cheap: Variant; reference: Variant | null; reference_error: string | null };
type Combo = { index: number; label: string };
type Candidate = { combo: number; summary: string; score: Score; design: unknown };
type ComboResult = {
  combo: Combo;
  points: Array<{ cost_usd: number; shortfall: number; meets_target: boolean }>;
  best: Candidate[];
  evaluations: number;
  notes: string[];
};

/** Per-job wall limit so a stuck Wasm call cannot leave the page spinning forever. */
const JOB_TIMEOUT_MS = 4 * 60 * 1000;

export function runJob(
  method: string,
  request: Record<string, unknown>,
  onProgress: (done: number, total: number) => void,
  workers: Worker[],
): Promise<string> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./jobWorker.ts", import.meta.url), { type: "module" });
    workers.push(worker);
    let settled = false;
    const finish = (fn: () => void) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      try {
        worker.terminate();
      } catch {
        /* already gone */
      }
      fn();
    };
    const timer = setTimeout(() => {
      finish(() => reject(new Error(`The job timed out after ${JOB_TIMEOUT_MS / 60000} minutes.`)));
    }, JOB_TIMEOUT_MS);
    worker.onmessage = (event: MessageEvent<JobMessage>) => {
      const message = event.data;
      if (message.type === "progress") {
        onProgress(message.done, message.total);
      } else if (message.type === "result") {
        finish(() => resolve(message.json));
      } else {
        finish(() => reject(new Error(message.message)));
      }
    };
    // Without this, a crashed or OOM-killed worker leaves the Promise pending forever —
    // Generate then never leaves "Proposing cabinets..." (Actions Check hang).
    worker.onerror = (event) => {
      finish(() => reject(new Error(event.message || "The job worker failed.")));
    };
    worker.onmessageerror = () => {
      finish(() => reject(new Error("The job worker returned a bad message.")));
    };
    worker.postMessage({ method, request });
  });
}

function money(score: Score) {
  return `${score.cost_complete ? "" : "at least "}$${score.cost_usd.toFixed(2)}`;
}

export function ScoreCells({ score }: { score: Score }) {
  const preference =
    score.preference != null && Number.isFinite(score.preference)
      ? score.preference.toFixed(2)
      : "—";
  return (
    <>
      <td>{preference}</td>
      <td>{score.f6_hz.toFixed(0)} Hz</td>
      <td>{score.flatness_db.toFixed(1)} dB</td>
      <td>{score.max_spl_db.toFixed(0)} dB</td>
      <td>{money(score)}</td>
      <td>{score.meets_target ? "Meets the target" : `Short by ${score.shortfall.toFixed(2)}`}</td>
    </>
  );
}

export const HEADER = (
  <tr>
    <th></th>
    <th title="Olive preference rating from CTA-2034 curves. Higher is better; the target asks for at least 5.0 by default.">
      Preference
    </th>
    <th title="Bass limit: the lowest frequency within 6 dB of the normal level.">Bass limit</th>
    <th title="Evenness: how closely the response follows the target.">Evenness</th>
    <th title="Maximum clean level: the loudest it plays without exceeding a limit, at 1 m.">Clean level</th>
    <th title="Cost of the whole system, from catalog prices.">Cost</th>
    <th title="Against the quality target: preference shortfall plus clean-level shortfall.">Target</th>
    <th></th>
  </tr>
);

/** Cost beside quality: reference build, build methods, and the optimizer. */
export function CostQuality({
  text,
  onOpen,
}: {
  text: string | null;
  onOpen: (design: unknown, label: string) => void;
}) {
  const workers = useRef<Worker[]>([]);
  const [note, setNote] = useState("");
  const [reference, setReference] = useState<Reference | null>(null);
  const [methods, setMethods] = useState<MethodRow[] | null>(null);
  const [results, setResults] = useState<ComboResult[]>([]);
  const [progress, setProgress] = useState<Record<number, string>>({});
  const [verified, setVerified] = useState("");

  const stop = () => {
    workers.current.forEach((worker) => worker.terminate());
    workers.current = [];
  };

  let hasPassive = false;
  if (text) {
    try {
      const parsed = JSON.parse(text) as { dsp?: { passive?: unknown } };
      hasPassive = parsed.dsp?.passive != null;
    } catch {
      hasPassive = false;
    }
  }

  const ranked = results
    .flatMap((result) => result.best)
    .sort((a, b) => rank(a.score) - rank(b.score))
    .slice(0, 8);
  const points = results.flatMap((result) => result.points);

  return (
    <section className="panel" data-testid="cost-quality">
      <h2>Cost and quality</h2>
      <p>
        Every score here is after correction, with the cost of the whole system beside it. The target is the
        design&apos;s quality target.
      </p>
      <div className="row">
        <button
          type="button"
          disabled={!text}
          title="The same layout with the closest quality parts and a box of the same air volume cut from plywood or MDF."
          onClick={() => {
            if (!text) {
              return;
            }
            setNote("Building and scoring the reference build…");
            runJob("compare.reference", { text }, () => {}, workers.current)
              .then((json) => {
                const value = JSON.parse(json) as Reference & { error?: string };
                if (value.error) {
                  setNote(`Reference build failed: ${value.error}`);
                  return;
                }
                setReference(value);
                setNote("");
              })
              .catch((error: Error) => setNote(`Reference build failed: ${error.message}`));
          }}
        >
          Compare with a reference build
        </button>
        <button
          type="button"
          disabled={!text}
          title="The same cabinet, same air volume, printed, cut from each sheet stock, or made from a pipe."
          onClick={() => {
            if (!text) {
              return;
            }
            setNote("Building the cabinet each way and scoring it…");
            runJob("compare.methods", { text }, () => {}, workers.current)
              .then((json) => {
                const value = JSON.parse(json) as MethodRow[] | { error: string };
                if (!Array.isArray(value)) {
                  setNote(`Build methods failed: ${value.error}`);
                  return;
                }
                setMethods(value);
                setNote("");
              })
              .catch((error: Error) => setNote(`Build methods failed: ${error.message}`));
          }}
        >
          Compare build methods
        </button>
        <button
          type="button"
          disabled={!text}
          data-testid="optimize-run"
          title={
            hasPassive
              ? "Keeps the cabinet and board wiring fixed, and searches resistor, inductor, and capacitor values toward the quality target."
              : "Searches box volume, port tuning, crossover, and levels — and for a one-driver cabinet, one nearby catalog driver — sealed and vented, and ranks the results: the cheapest that meets the target first."
          }
          onClick={() => {
            if (!text) {
              return;
            }
            stop();
            setResults([]);
            setProgress({});
            setVerified("");
            setNote(
              hasPassive
                ? "Tuning the passive board toward the target…"
                : "Finding the cheapest design that meets the target…",
            );
            void (async () => {
              const combosJson = await runJob("optimize.combos", { text }, () => {}, workers.current);
              const combos = JSON.parse(combosJson) as Combo[] | { error: string };
              if (!Array.isArray(combos)) {
                setNote(`The optimizer failed: ${combos.error}`);
                return;
              }
              const finished: ComboResult[] = [];
              // Cap parallel Wasm workers: each combo scores dozens of corrected designs.
              const evaluations = hasPassive
                ? 60
                : Math.max(12, Math.min(30, Math.floor(48 / Math.max(1, combos.length))));
              const concurrency = Math.min(2, combos.length);
              let next = 0;
              const runOne = async (): Promise<void> => {
                while (next < combos.length) {
                  const combo = combos[next];
                  next += 1;
                  try {
                    const json = await runJob(
                      "optimize.combo",
                      { text, combo: combo.index, seed: 1, evaluations },
                      (done, total) =>
                        setProgress((old) => ({
                          ...old,
                          [combo.index]: `${combo.label}: ${done} of ${total}`,
                        })),
                      workers.current,
                    );
                    const value = JSON.parse(json) as ComboResult & { error?: string };
                    if (value.error) {
                      setProgress((old) => ({
                        ...old,
                        [combo.index]: `${combo.label}: ${value.error}`,
                      }));
                      continue;
                    }
                    finished.push(value);
                    setResults([...finished]);
                    setProgress((old) => ({
                      ...old,
                      [combo.index]: `${combo.label}: done, ${value.evaluations} designs scored.`,
                    }));
                  } catch (error) {
                    setProgress((old) => ({
                      ...old,
                      [combo.index]: `${combo.label}: ${(error as Error).message}`,
                    }));
                  }
                }
              };
              await Promise.all(Array.from({ length: concurrency }, () => runOne()));
              const top = finished
                .flatMap((result) => result.best)
                .sort((a, b) => rank(a.score) - rank(b.score))[0];
              if (!top) {
                setNote("The optimizer gave no result.");
                return;
              }
              // Passive value search leaves the cabinet unchanged; skip the slow full-3D check.
              if (hasPassive) {
                setVerified(
                  `Passive board search kept the cabinet fixed. Best preference ${
                    top.score.preference != null && Number.isFinite(top.score.preference)
                      ? top.score.preference.toFixed(2)
                      : "—"
                  }, shortfall ${top.score.shortfall.toFixed(2)}.`,
                );
                setNote("The optimizer finished.");
                return;
              }
              // README 11.3: verify the finalist with the full 3D model and report what changed.
              setNote("Checking the top design in full 3D…");
              try {
                const json = await runJob(
                  "simulate.full3d",
                  { text: JSON.stringify(top.design), fdtd_dx_m: 0.01, radiation: "baffle" },
                  () => {},
                  workers.current,
                );
                const full = JSON.parse(json) as {
                  f6_hz: number;
                  flatness_db: number;
                  max_spl_metric_db: number;
                  error?: string;
                };
                if (full.error) {
                  setVerified(`The full 3D check could not run: ${full.error}`);
                } else {
                  setVerified(
                    `Full 3D check of the top design (10 mm cells): bass limit ${top.score.f6_hz.toFixed(0)} → ${full.f6_hz.toFixed(0)} Hz, evenness ${top.score.flatness_db.toFixed(1)} → ${full.flatness_db.toFixed(1)} dB, clean level ${top.score.max_spl_db.toFixed(0)} → ${full.max_spl_metric_db.toFixed(0)} dB.`,
                  );
                }
              } catch (error) {
                setVerified(`The full 3D check could not run: ${(error as Error).message}`);
              }
              setNote("The optimizer finished.");
            })();
          }}
        >
          {hasPassive ? "Tune the passive board" : "Find the cheapest design"}
        </button>
        <button
          type="button"
          onClick={() => {
            stop();
            setNote("Canceled.");
          }}
        >
          Cancel
        </button>
      </div>
      <p data-testid="cost-quality-note">{note}</p>

      {reference && (
        <table data-testid="reference-table">
          <thead>{HEADER}</thead>
          <tbody>
            {[reference.cheap, reference.reference].map((variant) =>
              variant ? (
                <tr key={variant.label} title={variant.notes.join(" ")}>
                  <th>{variant.label}</th>
                  <ScoreCells score={variant.score} />
                  <td>
                    <button type="button" onClick={() => onOpen(variant.design, variant.label)}>
                      Open
                    </button>
                  </td>
                </tr>
              ) : null,
            )}
          </tbody>
        </table>
      )}
      {reference?.reference_error ? <p>No reference build: {reference.reference_error}</p> : null}
      {reference?.reference?.notes.length ? <p>{reference.reference.notes.join(" ")}</p> : null}

      {methods && (
        <table data-testid="methods-table">
          <thead>{HEADER}</thead>
          <tbody>
            {methods.map((row) =>
              "error" in row ? (
                <tr key={row.label}>
                  <th>{row.label}</th>
                  <td colSpan={6}>Not possible: {row.error}</td>
                </tr>
              ) : (
                <tr key={row.label} title={row.notes.join(" ")}>
                  <th>{row.label}</th>
                  <ScoreCells score={row.score} />
                  <td>
                    <button type="button" onClick={() => onOpen(row.design, row.label)}>
                      Open
                    </button>
                  </td>
                </tr>
              ),
            )}
          </tbody>
        </table>
      )}
      {methods ? <p>Wall vibration is not simulated, so stiff and floppy materials score alike.</p> : null}

      {Object.keys(progress).length > 0 && (
        <ul data-testid="optimizer-progress">
          {Object.entries(progress).map(([key, line]) => (
            <li key={key}>{line}</li>
          ))}
        </ul>
      )}
      {ranked.length > 0 && (
        <>
          <table data-testid="optimizer-table">
            <thead>{HEADER}</thead>
            <tbody>
              {ranked.map((candidate, index) => (
                <tr key={`${candidate.summary}-${index}`}>
                  <th>{candidate.summary}</th>
                  <ScoreCells score={candidate.score} />
                  <td>
                    <button type="button" onClick={() => onOpen(candidate.design, `optimized-${index + 1}`)}>
                      Open
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {verified ? <p data-testid="optimizer-verified">{verified}</p> : null}
          <CostPlot points={points} />
          {results.flatMap((result) => result.notes).length > 0 ? (
            <p>{[...new Set(results.flatMap((result) => result.notes))].join(" ")}</p>
          ) : null}
        </>
      )}
    </section>
  );
}

export function rank(score: Score) {
  if (score.problems.length > 0) {
    return 1e12;
  }
  return score.meets_target ? score.cost_usd : 1e6 + 1e3 * score.shortfall + score.cost_usd * 1e-3;
}

/** Cost against quality: every scored candidate. Lower is better on both axes. */
export function CostPlot({ points, caption }: {
  points: Array<{ cost_usd: number; shortfall: number; meets_target: boolean }>;
  caption?: string;
}) {
  const width = 520;
  const height = 240;
  const pad = 44;
  const usable = points.filter((p) => Number.isFinite(p.cost_usd) && Number.isFinite(p.shortfall) && p.shortfall < 50);
  if (usable.length === 0) {
    return null;
  }
  const maxCost = Math.max(...usable.map((p) => p.cost_usd), 1) * 1.05;
  const minCost = Math.min(...usable.map((p) => p.cost_usd)) * 0.95;
  const maxShort = Math.max(...usable.map((p) => p.shortfall), 0.5) * 1.05;
  const x = (cost: number) => pad + ((cost - minCost) / Math.max(maxCost - minCost, 1e-6)) * (width - pad - 10);
  const y = (shortfall: number) => height - pad + 10 - (shortfall / maxShort) * (height - pad);
  return (
    <figure>
      <figcaption>
        {caption ?? "Cost against quality: every design the optimizer scored. Lower and further left is better."}
      </figcaption>
      <svg data-testid="cost-plot" width={width} height={height} role="img" aria-label="Cost against shortfall">
        <line x1={pad} y1={height - pad + 10} x2={width - 10} y2={height - pad + 10} stroke="currentColor" />
        <line x1={pad} y1={10} x2={pad} y2={height - pad + 10} stroke="currentColor" />
        <text x={width / 2} y={height - 4} textAnchor="middle" fontSize="12">
          Cost of the system ($)
        </text>
        <text x={12} y={height / 2} fontSize="12" transform={`rotate(-90 12 ${height / 2})`} textAnchor="middle">
          Shortfall from target
        </text>
        <text x={pad} y={height - pad + 24} fontSize="11">
          {minCost.toFixed(0)}
        </text>
        <text x={width - 10} y={height - pad + 24} fontSize="11" textAnchor="end">
          {maxCost.toFixed(0)}
        </text>
        {usable.map((p, index) => (
          <circle
            key={index}
            cx={x(p.cost_usd)}
            cy={y(p.shortfall)}
            r={3}
            fill={p.meets_target ? "seagreen" : "sienna"}
            opacity={0.7}
          />
        ))}
      </svg>
    </figure>
  );
}
