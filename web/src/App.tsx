import { useEffect, useRef, useState } from "react";

import { CabinetView } from "./CabinetView";
import { CostPanel } from "./CostPanel";
import type { Issue } from "./generated/Issue";
import { displayGrid, Graph, nearestValue } from "./Graph";
import { buildCabinetMesh, meshTo3mf, meshToStl, type SolidMesh } from "./printSolid";
import { useEngine, type GeometryReport, type SimulateResult } from "./useEngine";
import { coefficientMessage, useListen, type ListenSource } from "./useListen";
import "./App.css";

type Design = {
  name: string;
  speaker: {
    enclosure: {
      outer: Record<string, number | string>;
      chambers: unknown[];
      ducts: unknown[];
      mounts: Array<Record<string, unknown>>;
      solids: unknown[];
    };
    radiators: Array<Record<string, unknown>>;
    listening: { distance_m: number };
  };
  dsp: {
    sample_rate_hz: number;
    whole_speaker: unknown[];
    per_radiator: Array<{
      blocks: Array<Record<string, unknown>>;
      delay_s: number;
      gain_db: number;
      invert: boolean;
      limiter: unknown;
    }>;
    correction: { enabled: boolean; protected_band_hz: number[]; max_filters: number };
  };
  electronics: { amplifiers: string[]; dsp_device: string | null };
  build: { material: string; infill_fraction: number; stereo_pair: boolean; extras_cost: number };
  target: string;
  [key: string]: unknown;
};

const grid = displayGrid();

function mm(meters: number) {
  return Math.round(meters * 1000 * 1000) / 1000;
}

function fidelityText(label: SimulateResult["fidelity"]) {
  const ignored = label.ignored.length ? ` Left out: ${label.ignored.join("; ")}.` : "";
  return `Model: ${label.enclosure} box, ${label.radiation} radiation, ${label.room} room.${ignored}`;
}

export default function App() {
  const { version, ready, error: engineError, designCheck, simulate, geometry, correct, listen, driverRecord } = useEngine();
  const playback = useListen();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const audioFileRef = useRef<HTMLInputElement>(null);
  const compareInputRef = useRef<HTMLInputElement>(null);
  const revision = useRef(0);

  const [designName, setDesignName] = useState("No design loaded");
  const [design, setDesign] = useState<Design | null>(null);
  const [canonical, setCanonical] = useState<string | null>(null);
  const [issues, setIssues] = useState<Issue[]>([]);
  const [status, setStatus] = useState("");
  const [sim, setSim] = useState<SimulateResult | null>(null);
  const [simIssues, setSimIssues] = useState<SimulateResult["issues"]>([]);
  const [geom, setGeom] = useState<GeometryReport | null>(null);
  const [solid, setSolid] = useState<SolidMesh | null>(null);
  const [fdtdNote, setFdtdNote] = useState("The full 3D air model has not been run.");
  const [fdtdWarning, setFdtdWarning] = useState("");
  const [correctionNote, setCorrectionNote] = useState("");
  const [sweep, setSweep] = useState<Array<{ volume: number; bass: number }>>([]);
  const [filterText, setFilterText] = useState("");
  const [source, setSource] = useState<ListenSource>("pink");
  const [volts, setVolts] = useState(2.83);
  const [hearCorrection, setHearCorrection] = useState(true);
  const [audioFile, setAudioFile] = useState<ArrayBuffer | null>(null);
  const [driverLines, setDriverLines] = useState<string[]>([]);
  const [simB, setSimB] = useState<SimulateResult | null>(null);
  const [nameB, setNameB] = useState("");
  const [canonicalB, setCanonicalB] = useState<string | null>(null);
  const [fdtdCurve, setFdtdCurve] = useState<{
    freq: number[];
    mag: number[];
    lumped: number[];
    ignored: string[];
  } | null>(null);
  const fdtdWorker = useRef<Worker | null>(null);

  async function publish(next: Design, label: string) {
    const rev = ++revision.current;
    setDesign(next);
    setDesignName(label);
    const checked = await designCheck(JSON.stringify(next));
    if (rev !== revision.current) {
      return;
    }
    setCanonical(checked.canonical_json);
    setIssues(checked.issues);
    setStatus(
      checked.issues.length === 0
        ? "Design checked with no warnings."
        : `${checked.issues.length} notice(s) from the engine.`,
    );
    const result = await simulate(checked.canonical_json, rev, grid);
    if (rev !== revision.current || result.revision !== rev) {
      return;
    }
    if (result.error) {
      setStatus(result.error);
      return;
    }
    setSim(result);
    setSimIssues(result.issues);
    const report = await geometry(checked.canonical_json);
    if (rev !== revision.current || report.error) {
      return;
    }
    setGeom(report);
    const mesh = await buildCabinetMesh(report);
    if (rev !== revision.current) {
      return;
    }
    setSolid(mesh);
  }

  async function onOpenFile(file: File) {
    const text = await file.text();
    const checked = await designCheck(text);
    await publish(JSON.parse(checked.canonical_json) as Design, file.name);
  }

  function onSave() {
    if (!canonical) {
      return;
    }
    const blob = new Blob([canonical], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = designName.endsWith(".seatlock.json") ? designName : "design.seatlock.json";
    anchor.click();
    URL.revokeObjectURL(url);
  }

  async function loadExample() {
    const response = await fetch("/examples/sealed-test.seatlock.json");
    const text = await response.text();
    const checked = await designCheck(text);
    await publish(JSON.parse(checked.canonical_json) as Design, "sealed-test.seatlock.json");
  }

  function updateNumber(path: string, value: number) {
    if (!design || Number.isNaN(value)) {
      return;
    }
    const next = structuredClone(design);
    const keys = path.split(".");
    let cursor: Record<string, unknown> = next;
    for (let i = 0; i < keys.length - 1; i += 1) {
      cursor = cursor[keys[i]] as Record<string, unknown>;
    }
    cursor[keys[keys.length - 1]] = value;
    void publish(next, designName);
  }

  function updateJson(path: string, raw: string) {
    if (!design) {
      return;
    }
    try {
      const value = JSON.parse(raw) as unknown;
      const next = structuredClone(design);
      const keys = path.split(".");
      let cursor: Record<string, unknown> = next;
      for (let i = 0; i < keys.length - 1; i += 1) {
        cursor = cursor[keys[i]] as Record<string, unknown>;
      }
      cursor[keys[keys.length - 1]] = value;
      void publish(next, designName);
    } catch {
      setStatus("That list is not valid JSON yet.");
    }
  }

  const outer = design?.speaker.enclosure.outer;
  const mount = design?.speaker.enclosure.mounts[0];
  const radiator = design?.speaker.radiators[0];
  const chain = design?.dsp.per_radiator[0];
  const block = chain?.blocks[0];
  useEffect(() => {
    const ids = design?.speaker.radiators.map((item) => String(item.driver_id ?? "")) ?? [];
    if (ids.length === 0) {
      setDriverLines([]);
      return;
    }
    let cancel = false;
    void Promise.all(ids.map((id) => driverRecord(id).catch(() => ({ error: id })))).then((records) => {
      if (cancel) {
        return;
      }
      setDriverLines(
        records.map((record, index) => {
          if ("error" in record || record.status === "synthetic" || String(ids[index]).startsWith("test-")) {
            return `Driver ${index + 1}: Test driver, not a real product`;
          }
          const readOn = record.price?.read_on;
          const stale =
            readOn !== undefined && Date.now() - Date.parse(readOn) > 90 * 24 * 60 * 60 * 1000;
          const price = record.price?.amount;
          const money =
            price === undefined
              ? "price unknown"
              : `${record.price?.vendor ?? "vendor"} $${price} as of ${readOn ?? "an unknown date"}`;
          const tier = record.tier ? `${record.tier} ` : "";
          const unconfirmed = record.price?.via || record.mounting?.via ? " unconfirmed" : "";
          return `Driver ${index + 1}: ${tier}${record.maker ?? ""} ${record.model ?? ids[index]}. ${money}${unconfirmed}${stale ? ". This price is more than 90 days old." : ""}`;
        }),
      );
    });
    return () => {
      cancel = true;
    };
  }, [design, driverRecord]);

  const curve = sim?.radiators[0];
  const spl40 = curve ? nearestValue(sim.frequencies_hz, curve.spl_db, 40) : Number.NaN;
  const voltageDb = 20 * Math.log10(volts / 2.83);
  const levelAtOneMeter =
    curve && sim ? nearestValue(sim.frequencies_hz, curve.spl_db, 1000) + voltageDb : null;
  const limitHot = curve
    ? curve.spl_db.some((level, index) => level + voltageDb > (curve.max_spl_db[index] ?? Infinity) + 0.05)
    : false;

  return (
    <main className="app">
      <header>
        <h1>Seatlock Listening Lab</h1>
        <p className="lede">Try a sealed box with one driver before you build it.</p>
      </header>

      <section className="panel">
        <h2>Engine</h2>
        <p data-testid="engine-version">
          Version <strong>{ready ? version : "loading…"}</strong>
        </p>
        {engineError && <p className="error">{engineError}</p>}
      </section>

      <section className="panel">
        <h2>Design file</h2>
        <div className="row">
          <button type="button" onClick={() => fileInputRef.current?.click()}>
            Open design…
          </button>
          <input
            ref={fileInputRef}
            type="file"
            accept=".json,.seatlock.json,application/json"
            hidden
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) {
                void onOpenFile(file);
              }
            }}
          />
          <button type="button" onClick={() => void loadExample()} data-testid="load-example">
            Load example sealed box
          </button>
          <button type="button" onClick={onSave} disabled={!canonical}>
            Save design
          </button>
        </div>
        <p>
          Current file: <span data-testid="design-name">{designName}</span>
        </p>
        <p data-testid="design-status">{status}</p>
        {issues.length > 0 && (
          <ul className="issues" data-testid="issue-list">
            {issues.map((issue) => (
              <li key={`${issue.code}-${issue.field}`}>
                <span className="issue-field">{issue.field || "(whole file)"}</span>: {issue.message}
              </li>
            ))}
          </ul>
        )}
      </section>

      {design && outer && mount && radiator && chain && (
        <section className="panel">
          <h2>Speaker</h2>
          <label>
            Name
            <input value={design.name} onChange={(event) => {
              const next = structuredClone(design);
              next.name = event.target.value;
              void publish(next, designName);
            }} />
          </label>
          <div className="grid">
            <NumberField label="Width (mm)" testId="box-width-mm" value={mm(Number(outer.width_m))} onChange={(value) => updateNumber("speaker.enclosure.outer.width_m", value / 1000)} />
            <NumberField label="Height (mm)" value={mm(Number(outer.height_m))} onChange={(value) => updateNumber("speaker.enclosure.outer.height_m", value / 1000)} />
            <NumberField label="Depth (mm)" value={mm(Number(outer.depth_m))} onChange={(value) => updateNumber("speaker.enclosure.outer.depth_m", value / 1000)} />
            <NumberField label="Wall thickness (mm)" value={mm(Number(outer.wall_m))} onChange={(value) => updateNumber("speaker.enclosure.outer.wall_m", value / 1000)} />
            <NumberField label="Edge radius (mm)" value={mm(Number(outer.edge_radius_m ?? 0))} onChange={(value) => updateNumber("speaker.enclosure.outer.edge_radius_m", value / 1000)} />
            <NumberField label="Listening distance (m)" value={design.speaker.listening.distance_m} onChange={(value) => updateNumber("speaker.listening.distance_m", value)} />
          </div>
          <h3>Driver opening</h3>
          <div className="grid">
            <TextField label="Opening name" value={String(mount.id)} onChange={(value) => {
              const next = structuredClone(design);
              next.speaker.enclosure.mounts[0].id = value;
              void publish(next, designName);
            }} />
            <TextField label="Face" value={String(mount.surface)} onChange={(value) => {
              const next = structuredClone(design);
              next.speaker.enclosure.mounts[0].surface = value;
              void publish(next, designName);
            }} />
            <NumberField label="Across the face (mm)" value={mm((mount.at_m as number[])[0])} onChange={(value) => {
              const next = structuredClone(design);
              (next.speaker.enclosure.mounts[0].at_m as number[])[0] = value / 1000;
              void publish(next, designName);
            }} />
            <NumberField label="Up the face (mm)" value={mm((mount.at_m as number[])[1])} onChange={(value) => {
              const next = structuredClone(design);
              (next.speaker.enclosure.mounts[0].at_m as number[])[1] = value / 1000;
              void publish(next, designName);
            }} />
            <TextField label="Air behind the driver" value={String(mount.rear_into)} onChange={(value) => {
              const next = structuredClone(design);
              next.speaker.enclosure.mounts[0].rear_into = value;
              void publish(next, designName);
            }} />
            <NumberField label="Recess (mm)" value={mm(Number(mount.recess_m ?? 0))} onChange={(value) => updateNumber("speaker.enclosure.mounts.0.recess_m", value / 1000)} />
          </div>
          <h3>Driver</h3>
          <div className="grid">
            <TextField label="Kind of sound source" value={String(radiator.kind)} onChange={(value) => {
              const next = structuredClone(design);
              next.speaker.radiators[0].kind = value;
              void publish(next, designName);
            }} />
            <TextField label="Driver" value={String(radiator.driver_id)} onChange={(value) => {
              const next = structuredClone(design);
              next.speaker.radiators[0].driver_id = value;
              void publish(next, designName);
            }} />
            <TextField label="Opening used" value={String(radiator.mount)} onChange={(value) => {
              const next = structuredClone(design);
              next.speaker.radiators[0].mount = value;
              void publish(next, designName);
            }} />
          </div>
          {design.speaker.radiators.map((item, index) => (
            <label key={`driver-${index}`}>
              Driver {index + 1} catalog id
              <input
                data-testid={`driver-id-${index}`}
                value={String(item.driver_id ?? "")}
                onChange={(event) => {
                  const next = structuredClone(design);
                  next.speaker.radiators[index].driver_id = event.target.value;
                  void publish(next, designName);
                }}
              />
            </label>
          ))}
          {driverLines.map((line) => (
            <p key={line} data-testid="driver-note">{line}</p>
          ))}
          <h3>Processing</h3>
          <div className="grid">
            <NumberField label="Sample rate (Hz)" value={design.dsp.sample_rate_hz} onChange={(value) => updateNumber("dsp.sample_rate_hz", value)} />
            <TextField label="Filter type" value={String(block?.type ?? "")} onChange={(value) => {
              const next = structuredClone(design);
              next.dsp.per_radiator[0].blocks[0].type = value;
              void publish(next, designName);
            }} />
            <NumberField label="Filter frequency (Hz)" value={Number(block?.f_hz ?? 0)} onChange={(value) => updateNumber("dsp.per_radiator.0.blocks.0.f_hz", value)} />
            <NumberField label="Filter sharpness (Q)" value={Number(block?.q ?? 0.707)} onChange={(value) => updateNumber("dsp.per_radiator.0.blocks.0.q", value)} />
            <NumberField label="Delay (ms)" value={chain.delay_s * 1000} onChange={(value) => updateNumber("dsp.per_radiator.0.delay_s", value / 1000)} />
            <NumberField label="Gain (dB)" value={chain.gain_db} onChange={(value) => updateNumber("dsp.per_radiator.0.gain_db", value)} />
          </div>
          <label className="check">
            <input type="checkbox" checked={chain.invert} onChange={(event) => {
              const next = structuredClone(design);
              next.dsp.per_radiator[0].invert = event.target.checked;
              void publish(next, designName);
            }} />
            Flip polarity
          </label>
          <label className="check">
            <input type="checkbox" checked={design.dsp.correction.enabled} onChange={(event) => {
              const next = structuredClone(design);
              next.dsp.correction.enabled = event.target.checked;
              if (!event.target.checked) {
                for (const chain of next.dsp.per_radiator) {
                  chain.blocks = chain.blocks.filter((block) => block.correction !== true);
                }
              }
              void publish(next, designName);
            }} />
            Correction enabled
          </label>
          <div className="grid">
            <NumberField label="Protected band start (Hz)" value={design.dsp.correction.protected_band_hz[0]} onChange={(value) => {
              const next = structuredClone(design);
              next.dsp.correction.protected_band_hz[0] = value;
              void publish(next, designName);
            }} />
            <NumberField label="Protected band end (Hz)" value={design.dsp.correction.protected_band_hz[1]} onChange={(value) => {
              const next = structuredClone(design);
              next.dsp.correction.protected_band_hz[1] = value;
              void publish(next, designName);
            }} />
            <NumberField label="Maximum correction filters" value={design.dsp.correction.max_filters} onChange={(value) => updateNumber("dsp.correction.max_filters", value)} />
          </div>
          <label>
            Chambers (JSON)
            <textarea value={JSON.stringify(design.speaker.enclosure.chambers)} onChange={(event) => updateJson("speaker.enclosure.chambers", event.target.value)} />
          </label>
          <label>
            Ducts (JSON)
            <textarea value={JSON.stringify(design.speaker.enclosure.ducts)} onChange={(event) => updateJson("speaker.enclosure.ducts", event.target.value)} />
          </label>
          <label>
            Bracing (JSON)
            <textarea value={JSON.stringify(design.speaker.enclosure.solids)} onChange={(event) => updateJson("speaker.enclosure.solids", event.target.value)} />
          </label>
          <label>
            Whole-speaker processing (JSON)
            <textarea value={JSON.stringify(design.dsp.whole_speaker)} onChange={(event) => updateJson("dsp.whole_speaker", event.target.value)} />
          </label>
          <label>
            Limiter (JSON, or null)
            <textarea value={JSON.stringify(chain.limiter)} onChange={(event) => updateJson("dsp.per_radiator.0.limiter", event.target.value)} />
          </label>
          <h3>Build</h3>
          <div className="grid">
            <TextField label="Amplifiers" value={design.electronics.amplifiers.join(", ")} onChange={(value) => {
              const next = structuredClone(design);
              next.electronics.amplifiers = value.split(",").map((item) => item.trim()).filter(Boolean);
              void publish(next, designName);
            }} />
            <TextField label="DSP device" value={design.electronics.dsp_device ?? ""} onChange={(value) => {
              const next = structuredClone(design);
              next.electronics.dsp_device = value === "" ? null : value;
              void publish(next, designName);
            }} />
            <TextField label="Material" value={design.build.material} onChange={(value) => {
              const next = structuredClone(design);
              next.build.material = value;
              void publish(next, designName);
            }} />
            <NumberField label="Infill (0 to 1)" value={design.build.infill_fraction} onChange={(value) => updateNumber("build.infill_fraction", value)} />
            <NumberField label="Extras cost" value={design.build.extras_cost} onChange={(value) => updateNumber("build.extras_cost", value)} />
            <TextField label="Target" value={design.target} onChange={(value) => {
              const next = structuredClone(design);
              next.target = value;
              void publish(next, designName);
            }} />
          </div>
          <label className="check">
            <input type="checkbox" checked={design.build.stereo_pair} onChange={(event) => {
              const next = structuredClone(design);
              next.build.stereo_pair = event.target.checked;
              void publish(next, designName);
            }} />
            Stereo pair
          </label>
        </section>
      )}

      {geom && solid && (
        <section className="panel">
          <h2>Cabinet</h2>
          <p>
            Air volume <span data-testid="formula-air">{geom.interior_volume_m3.toExponential(6)}</span> m³.
            Print solid <span data-testid="solid-volume">{solid.solidVolumeM3.toExponential(6)}</span> m³ implies{" "}
            <span data-testid="print-air">
              {(
                geom.width_m * geom.depth_m * geom.height_m -
                solid.solidVolumeM3 -
                geom.mounts.reduce(
                  (sum, mount) => sum + Math.PI * (mount.cutout_diameter_m / 2) ** 2 * geom.wall_m,
                  0,
                )
              ).toExponential(6)}
            </span>{" "}
            m³. 5 mm cells <span data-testid="voxel-air">{geom.voxel_air_volume_m3.toExponential(6)}</span> m³.
          </p>
          <CostPanel cost={geom.cost} printNote={geom.print_note} />
          <CabinetView
            positions={solid.positions}
            indices={solid.indices}
            driver={geom.mounts[0]?.position_m ?? null}
          />
          <div className="row">
            {geom.warnings?.map((line) => (
              <p key={line} data-testid="mount-warning">{line}</p>
            ))}
            <button
              type="button"
              data-testid="export-stl"
              disabled={geom.can_export_print === false}
              onClick={() => {
                if (geom.can_export_print === false) {
                  return;
                }
                const bytes = meshToStl(solid.positions, solid.indices);
                const blob = new Blob([bytes], { type: "model/stl" });
                const url = URL.createObjectURL(blob);
                const anchor = document.createElement("a");
                anchor.href = url;
                anchor.download = "cabinet.stl";
                anchor.click();
                URL.revokeObjectURL(url);
              }}
            >
              Export print file
            </button>
            <button
              type="button"
              data-testid="export-3mf"
              disabled={geom.can_export_print === false}
              onClick={() => {
                if (geom.can_export_print === false) {
                  return;
                }
                const bytes = meshTo3mf(solid.positions, solid.indices);
                const copy = new ArrayBuffer(bytes.byteLength);
                new Uint8Array(copy).set(bytes);
                const blob = new Blob([copy], { type: "model/3mf" });
                const url = URL.createObjectURL(blob);
                const anchor = document.createElement("a");
                anchor.href = url;
                anchor.download = "cabinet.3mf";
                anchor.click();
                URL.revokeObjectURL(url);
              }}
            >
              Export 3MF
            </button>
            <button
              type="button"
              onClick={() => {
                fdtdWorker.current?.terminate();
                const worker = new Worker(new URL("./fdtdWorker.ts", import.meta.url), { type: "module" });
                fdtdWorker.current = worker;
                setFdtdCurve(null);
                setFdtdWarning("");
                setFdtdNote("Solving the air inside the box…");
                const mount = geom.mounts[0];
                worker.onmessage = (event) => {
                  if (event.data.type === "done") {
                    const curve = JSON.parse(event.data.json) as {
                      elapsed_ms: number;
                      error?: string;
                      warning?: string;
                      ignored?: string[];
                      frequencies_hz?: number[];
                      real?: number[];
                      imag?: number[];
                    };
                    setFdtdNote(
                      curve.error
                        ? curve.error
                        : `Full 3D finished in ${(curve.elapsed_ms / 1000).toFixed(1)} s.`,
                    );
                    setFdtdWarning(curve.warning ?? "");
                    if (curve.frequencies_hz && curve.real && curve.imag) {
                      const compliance = geom.interior_volume_m3 / (1.204 * 343.2 * 343.2);
                      setFdtdCurve({
                        freq: curve.frequencies_hz,
                        mag: curve.real.map((re, index) => Math.hypot(re, curve.imag?.[index] ?? 0)),
                        lumped: curve.frequencies_hz.map((freq) => 1 / (2 * Math.PI * freq * compliance)),
                        ignored: curve.ignored ?? [],
                      });
                    }
                  }
                };
                worker.postMessage({
                  depth_m: geom.depth_m - 2 * geom.wall_m,
                  width_m: geom.width_m - 2 * geom.wall_m,
                  height_m: geom.height_m - 2 * geom.wall_m,
                  dx: 0.01,
                  center_y_m: (mount?.position_m[0] ?? 0) - geom.wall_m,
                  center_z_m: (mount?.position_m[2] ?? 0) - geom.wall_m,
                  radius_m: (mount?.cutout_diameter_m ?? 0.04) / 2,
                });
              }}
            >
              Run full 3D
            </button>
            <button
              type="button"
              onClick={() => {
                fdtdWorker.current?.terminate();
                fdtdWorker.current = null;
                setFdtdNote("Full 3D canceled.");
              }}
            >
              Cancel full 3D
            </button>
          </div>
          <p data-testid="fdtd-note">{fdtdNote}</p>
          {fdtdNote.includes("unreachable") || fdtdNote.includes("RuntimeError") ? (
            <p data-testid="fdtd-error">{fdtdNote}</p>
          ) : null}
          {fdtdWarning ? <p data-testid="fdtd-warning">{fdtdWarning}</p> : null}
          {fdtdCurve && (
            <Graph
              title="Air load"
              explanation="How the air in the box pushes back. Green is the quick model, brown is the full 3D model."
              freqs={fdtdCurve.freq}
              values={fdtdCurve.lumped}
              overlay={fdtdCurve.mag}
              unit="Pa·s/m³"
              testId="impedance-3d-graph"
              fidelity={`Quick sealed box and full 3D, rigid walls. ${fdtdCurve.ignored.join(" ")}`}
            />
          )}
        </section>
      )}

      {curve && sim && (
        <section className="panel">
          <h2>Results</h2>
          <p>
            Level at 40 Hz: <strong data-testid="spl-40hz">{spl40.toFixed(2)}</strong> dB SPL
          </p>
          <p data-testid="scores">
            Bass limit {sim.f6_hz.toFixed(0)} Hz. Evenness {sim.flatness_db.toFixed(1)} dB. Maximum
            clean level {sim.max_spl_metric_db.toFixed(0)} dB. Cost: {sim.cost.summary}
          </p>
          <div className="row">
            <button type="button" onClick={() => compareInputRef.current?.click()}>
              Compare with another design
            </button>
            <input
              ref={compareInputRef}
              data-testid="compare-file"
              type="file"
              accept="application/json,.json"
              hidden
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (!file) {
                  return;
                }
                void (async () => {
                  const text = await file.text();
                  const checked = await designCheck(text);
                  const result = await simulate(checked.canonical_json, revision.current, grid);
                  setSimB(result);
                  setNameB(file.name);
                  setCanonicalB(checked.canonical_json);
                })();
              }}
            />
          </div>
          {simB && (
            <p data-testid="scores-b">
              {nameB}: bass limit {simB.f6_hz.toFixed(0)} Hz. Evenness {simB.flatness_db.toFixed(1)} dB.
              Maximum clean level {simB.max_spl_metric_db.toFixed(0)} dB. Cost: {simB.cost.summary}
            </p>
          )}
          <div className="row">
            <button
              type="button"
              onClick={() => {
                if (!canonical || !design) {
                  return;
                }
                void (async () => {
                  const plan = await correct(canonical);
                  if (plan.error) {
                    setCorrectionNote(plan.error);
                    return;
                  }
                  const next = structuredClone(design);
                  const plans = plan.per_radiator ?? [{ filters: plan.filters, message: plan.message }];
                  plans.forEach((entry, index) => {
                    const chain = next.dsp.per_radiator[index];
                    if (!chain) {
                      return;
                    }
                    const kept = chain.blocks.filter((block) => block.correction !== true);
                    chain.blocks = [...kept, ...entry.filters];
                  });
                  next.dsp.correction.enabled = true;
                  setCorrectionNote(plan.message);
                  await publish(next, designName);
                })();
              }}
            >
              Apply correction
            </button>
            <button
              type="button"
              data-testid="export-filters"
              onClick={() => {
                if (!canonical) {
                  return;
                }
                void (async () => {
                  const pack = await listen(canonical, true, 0, design?.dsp.sample_rate_hz ?? 48000);
                  if (pack.error) {
                    setCorrectionNote(pack.error);
                    return;
                  }
                  setFilterText(pack.filter_text);
                  const blob = new Blob([pack.filter_text], { type: "text/plain" });
                  const url = URL.createObjectURL(blob);
                  const anchor = document.createElement("a");
                  anchor.href = url;
                  anchor.download = "filters.txt";
                  anchor.click();
                  URL.revokeObjectURL(url);
                })();
              }}
            >
              Export filters
            </button>
            <button
              type="button"
              onClick={() => {
                if (!design) {
                  return;
                }
                void (async () => {
                  const points: Array<{ volume: number; bass: number }> = [];
                  for (const depth of [0.1, 0.14, 0.18, 0.24, 0.32, 0.4]) {
                    const next = structuredClone(design);
                    next.speaker.enclosure.outer.depth_m = depth;
                    const checked = await designCheck(JSON.stringify(next));
                    const result = await simulate(checked.canonical_json, revision.current, grid);
                    points.push({
                      volume: result.interior_volume_m3 ?? 0,
                      bass: result.f6_hz,
                    });
                  }
                  setSweep(points);
                })();
              }}
            >
              Sweep box depth
            </button>
          </div>
          {correctionNote && <p data-testid="correction-note">{correctionNote}</p>}
          {filterText && (
            <pre data-testid="filter-text" className="filter-text">
              {filterText}
            </pre>
          )}
          {sweep.length > 0 && (
            <svg data-testid="volume-sweep" viewBox="0 0 320 140" role="img" aria-label="Bass limit against box volume">
              <text x="8" y="16" fontSize="12">
                Bass limit against box size
              </text>
              <polyline
                fill="none"
                stroke="#2f5d50"
                strokeWidth="2"
                points={sweep
                  .map((point, index) => {
                    const x = 20 + index * 50;
                    const y = 120 - Math.min(point.bass, 200) / 2;
                    return `${x},${y}`;
                  })
                  .join(" ")}
              />
            </svg>
          )}
          <p>Cost: {sim.cost.summary}</p>
          {simIssues.length > 0 && (
            <ul className="issues">
              {simIssues.map((issue) => (
                <li key={`${issue.code}-${issue.field}`}>
                  <span className="issue-field">{issue.field || "(result)"}</span>: {issue.message}
                </li>
              ))}
            </ul>
          )}
          <Graph
            title="Loudness"
            explanation="How loud this speaker is at each frequency, on axis. A second design, if loaded, is drawn in brown."
            freqs={sim.frequencies_hz}
            values={sim.summed_spl_db}
            overlay={simB?.summed_spl_db}
            unit="dB SPL"
            testId="response-graph"
            fidelity={fidelityText(curve.fidelity)}
          />
          {sim.radiators.map((radiatorCurve, index) => (
            <Graph
              key={`driver-graph-${index}`}
              title={`Driver ${index + 1}`}
              explanation="This driver's own contribution, including its port if it has one."
              freqs={sim.frequencies_hz}
              values={radiatorCurve.spl_db}
              overlay={simB?.radiators[index]?.spl_db}
              unit="dB SPL"
              testId={`response-radiator-${index}`}
              fidelity={fidelityText(radiatorCurve.fidelity)}
            />
          ))}
          {curve.port_speed_m?.some((speed) => speed > 0) && (
            <Graph
              title="Port air speed"
              explanation="How fast air moves in the port. It gets noisy above about 17 m/s."
              freqs={sim.frequencies_hz}
              values={curve.port_speed_m}
              unit="m/s"
              testId="port-speed-graph"
              fidelity={fidelityText(curve.fidelity)}
            />
          )}
          <Graph
            title="Impedance"
            explanation="How hard the driver is for the amplifier to drive."
            freqs={sim.frequencies_hz}
            values={curve.impedance_ohm}
            unit="ohm"
            testId="impedance-graph"
            fidelity={fidelityText(curve.fidelity)}
          />
          <Graph
            title="Cone travel"
            explanation="How far the cone moves (excursion) at the test voltage."
            freqs={sim.frequencies_hz}
            values={curve.cone_travel_m.map((meters) => meters * 1000)}
            unit="mm"
            testId="excursion-graph"
            fidelity={fidelityText(curve.fidelity)}
          />
        </section>
      )}

      <section className="panel">
        <h2>Listen</h2>
        <p data-testid="listen-note">
          You are hearing the simulated speaker through your own headphones or speakers. Differences
          between designs are reliable. Absolute tone is not.
        </p>
        <div className="row">
          <label>
            Source
            <select
              data-testid="listen-source"
              value={source}
              onChange={(event) => setSource(event.target.value as ListenSource)}
            >
              <option value="pink">Pink noise</option>
              <option value="sweep">Log sweep</option>
              <option value="tone">Tone</option>
              <option value="file">Audio file</option>
            </select>
          </label>
          <label>
            Amplifier voltage (V)
            <input
              data-testid="listen-voltage"
              type="range"
              min={0.5}
              max={20}
              step={0.1}
              value={volts}
              onChange={(event) => {
                const next = Number(event.target.value);
                setVolts(next);
                playback.send(new Float64Array([2, next]));
              }}
            />
          </label>
        </div>
        <p>
          {volts.toFixed(1)} V.{" "}
          <span data-testid="listen-level">
            {levelAtOneMeter === null
              ? "Load a design to see the level at 1 m."
              : `Level at 1 m: ${levelAtOneMeter.toFixed(0)} dB.`}
          </span>{" "}
          <span data-testid="cone-limit" className={limitHot ? "limit-light lit" : "limit-light"}>
            {limitHot ? "Limit exceeded" : "Within limits"}
          </span>
        </p>
        {sim?.radiators.map((radiatorCurve, index) => {
          const hot = radiatorCurve.spl_db.some(
            (level, bin) => level + voltageDb > (radiatorCurve.max_spl_db[bin] ?? Infinity) + 0.05,
          );
          return (
            <span key={index} data-testid={`cone-limit-${index}`} className={hot ? "limit-light lit" : "limit-light"}>
              Driver {index + 1}: {hot ? "Limit exceeded" : "Within limits"}
            </span>
          );
        })}
        <label className="check">
          <input
            data-testid="hear-correction"
            type="checkbox"
            checked={hearCorrection}
            onChange={(event) => {
              const on = event.target.checked;
              setHearCorrection(on);
              if (!canonical || !playback.playing) {
                return;
              }
              void listen(canonical, on, 0, design?.dsp.sample_rate_hz ?? 48000).then((pack) => {
                if (!pack.error) {
                  playback.send(
                    coefficientMessage(pack.channels),
                  );
                }
              });
            }}
          />
          Hear correction (short crossfade)
        </label>
        <div className="row">
          <button
            type="button"
            data-testid="listen-play"
            onClick={() => {
              if (!canonical) {
                playback.setError("Load a design before listening.");
                return;
              }
              void playback.start({
                source,
                volts,
                file: audioFile,
                prepare: async (sampleRate) => {
                  const pack = await listen(canonical, hearCorrection, 2048, sampleRate);
                  if (pack.error) {
                    throw new Error(pack.error);
                  }
                  return { channels: pack.channels, impulses: pack.impulses };
                },
              }).catch((err: unknown) => {
                playback.setError(err instanceof Error ? err.message : String(err));
              });
            }}
            disabled={playback.playing}
          >
            Play
          </button>
          <button type="button" data-testid="listen-stop" onClick={() => void playback.stop()} disabled={!playback.playing}>
            Stop
          </button>
          <button
            type="button"
            data-testid="listen-ab"
            disabled={!canonicalB || !playback.playing}
            onClick={() => {
              if (!canonicalB || !sim || !simB) {
                return;
              }
              const match = 0.25 * 10 ** ((bandMean(sim) - bandMean(simB)) / 20);
              void playback
                .armOther(async (sampleRate) => {
                  const pack = await listen(canonicalB, hearCorrection, 2048, sampleRate);
                  if (pack.error) {
                    throw new Error(pack.error);
                  }
                  return { channels: pack.channels, impulses: pack.impulses };
                })
                .then(() => playback.crossfade(true, match));
            }}
          >
            Hear the other design
          </button>
          <button type="button" onClick={() => audioFileRef.current?.click()}>
            Open audio file
          </button>
          <input
            ref={audioFileRef}
            data-testid="listen-file"
            type="file"
            accept="audio/*"
            hidden
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (!file) {
                return;
              }
              void file.arrayBuffer().then((buffer) => {
                setAudioFile(buffer);
                setSource("file");
              });
            }}
          />
        </div>
        {playback.error && <p className="error">{playback.error}</p>}
      </section>
    </main>
  );
}

function bandMean(result: SimulateResult) {
  const values = result.summed_spl_db.filter(
    (_, index) => result.frequencies_hz[index] >= 300 && result.frequencies_hz[index] <= 3000,
  );
  if (values.length === 0) {
    return 0;
  }
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function NumberField({
  label,
  value,
  onChange,
  testId,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
  testId?: string;
}) {
  return (
    <label>
      {label}
      <input
        data-testid={testId}
        type="number"
        value={Number.isFinite(value) ? value : ""}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  );
}

function TextField({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <label>
      {label}
      <input value={value} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}
