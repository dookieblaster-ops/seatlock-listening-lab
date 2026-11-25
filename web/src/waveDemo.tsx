import { StrictMode, useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { WaveView, type WaveFieldReport } from "./WaveView";
import "./App.css";

const DOMAINS = ["inside", "outside", "room"] as const;

/** Outer shell for sealed-test (0.16 × 0.18 × 0.25 m), matching the fixture. */
function sealedTestShell() {
  const w = 0.16;
  const d = 0.18;
  const h = 0.25;
  const positions = new Float32Array([
    0, 0, 0, w, 0, 0, w, d, 0, 0, d, 0, 0, 0, h, w, 0, h, w, d, h, 0, d, h,
  ]);
  const indices = new Uint32Array([
    0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6, 0, 4, 5, 0, 5, 1, 1, 5, 6, 1, 6, 2, 2, 6, 7, 2, 7, 3, 3, 7, 4,
    3, 4, 0,
  ]);
  return { positions, indices };
}

function Demo() {
  const [domain, setDomain] = useState<(typeof DOMAINS)[number]>("inside");
  const [report, setReport] = useState<WaveFieldReport | null>(null);
  const [error, setError] = useState("");
  const cabinet = useMemo(() => sealedTestShell(), []);

  useEffect(() => {
    let canceled = false;
    setReport(null);
    setError("");
    void fetch(`/wave-demo/wave-${domain}.json`)
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`missing fixture wave-${domain}.json`);
        }
        return (await response.json()) as WaveFieldReport;
      })
      .then((next) => {
        if (!canceled) setReport(next);
      })
      .catch((err: Error) => {
        if (!canceled) setError(err.message);
      });
    return () => {
      canceled = true;
    };
  }, [domain]);

  return (
    <div className="app" style={{ maxWidth: "48rem" }}>
      <h1>Wave view demo</h1>
      <p className="lede">Local fixture preview for the audiophile 3D sound graphics.</p>
      <div className="row">
        {DOMAINS.map((item) => (
          <button
            key={item}
            type="button"
            data-testid={`demo-${item}`}
            className={domain === item ? "active" : undefined}
            onClick={() => setDomain(item)}
          >
            {item}
          </button>
        ))}
      </div>
      {error ? <p className="error">{error}</p> : null}
      {!report && !error ? <p>Loading {domain} field…</p> : null}
      {report ? (
        <WaveView report={report} cabinet={domain === "room" ? null : cabinet} />
      ) : null}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Demo />
  </StrictMode>,
);
