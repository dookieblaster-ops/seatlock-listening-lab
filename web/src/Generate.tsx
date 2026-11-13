import { useRef, useState } from "react";

import { CostPlot, HEADER, rank, runJob, ScoreCells, type Score } from "./CostQuality";

type Family = { index: number; label: string; outer: string; layout: string; face: string };
type Generated = { family: number; summary: string; score: Score; design: unknown };
type FamilyResult = {
  family: Family;
  points: Array<{ cost_usd: number; shortfall: number; meets_target: boolean }>;
  best: Generated[];
  evaluations: number;
  thrown_out: number;
  notes: string[];
};
type Checked = { f6_hz: number; flatness_db: number; max_spl_metric_db: number; error?: string };

/**
 * "Generate cabinets" (README section 11.4): the lab proposes whole cabinets (outer form,
 * inside layout, driver, where it sits and which way it faces) and ranks them.
 */
export function Generate({ text, onOpen }: { text: string; onOpen: (design: unknown, label: string) => void }) {
  const workers = useRef<Worker[]>([]);
  const [note, setNote] = useState("");
  const [results, setResults] = useState<FamilyResult[]>([]);
  const [progress, setProgress] = useState<Record<number, string>>({});
  const [verified, setVerified] = useState("");
  const [busy, setBusy] = useState(false);

  const stop = () => {
    workers.current.forEach((worker) => worker.terminate());
    workers.current = [];
    setBusy(false);
  };

  const start = () => {
    stop();
    setResults([]);
    setProgress({});
    setVerified("");
    setBusy(true);
    setNote("Proposing cabinets and scoring them...");
    void (async () => {
      const listJson = await runJob("generate.families", { text }, () => {}, workers.current);
      const families = JSON.parse(listJson) as Family[] | { error: string };
      if (!Array.isArray(families)) {
        setNote(`The generator failed: ${families.error}`);
        setBusy(false);
        return;
      }
      const finished: FamilyResult[] = [];
      const queue = [...families];
      const lanes = Math.max(2, Math.min(6, (navigator.hardwareConcurrency || 4) - 1));
      const lane = async () => {
        for (let family = queue.shift(); family; family = queue.shift()) {
          const { index, label } = family;
          try {
            const json = await runJob(
              "generate.family",
              { text, family: index, seed: 1, evaluations: 12 },
              (done, total) => setProgress((old) => ({ ...old, [index]: `${label}: ${done} of ${total}` })),
              workers.current,
            );
            const value = JSON.parse(json) as FamilyResult & { error?: string };
            if (value.error) {
              setProgress((old) => ({ ...old, [index]: `${label}: ${value.error}` }));
              continue;
            }
            finished.push(value);
            setResults([...finished]);
            setProgress((old) => ({
              ...old,
              [index]: `${label}: done, ${value.evaluations} scored, ${value.thrown_out} could not be built.`,
            }));
          } catch {
            return;
          }
        }
      };
      await Promise.all(Array.from({ length: lanes }, lane));
      const top = finished.flatMap((result) => result.best).sort((a, b) => rank(a.score) - rank(b.score))[0];
      if (!top) {
        setNote("The generator gave no result.");
        setBusy(false);
        return;
      }
      // README 11.4: check the best with the 3D models and say what changed.
      const family = families[top.family];
      const outside = family.face !== "front";
      setNote(outside ? "Checking the top cabinet with the outside in 3D..." : "Checking the top cabinet in full 3D...");
      try {
        const request = outside
          ? { text: JSON.stringify(top.design), enclosure: "waveguide", radiation: "exterior", exterior_dx_m: 0.01 }
          : { text: JSON.stringify(top.design), fdtd_dx_m: 0.01, radiation: "baffle" };
        const json = await runJob("simulate.full3d", request, () => {}, workers.current);
        const full = JSON.parse(json) as Checked;
        setVerified(
          full.error
            ? `The 3D check could not run: ${full.error}`
            : `3D check of the top cabinet (${outside ? "the air outside" : "the air inside"}, 10 mm cells): bass limit ${top.score.f6_hz.toFixed(0)} → ${full.f6_hz.toFixed(0)} Hz, evenness ${top.score.flatness_db.toFixed(1)} → ${full.flatness_db.toFixed(1)} dB, clean level ${top.score.max_spl_db.toFixed(0)} → ${full.max_spl_metric_db.toFixed(0)} dB.`,
        );
      } catch (error) {
        setVerified(`The 3D check could not run: ${(error as Error).message}`);
      }
      setNote("The generator finished.");
      setBusy(false);
    })();
  };

  const ranked = results
    .flatMap((result) => result.best)
    .sort((a, b) => rank(a.score) - rank(b.score))
    .slice(0, 12);
  const notes = [...new Set(results.flatMap((result) => result.notes.slice(0, 2)))];
  return (
    <section className="panel" data-testid="generate">
      <h2>Generate cabinets</h2>
      <p>
        The lab proposes whole cabinets: the outer form, the inside (sealed, ported, or a folded line), a driver from
        the catalog, where it sits, and which way it faces. Each is corrected and scored like any design, and the
        closest to the quality target for the least money comes first. The settings of the design that is open
        (target, room, amplifier, print material) are kept.
      </p>
      <div className="row">
        <button
          type="button"
          data-testid="generate-start"
          disabled={!text || busy}
          title="Searches every outer form the lab can print, with each inside layout and each real driver it can mount."
          onClick={start}
        >
          Generate cabinets
        </button>
        <button
          type="button"
          disabled={!busy}
          onClick={() => {
            stop();
            setNote("Canceled.");
          }}
        >
          Cancel
        </button>
      </div>
      <p data-testid="generate-note">{note}</p>
      {Object.keys(progress).length > 0 && (
        <ul data-testid="generate-progress">
          {Object.entries(progress).map(([key, line]) => (
            <li key={key}>{line}</li>
          ))}
        </ul>
      )}
      {ranked.length > 0 && (
        <>
          <table data-testid="generate-table">
            <thead>{HEADER}</thead>
            <tbody>
              {ranked.map((candidate, index) => (
                <tr key={`${candidate.summary}-${index}`}>
                  <th>{candidate.summary}</th>
                  <ScoreCells score={candidate.score} />
                  <td>
                    <button type="button" onClick={() => onOpen(candidate.design, `generated-${index + 1}`)}>
                      Open
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {verified ? <p data-testid="generate-verified">{verified}</p> : null}
          <CostPlot
            points={results.flatMap((result) => result.points)}
            caption="Cost against quality: every cabinet the generator scored. Lower and further left is better."
          />
          {notes.length > 0 ? <p>{notes.join(" ")}</p> : null}
        </>
      )}
    </section>
  );
}
