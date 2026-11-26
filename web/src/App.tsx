import { PanelBuildField, PrintMaterialField, WallBuildField } from "./MaterialControls";
import type { Catalogs } from "./ImportedCabinet";
import { useEffect, useRef, useState } from "react";

import { CabinetView } from "./CabinetView";
import { ImportedCabinet, placeAt, type PickTarget } from "./ImportedCabinet";
import { CostPanel } from "./CostPanel";
import { CostQuality } from "./CostQuality";
import { Generate } from "./Generate";
import { printToScad } from "./openscad";
import { OuterFields } from "./OuterFields";
import { PassiveEditor, type PassiveNetwork } from "./PassiveEditor";
import { ShapeInside } from "./ShapeInside";
import type { Issue } from "./generated/Issue";
import { displayGrid, Graph, nearestValue } from "./Graph";
import {
  buildPrint,
  hullsOf,
  meshTo3mf,
  meshToStl,
  type PrintPart,
  type SolidMesh,
} from "./printSolid";
import type { FullMessage } from "./fdtdWorker";
import { useEngine, type GeometryReport, type SimulateResult } from "./useEngine";
import { coefficientMessage, useListen, type ListenSource } from "./useListen";
import { WaveView, type WaveFieldReport } from "./WaveView";
import type { WaveMessage } from "./wavefieldWorker";
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
    room?: Record<string, unknown>;
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
    /** Passive R/L/C board from one amp to the drivers (README section 7). */
    passive?: PassiveNetwork | null;
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
  const { version, ready, error: engineError, designCheck, simulate, geometry, correct, listen, driverRecord, catalogList } = useEngine();
  const [catalogs, setCatalogs] = useState<Catalogs | null>(null);
  useEffect(() => {
    if (!ready) {
      return;
    }
    void Promise.all([catalogList("panels"), catalogList("damping"), catalogList("fills"), catalogList("materials")])
      .then(([panels, damping, fills, materials]) => setCatalogs({ panels, damping, fills, materials }))
      .catch(() => setCatalogs(null));
  }, [ready, catalogList]);
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
  const [parts, setParts] = useState<PrintPart[]>([]);
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
  const [fdtdRuns, setFdtdRuns] = useState<SimulateResult[]>([]);
  const fdtdWorker = useRef<Worker | null>(null);
  const outsideWorker = useRef<Worker | null>(null);
  const [outsideRuns, setOutsideRuns] = useState<OutsideRun[]>([]);
  const [outsideNote, setOutsideNote] = useState("");
  const bemWorker = useRef<Worker | null>(null);
  const [bemResult, setBemResult] = useState<SimulateResult | null>(null);
  const waveWorker = useRef<Worker | null>(null);
  const [waveReport, setWaveReport] = useState<WaveFieldReport | null>(null);
  const [waveNote, setWaveNote] = useState("The 3D wave views have not been run.");
  const [waveFreq, setWaveFreq] = useState(750);
  const [bemNote, setBemNote] = useState("");
  const [bemElapsedMs, setBemElapsedMs] = useState(0);

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
    const built = await buildPrint(report);
    if (rev !== revision.current) {
      return;
    }
    setSolid(built.mesh);
    setParts(built.parts);
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

  const [picking, setPicking] = useState<PickTarget>(null);
  const outer = design?.speaker.enclosure.outer;
  const mount = design?.speaker.enclosure.mounts[0];
  const radiator = design?.speaker.radiators[0];
  const chain = design?.dsp.per_radiator[0];
  const block = chain?.blocks[0];
  useEffect(() => {
    const sources = design?.speaker.radiators ?? [];
    const withDriver = sources
      .map((item, index) => ({ index, id: item.driver_id }))
      .filter((item): item is { index: number; id: string } => typeof item.id === "string");
    const ids = withDriver.map((item) => item.id);
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
        records.map((record, place) => {
          const index = withDriver[place].index;
          if ("error" in record || record.status === "synthetic" || String(ids[place]).startsWith("test-")) {
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
          return `Driver ${index + 1}: ${tier}${record.maker ?? ""} ${record.model ?? ids[place]}. ${money}${unconfirmed}${stale ? ". This price is more than 90 days old." : ""}`;
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
            data-testid="open-design-input"
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
          {outer.kind === "mesh" ? (
            <div className="grid" data-testid="outer-fields" data-kind="mesh">
              <p data-testid="outer-kind">
                Outer shape: <strong>mesh</strong>
              </p>
              <NumberField
                label="Listening distance (m)"
                value={design.speaker.listening.distance_m}
                onChange={(value) => updateNumber("speaker.listening.distance_m", value)}
              />
            </div>
          ) : (
            <OuterFields
              outer={outer as Record<string, unknown>}
              listeningDistanceM={design.speaker.listening.distance_m}
              onNumber={(field, value) => {
                if (field === "__listening_distance_m") {
                  updateNumber("speaker.listening.distance_m", value);
                  return;
                }
                updateNumber(`speaker.enclosure.outer.${field}`, value);
              }}
              onText={(field, value) => {
                const next = structuredClone(design);
                (next.speaker.enclosure.outer as Record<string, unknown>)[field] = value;
                void publish(next, designName);
              }}
              onJson={(field, raw) => updateJson(`speaker.enclosure.outer.${field}`, raw)}
            />
          )}
          <ImportedCabinet
            design={design}
            publish={(next) => void publish(next, designName)}
            picking={picking}
            setPicking={setPicking}
            catalogs={catalogs ?? undefined}
          />
          <h3>Room</h3>
          <label title="Adds the wall behind the speaker and the room's reverberant sound to the results. It is an estimate for comparing designs, not a room simulation.">
            <input
              data-testid="room-toggle"
              type="checkbox"
              checked={design.speaker.room?.kind === "simple"}
              onChange={(event) => {
                const next = structuredClone(design);
                if (event.target.checked) {
                  next.speaker.room = { kind: "simple", wall_behind_m: 0.5, wall_reflection: 0.9, volume_m3: 50, rt60_s: 0.4 };
                } else {
                  delete next.speaker.room;
                }
                void publish(next, designName);
              }}
            />
            Estimate the sound in a room
          </label>
          {design.speaker.room?.kind === "simple" && (
            <div className="grid">
              <NumberField label="Wall behind the speaker (m)" testId="room-wall-m" value={Number(design.speaker.room.wall_behind_m)} onChange={(value) => updateNumber("speaker.room.wall_behind_m", value)} />
              <NumberField label="Wall reflection (0 to 1)" value={Number(design.speaker.room.wall_reflection)} onChange={(value) => updateNumber("speaker.room.wall_reflection", value)} />
              <NumberField label="Room volume (m^3)" value={Number(design.speaker.room.volume_m3)} onChange={(value) => updateNumber("speaker.room.volume_m3", value)} />
              <NumberField label="Reverberation time, RT60 (s)" value={Number(design.speaker.room.rt60_s)} onChange={(value) => updateNumber("speaker.room.rt60_s", value)} />
            </div>
          )}
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
            {typeof radiator.driver_id === "string" && (
              <TextField label="Driver" value={radiator.driver_id} onChange={(value) => {
                const next = structuredClone(design);
                next.speaker.radiators[0].driver_id = value;
                void publish(next, designName);
              }} />
            )}
            <TextField label="Opening used" value={String(radiator.mount)} onChange={(value) => {
              const next = structuredClone(design);
              next.speaker.radiators[0].mount = value;
              void publish(next, designName);
            }} />
          </div>
          {design.speaker.radiators.map((item, index) =>
            item.kind === "dml_panel" ? (
              <PanelSource key={`panel-${index}`} item={item} index={index} catalogs={catalogs} onChange={(change) => {
                const next = structuredClone(design);
                change(next.speaker.radiators[index]);
                void publish(next, designName);
              }} />
            ) : (
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
            ),
          )}
          {driverLines.map((line) => (
            <p key={line} data-testid="driver-note">{line}</p>
          ))}
          {design.speaker.radiators.map((item, index) =>
            index === 0 || item.kind === "dml_panel" ? null : (
              <div className="grid" key={`extra-driver-${index}`} data-testid={`extra-driver-${index}`}>
                <TextField
                  label={`Driver ${index + 1} face`}
                  value={String(
                    design.speaker.enclosure.mounts.find((mount) => mount.id === item.mount)?.surface ?? "front",
                  )}
                  onChange={(value) => {
                    const next = structuredClone(design);
                    const mount = next.speaker.enclosure.mounts.find((row) => row.id === item.mount);
                    if (mount) {
                      mount.surface = value;
                    }
                    void publish(next, designName);
                  }}
                />
                <NumberField
                  label={`Driver ${index + 1} across the face (mm)`}
                  value={mm(
                    Number(
                      (
                        design.speaker.enclosure.mounts.find((mount) => mount.id === item.mount)?.at_m as
                          | number[]
                          | undefined
                      )?.[0] ?? 0,
                    ),
                  )}
                  onChange={(value) => {
                    const next = structuredClone(design);
                    const mount = next.speaker.enclosure.mounts.find((row) => row.id === item.mount);
                    if (mount) {
                      const at = Array.isArray(mount.at_m) ? [...(mount.at_m as number[])] : [0, 0];
                      at[0] = value / 1000;
                      mount.at_m = at;
                    }
                    void publish(next, designName);
                  }}
                />
                <NumberField
                  label={`Driver ${index + 1} up the face (mm)`}
                  value={mm(
                    Number(
                      (
                        design.speaker.enclosure.mounts.find((mount) => mount.id === item.mount)?.at_m as
                          | number[]
                          | undefined
                      )?.[1] ?? 0,
                    ),
                  )}
                  onChange={(value) => {
                    const next = structuredClone(design);
                    const mount = next.speaker.enclosure.mounts.find((row) => row.id === item.mount);
                    if (mount) {
                      const at = Array.isArray(mount.at_m) ? [...(mount.at_m as number[])] : [0, 0];
                      at[1] = value / 1000;
                      mount.at_m = at;
                    }
                    void publish(next, designName);
                  }}
                />
                <button
                  type="button"
                  data-testid={`remove-driver-${index}`}
                  onClick={() => {
                    const next = structuredClone(design);
                    const mountId = String(next.speaker.radiators[index]?.mount ?? "");
                    next.speaker.radiators.splice(index, 1);
                    next.dsp.per_radiator.splice(index, 1);
                    if (
                      mountId &&
                      !next.speaker.radiators.some((radiator) => String(radiator.mount) === mountId)
                    ) {
                      next.speaker.enclosure.mounts = next.speaker.enclosure.mounts.filter(
                        (row) => String(row.id) !== mountId,
                      );
                    }
                    void publish(next, designName);
                  }}
                >
                  Remove driver {index + 1}
                </button>
              </div>
            ),
          )}
          <button
            type="button"
            data-testid="add-driver"
            title="Adds another driver, its own opening, and its own processing channel. Vents are air paths that lead outside; add as many as you need."
            onClick={() => {
              const next = structuredClone(design);
              const n = next.speaker.radiators.length + 1;
              const id = `driver-${n}`;
              const width = Number(next.speaker.enclosure.outer.width_m ?? 0.16);
              const height = Number(next.speaker.enclosure.outer.height_m ?? 0.25);
              next.speaker.enclosure.mounts.push({
                id,
                surface: "front",
                at_m: [width / 2, Math.min(Math.max(0.04, height - 0.04), 0.04 + 0.04 * n)],
                rear_into: "main",
                recess_m: 0,
              });
              next.speaker.radiators.push({
                kind: "cone",
                driver_id: String(next.speaker.radiators[0]?.driver_id ?? "test-woofer-a"),
                mount: id,
              });
              next.dsp.per_radiator.push({
                blocks: [],
                delay_s: 0,
                gain_db: 0,
                invert: false,
                limiter: null,
              });
              void publish(next, designName);
            }}
          >
            Add another driver
          </button>
          <p>
            Vents are air paths that open outside. Add as many as the experiment needs; each one is heard.
          </p>
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
          <PassiveEditor
            value={design.dsp.passive ?? null}
            radiatorCount={design.speaker.radiators.length}
            onChange={(passive) => {
              const next = structuredClone(design);
              next.dsp.passive = passive;
              void publish(next, designName);
            }}
          />
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
          <InsideEditor
            chambers={design.speaker.enclosure.chambers as Array<Record<string, unknown>>}
            ducts={design.speaker.enclosure.ducts as Array<Record<string, unknown>>}
            catalogs={catalogs}
            onChange={(change) => {
              const next = structuredClone(design);
              change(
                next.speaker.enclosure.chambers as Array<Record<string, unknown>>,
                next.speaker.enclosure.ducts as Array<Record<string, unknown>>,
              );
              void publish(next, designName);
            }}
          />
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
            <PrintMaterialField value={design.build.material} catalogs={catalogs} onChange={(value) => {
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
          <WallBuildField
            wall={(design.build as Record<string, unknown>).wall as never}
            catalogs={catalogs}
            onChange={(wall) => {
              const next = structuredClone(design);
              const build = next.build as Record<string, unknown>;
              if (wall) {
                build.wall = wall;
              } else {
                delete build.wall;
              }
              void publish(next, designName);
            }}
          />
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
              {/* Outside by formula is plastic plus air plus cut-outs, so the air the printed
                  solid leaves is the formula's plastic and air minus the printed plastic. */}
              {(geom.solid_volume_m3 + geom.interior_volume_m3 - solid.solidVolumeM3).toExponential(6)}
            </span>{" "}
            m³. 5 mm cells <span data-testid="voxel-air">{geom.voxel_air_volume_m3.toExponential(6)}</span> m³.
            Printed plastic by formula <span data-testid="formula-solid">{geom.solid_volume_m3.toExponential(6)}</span> m³.
          </p>
          <CostPanel cost={geom.cost} printNote={geom.print_note} />
          <CabinetView
            positions={solid.positions}
            indices={solid.indices}
            driver={geom.mounts[0]?.position_m ?? null}
            markers={design?.speaker.enclosure.outer.kind === "mesh" ? geom.mounts.slice(1).map((m) => m.position_m) : undefined}
            onPick={
              picking && design
                ? (point, normal) => {
                    void publish(placeAt(design, picking, point, normal), designName);
                    setPicking(null);
                  }
                : undefined
            }
          />
          <div className="row">
            {(geom.print_split?.parts.length ?? 0) > 0 && parts.length > 0 && (
              <table data-testid="print-parts" title="Each printed part for one speaker, cut to fit the printer.">
                <thead>
                  <tr>
                    <th>Part</th>
                    <th>Size</th>
                    <th>Plastic</th>
                    <th>Print time</th>
                  </tr>
                </thead>
                <tbody>
                  {parts.map((part) => {
                    // The engine's own plastic per cubic meter and hours per kilogram.
                    const perM3 =
                      geom.print_mass_kg / (geom.solid_volume_m3 + (geom.print_split?.pins_m3 ?? 0));
                    const kg = part.volumeM3 * perM3;
                    const hoursPerKg =
                      geom.cost.print_mass_kg > 0
                        ? (geom.cost.print_time_h_at_least ?? 0) / geom.cost.print_mass_kg
                        : 0;
                    return (
                      <tr key={part.name}>
                        <td>{part.name}</td>
                        <td>{part.sizeMm.map((v) => v.toFixed(0)).join(" x ")} mm</td>
                        <td>{(kg * 1000).toFixed(0)} g</td>
                        <td>at least {(kg * hoursPerKg).toFixed(1)} h</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
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
                const bytes = meshTo3mf(
                  parts.length > 0 ? parts : [{ name: "cabinet", positions: solid.positions, indices: solid.indices }],
                );
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
              data-testid="export-scad"
              title="Save the cabinet as an OpenSCAD program: the lab's own recipe of simple solids, in millimeters. OpenSCAD and FreeCAD open it. Changes made there do not come back into the lab."
              disabled={geom.can_export_print === false || !geom.print_tree}
              onClick={() => {
                if (geom.can_export_print === false || !geom.print_tree) {
                  return;
                }
                const input = { print_tree: geom.print_tree, print_split: geom.print_split };
                void hullsOf(input.print_tree).then((hulls) => {
                  const text = printToScad(input, design?.name ?? "cabinet", (points) => hulls.get(JSON.stringify(points))!);
                  const url = URL.createObjectURL(new Blob([text], { type: "application/x-openscad" }));
                  const anchor = document.createElement("a");
                  anchor.href = url;
                  anchor.download = "cabinet.scad";
                  anchor.click();
                  URL.revokeObjectURL(url);
                });
              }}
            >
              Export OpenSCAD file
            </button>
            <button
              type="button"
              disabled={!canonical}
              title="Solve the air inside the cabinet, and around its ports, in 3D: first with 10 mm cells, then 5 mm."
              onClick={() => {
                if (!canonical) {
                  return;
                }
                fdtdWorker.current?.terminate();
                const worker = new Worker(new URL("./fdtdWorker.ts", import.meta.url), { type: "module" });
                fdtdWorker.current = worker;
                setFdtdRuns([]);
                setFdtdWarning("");
                setFdtdNote("Solving the air in 3D…");
                const finished: SimulateResult[] = [];
                worker.onmessage = (event: MessageEvent<FullMessage>) => {
                  const message = event.data;
                  if (message.type === "progress") {
                    const percent = Math.min(99, Math.round((100 * message.steps) / Math.max(message.maxSteps, 1)));
                    setFdtdNote(
                      `Solving the air in 3D with ${(message.dx * 1000).toFixed(0)} mm cells… ${percent}% of the time limit used so far.`,
                    );
                  } else if (message.type === "result") {
                    const result = JSON.parse(message.json) as SimulateResult;
                    if (result.error) {
                      setFdtdNote(`Full 3D failed: ${result.error}`);
                      return;
                    }
                    const problems = result.issues.filter((issue) => issue.code.startsWith("fdtd_"));
                    if (problems.length > 0) {
                      setFdtdWarning(problems.map((issue) => issue.message).join(" "));
                    }
                    if (result.fdtd) {
                      finished.push(result);
                      setFdtdRuns([...finished]);
                    }
                  } else if (message.type === "done") {
                    const times = finished
                      .map(
                        (run) =>
                          `${((run.fdtd?.dx_m ?? 0) * 1000).toFixed(0)} mm in ${((run.fdtd?.elapsed_ms ?? 0) / 1000).toFixed(1)} s`,
                      )
                      .join(", ");
                    setFdtdNote(finished.length > 0 ? `Full 3D finished: ${times}.` : "Full 3D failed: it gave no result.");
                    worker.terminate();
                    fdtdWorker.current = null;
                  } else {
                    setFdtdNote(`Full 3D failed: ${message.message}`);
                  }
                };
                worker.postMessage({ text: canonical, cellSizes: [0.01, 0.005], frequencies: grid });
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
            <button
              type="button"
              data-testid="outside-run"
              disabled={!canonical}
              title="Solve the air around the cabinet in 3D: first with 10 mm cells, then 5 mm. This is the model for drivers on the back, the sides, or the top, and for cabinets that are not boxes. With a room it takes several minutes."
              onClick={() => {
                if (!canonical) {
                  return;
                }
                outsideWorker.current?.terminate();
                const worker = new Worker(new URL("./fdtdWorker.ts", import.meta.url), { type: "module" });
                outsideWorker.current = worker;
                setOutsideRuns([]);
                setOutsideNote("Solving the air outside in 3D…");
                const finished: OutsideRun[] = [];
                worker.onmessage = (event: MessageEvent<FullMessage>) => {
                  const message = event.data;
                  if (message.type === "progress") {
                    const percent = Math.min(99, Math.round((100 * message.steps) / Math.max(message.maxSteps, 1)));
                    setOutsideNote(
                      `Solving the air outside in 3D with ${(message.dx * 1000).toFixed(0)} mm cells… a run is ${percent}% through its time limit.`,
                    );
                  } else if (message.type === "result") {
                    const result = JSON.parse(message.json) as SimulateResult;
                    const failed = result.error ?? result.issues.find((issue) => issue.severity === "error")?.message;
                    if (failed || result.summed_spl_db.length === 0) {
                      setOutsideNote(`The outside in 3D failed: ${failed ?? "it gave no result."}`);
                      return;
                    }
                    finished.push({ dx: message.dx, elapsedMs: message.elapsedMs, result });
                    setOutsideRuns([...finished]);
                  } else if (message.type === "done") {
                    const times = finished
                      .map((run) => `${(run.dx * 1000).toFixed(0)} mm in ${(run.elapsedMs / 1000).toFixed(1)} s`)
                      .join(", ");
                    if (finished.length > 0) {
                      setOutsideNote(`The outside in 3D finished: ${times}.`);
                    }
                    worker.terminate();
                    outsideWorker.current = null;
                  } else {
                    setOutsideNote(`The outside in 3D failed: ${message.message}`);
                  }
                };
                worker.postMessage({ text: canonical, cellSizes: [0.01, 0.005], frequencies: grid, outside: true });
              }}
            >
              Run the outside in 3D
            </button>
            <button
              type="button"
              onClick={() => {
                outsideWorker.current?.terminate();
                outsideWorker.current = null;
                setOutsideNote("The outside in 3D was canceled.");
              }}
            >
              Cancel the outside
            </button>
            <button
              type="button"
              data-testid="bem-run"
              disabled={!canonical}
              title="Solve the air around the cabinet with a boundary-element mesh of its outer shape (README 5.6). Draws over the everyday baffle model. Dense and slower; mouths stay rigid patches. Not the everyday path."
              onClick={() => {
                if (!canonical) {
                  return;
                }
                bemWorker.current?.terminate();
                const worker = new Worker(new URL("./engineWorker.ts", import.meta.url), { type: "module" });
                bemWorker.current = worker;
                setBemResult(null);
                setBemNote("Solving the boundary-element outside…");
                const started = performance.now();
                let ready = false;
                worker.onmessage = (event: MessageEvent<{ type: string; version?: string; json?: string; message?: string }>) => {
                  const message = event.data;
                  if (message.type === "ready") {
                    ready = true;
                    worker.postMessage({
                      type: "simulate",
                      id: 1,
                      text: canonical,
                      revision: revision.current,
                      frequencies: grid,
                      radiation: "bem",
                      bem_subdivisions: 4,
                    });
                    return;
                  }
                  if (message.type === "result" && message.json) {
                    const result = JSON.parse(message.json) as SimulateResult;
                    const failed =
                      result.error ?? result.issues.find((issue) => issue.severity === "error")?.message;
                    if (failed || result.summed_spl_db.length === 0) {
                      setBemNote(`The boundary-element outside failed: ${failed ?? "it gave no result."}`);
                    } else {
                      setBemElapsedMs(performance.now() - started);
                      setBemResult(result);
                      setBemNote(
                        `The boundary-element outside finished in ${((performance.now() - started) / 1000).toFixed(1)} s.`,
                      );
                    }
                    worker.terminate();
                    bemWorker.current = null;
                    return;
                  }
                  if (message.type === "error") {
                    setBemNote(`The boundary-element outside failed: ${message.message ?? "unknown error"}`);
                    worker.terminate();
                    bemWorker.current = null;
                  } else if (!ready && message.type !== "ready") {
                    setBemNote(`The boundary-element outside failed: unexpected worker reply.`);
                  }
                };
                worker.postMessage({ type: "init" });
              }}
            >
              Run the boundary-element outside
            </button>
            <button
              type="button"
              data-testid="bem-cancel"
              onClick={() => {
                bemWorker.current?.terminate();
                bemWorker.current = null;
                setBemNote("The boundary-element outside was canceled.");
              }}
            >
              Cancel the boundary-element outside
            </button>
          </div>
          <div className="row wave-launch">
            <label title="Pick near a cabinet or room resonance so standing-wave nodes show clearly (sealed-test height mode is about 750 Hz).">
              Wave frequency (Hz)
              <input
                data-testid="wave-freq"
                type="number"
                min={20}
                max={4000}
                step={10}
                value={waveFreq}
                onChange={(event) => setWaveFreq(Number(event.target.value) || 750)}
              />
            </label>
            {(
              [
                ["inside", "Show sound inside"],
                ["outside", "Show sound outside"],
                ["room", "Show sound in the room"],
              ] as const
            ).map(([domain, label]) => (
              <button
                key={domain}
                type="button"
                data-testid={`wave-run-${domain}`}
                disabled={!canonical || (domain === "room" && design?.speaker.room?.kind !== "simple")}
                title={
                  domain === "inside"
                    ? "Pressure field inside the cabinet: standing wave at this frequency, and a slow-motion pulse."
                    : domain === "outside"
                      ? "Pressure field around the cabinet from the boundary-element model at this frequency."
                      : "Bass-only 3D wave model of the room (turn on the room estimate first)."
                }
                onClick={() => {
                  if (!canonical) {
                    return;
                  }
                  waveWorker.current?.terminate();
                  const worker = new Worker(new URL("./wavefieldWorker.ts", import.meta.url), {
                    type: "module",
                  });
                  waveWorker.current = worker;
                  setWaveReport(null);
                  setWaveNote(
                    domain === "inside"
                      ? "Solving the air inside for the wave view…"
                      : domain === "outside"
                        ? "Solving the air outside for the wave view…"
                        : "Solving the room for the wave view…",
                  );
                  worker.onmessage = (event: MessageEvent<WaveMessage>) => {
                    const message = event.data;
                    if (message.type === "progress") {
                      const percent = Math.min(
                        99,
                        Math.round((100 * message.steps) / Math.max(message.maxSteps, 1)),
                      );
                      setWaveNote(`Solving the wave view… ${percent}%`);
                    } else if (message.type === "result") {
                      const parsed = JSON.parse(message.json) as WaveFieldReport & { error?: string };
                      if (parsed.error) {
                        setWaveNote(`Wave view failed: ${parsed.error}`);
                      } else {
                        if (message.elapsedMs > 0) {
                          parsed.elapsed_ms = message.elapsedMs;
                        }
                        setWaveReport(parsed);
                        setWaveNote(
                          `Wave view ready (${domain}, ${parsed.frequency_hz.toFixed(0)} Hz).`,
                        );
                      }
                      worker.terminate();
                      waveWorker.current = null;
                    } else {
                      setWaveNote(`Wave view failed: ${message.message}`);
                      worker.terminate();
                      waveWorker.current = null;
                    }
                  };
                  worker.postMessage({
                    text: canonical,
                    domain,
                    frequencyHz: waveFreq,
                    pulse: domain !== "outside",
                    dxM: domain === "room" ? 0.22 : domain === "outside" ? 0.022 : 0.012,
                  });
                }}
              >
                {label}
              </button>
            ))}
            <button
              type="button"
              data-testid="wave-cancel"
              onClick={() => {
                waveWorker.current?.terminate();
                waveWorker.current = null;
                setWaveNote("Wave view canceled.");
              }}
            >
              Cancel wave view
            </button>
          </div>
          <p data-testid="wave-note">{waveNote}</p>
          {waveReport && (
            <WaveView
              report={waveReport}
              cabinet={
                solid && waveReport.domain !== "room"
                  ? { positions: solid.positions, indices: solid.indices }
                  : null
              }
            />
          )}
          <p data-testid="outside-note">{outsideNote}</p>
          {outsideRuns.length > 0 && <OutsideThreeD runs={outsideRuns} quick={sim} />}
          <p data-testid="bem-note">{bemNote}</p>
          {bemResult && <BemOutside result={bemResult} quick={sim} elapsedMs={bemElapsedMs} />}
          <p data-testid="fdtd-note">{fdtdNote}</p>
          {fdtdNote.includes("unreachable") || fdtdNote.includes("RuntimeError") ? (
            <p data-testid="fdtd-error">{fdtdNote}</p>
          ) : null}
          {fdtdWarning ? <p data-testid="fdtd-warning">{fdtdWarning}</p> : null}
          {fdtdRuns.length > 0 && <FullThreeD runs={fdtdRuns} quick={sim} />}
        </section>
      )}

      <Generate
        text={canonical ?? ""}
        onOpen={(next, label) => {
          void publish(next as Design, label);
        }}
      />

      <ShapeInside
        text={canonical ?? ""}
        onOpen={(next, label) => {
          void publish(next as Design, label);
        }}
      />

      <CostQuality
        text={canonical}
        onOpen={(next, label) => {
          void publish(next as Design, label);
        }}
      />

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
          {sim.window_flatness_db !== undefined && sim.axis_flatness_db !== undefined ? (
            <p
              data-testid="evenness-parts"
              title="Evenness is the worse of two: the level straight ahead (in the room, when the design has one), and the level averaged over straight ahead, 10, 20 and 30 degrees to either side, and 10 degrees up and down (the listening window)."
            >
              Evenness straight ahead {sim.axis_flatness_db.toFixed(1)} dB; a little off to the sides and up and down{" "}
              {sim.window_flatness_db.toFixed(1)} dB. The score takes the worse.
            </p>
          ) : null}
          {sim.cta2034 ? (
            <p
              data-testid="preference-score"
              title="Olive preference rating from CTA-2034 curves (listening window, early reflections, sound power, and the predicted in-room response). Higher is better; published kit scores are often about 2 to 7."
            >
              Preference rating {sim.cta2034.score.preference.toFixed(2)} (CTA-2034 / Olive). Bass
              extension {sim.cta2034.score.lfx_hz.toFixed(0)} Hz. Narrow-band deviation on-axis{" "}
              {sim.cta2034.score.nbd_on.toFixed(2)} dB, predicted in-room {sim.cta2034.score.nbd_pir.toFixed(2)}{" "}
              dB. Smoothness {sim.cta2034.score.sm_pir.toFixed(2)}.
            </p>
          ) : null}
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
          {sim.in_room && (
            <Graph
              title="Loudness in the room"
              explanation="The sound at the listening position with the wall behind the speaker and the room's reverberant sound added. The direct sound alone is drawn in brown. The score is taken from this curve."
              freqs={sim.frequencies_hz}
              values={sim.in_room.spl_db}
              overlay={sim.summed_spl_db}
              unit="dB SPL"
              testId="in-room-graph"
              fidelity={fidelityText(sim.fidelity)}
            />
          )}
          {sim.cta2034 && (
            <Graph
              title="Predicted in-room (CTA-2034)"
              explanation="The CTA-2034 predicted in-room response: 12% listening window, 44% early reflections, and 44% sound power. The level straight ahead is drawn in brown. The preference rating comes from this family of curves."
              freqs={sim.frequencies_hz}
              values={sim.cta2034.predicted_in_room_db}
              overlay={sim.summed_spl_db}
              unit="dB"
              testId="cta2034-pir-graph"
              fidelity={fidelityText(sim.fidelity)}
            />
          )}
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
              Sound source {index + 1}: {hot ? "Limit exceeded" : "Within limits"}
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

type Rows = Array<Record<string, unknown>>;

function numbers(value: unknown): number[] {
  return Array.isArray(value) ? value.map((item) => Number(item)) : [];
}

/** Path points in mm, one "x, y, z" per line. Null while the text is not a valid path. */
function parsePath(text: string): number[][] | null {
  const lines = text.split("\n").map((line) => line.trim()).filter((line) => line.length > 0);
  if (lines.length < 2) {
    return null;
  }
  const points: number[][] = [];
  for (const line of lines) {
    const parts = line.split(/[,\s]+/).filter((part) => part.length > 0).map(Number);
    if (parts.length !== 3 || parts.some((part) => !Number.isFinite(part))) {
      return null;
    }
    points.push(parts.map((part) => part / 1000));
  }
  return points;
}

function pathText(path: unknown): string {
  return (Array.isArray(path) ? path : [])
    .map((point) => numbers(point).map((value) => String(Math.round(value * 10000) / 10)).join(", "))
    .join("\n");
}

function PathField({ value, index, onChange }: { value: unknown; index: number; onChange: (path: number[][]) => void }) {
  const [text, setText] = useState(pathText(value));
  const [bad, setBad] = useState(false);
  useEffect(() => {
    const current = parsePath(text);
    if (!current || JSON.stringify(current) !== JSON.stringify(value)) {
      setText(pathText(value));
      setBad(false);
    }
    // Only an outside change of the path resets the text.
  }, [JSON.stringify(value)]);
  return (
    <label title="The air path's center line, from its inside end, one corner per line: x, y, z in mm from the cabinet's outer front-bottom-left corner. Every corner is a bend.">
      Path corners (mm, x, y, z per line)
      <textarea
        data-testid={`duct-path-${index}`}
        rows={Math.max(3, text.split("\n").length)}
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          const path = parsePath(event.target.value);
          setBad(path === null);
          if (path) {
            onChange(path);
          }
        }}
      />
      {bad && <span className="warning">Each line needs three numbers, and a path needs two lines at least.</span>}
    </label>
  );
}

/**
 * The inside of the cabinet (README 6.2): air spaces (chambers) and the air paths between
 * them and to the outside (ducts, ports, folded and tapered lines).
 */
function InsideEditor({
  chambers,
  ducts,
  catalogs,
  onChange,
}: {
  chambers: Rows;
  ducts: Rows;
  catalogs: Catalogs | null;
  onChange: (change: (chambers: Rows, ducts: Rows) => void) => void;
}) {
  const places = ["main", ...chambers.map((chamber) => String(chamber.id)), "outside"].filter(
    (place, index, all) => all.indexOf(place) === index,
  );
  const corner = (row: Record<string, unknown>, key: string, axis: number) => mm(numbers(row[key])[axis] ?? 0);
  const damping = catalogs?.damping ?? [];
  return (
    <div data-testid="inside-editor">
      <h3 title="Boxes of air inside the cabinet. With none listed, the whole inside is one air space called main. Inside air that no chamber claims is printed solid, so chambers also make partitions.">
        Inside air spaces (chambers)
      </h3>
      {chambers.length === 0 && <p>None listed: the whole inside is one air space, main.</p>}
      {chambers.map((chamber, index) => (
        <div className="grid" key={`chamber-${index}`} data-testid={`chamber-${index}`}>
          <TextField label="Name" value={String(chamber.id ?? "")} onChange={(value) => onChange((rows) => {
            rows[index].id = value;
          })} />
          {(["min_m", "max_m"] as const).map((key) =>
            [0, 1, 2].map((axis) => (
              <NumberField
                key={`${key}-${axis}`}
                label={`${key === "min_m" ? "From" : "To"} ${["x", "y", "z"][axis]} (mm)`}
                value={corner(chamber, key, axis)}
                onChange={(value) => onChange((rows) => {
                  const point = numbers(rows[index][key]);
                  point[axis] = value / 1000;
                  rows[index][key] = point;
                })}
              />
            )),
          )}
          <label title="Damping fill packing this air space (mineral wool, glass wool, polyester). Everyday models use Delany–Bazley Cap from the fill's flow resistivity; the full 3D model uses a Rayleigh update with the same figure.">
            Damping fill
            <select
              data-testid={`chamber-fill-${index}`}
              value={chamber.fill == null ? "" : String(chamber.fill)}
              onChange={(event) => onChange((rows) => {
                rows[index].fill = event.target.value === "" ? null : event.target.value;
              })}
            >
              <option value="">None</option>
              {damping.map((m) => (
                <option key={m.id} value={m.id}>{m.name ?? m.id}</option>
              ))}
            </select>
          </label>
          <button type="button" onClick={() => onChange((rows) => {
            rows.splice(index, 1);
          })}>Remove this air space</button>
        </div>
      ))}
      <button type="button" data-testid="add-chamber" onClick={() => onChange((rows) => {
        rows.push({ id: `chamber-${rows.length + 1}`, min_m: [0.02, 0.02, 0.02], max_m: [0.1, 0.1, 0.1], fill: null });
      })}>Add an air space</button>
      <h3 title="Air paths: ports, tubes between air spaces, and folded or tapered lines. Each corner of a path is a bend. A slot that bends must stay in one plane.">
        Air paths (ports, ducts, and lines)
      </h3>
      {ducts.map((duct, index) => {
        const section = (duct.section ?? {}) as Record<string, unknown>;
        const round = (section.shape ?? "round") === "round";
        const start = numbers(section.start_m);
        const end = numbers(section.end_m).length > 0 ? numbers(section.end_m) : start;
        const setSize = (key: "start_m" | "end_m", slot: number, value: number) => onChange((_, rows) => {
          const next = { ...((rows[index].section ?? {}) as Record<string, unknown>) };
          const sizes = numbers(next[key]).length > 0 ? numbers(next[key]) : numbers(next.start_m);
          sizes[slot] = value / 1000;
          next[key] = sizes;
          rows[index].section = next;
        });
        return (
          <div key={`duct-${index}`} data-testid={`duct-${index}`}>
            <div className="grid">
              <TextField label="Name" value={String(duct.id ?? "")} onChange={(value) => onChange((_, rows) => {
                rows[index].id = value;
              })} />
              {(["from", "to"] as const).map((key) => (
                <label key={key}>
                  {key === "from" ? "From" : "To"}
                  <select value={String(duct[key] ?? "")} onChange={(event) => onChange((_, rows) => {
                    rows[index][key] = event.target.value;
                  })}>
                    {places.map((place) => (
                      <option key={place} value={place}>{place}</option>
                    ))}
                  </select>
                </label>
              ))}
              <label title="Round: a tube, sized by its diameter. Slot: a rectangle, width across the fold and height in the plane of its bends.">
                Shape
                <select data-testid={`duct-shape-${index}`} value={round ? "round" : "rect"} onChange={(event) => onChange((_, rows) => {
                  const rect = event.target.value === "rect";
                  const size = (values: number[]) => (rect ? [values[0] ?? 0.04, values[0] ?? 0.04] : [values[0] ?? 0.04]);
                  rows[index].section = { shape: event.target.value, start_m: size(start), end_m: size(end) };
                })}>
                  <option value="round">round</option>
                  <option value="rect">slot</option>
                </select>
              </label>
              {(round ? [0] : [0, 1]).map((slot) =>
                (["start_m", "end_m"] as const).map((key) => (
                  <NumberField
                    key={`${key}-${slot}`}
                    testId={`duct-${key === "start_m" ? "start" : "end"}-${slot}-${index}`}
                    label={`${round ? "Diameter" : slot === 0 ? "Width" : "Height"} at the ${key === "start_m" ? "start" : "end"} (mm)`}
                    value={mm((key === "start_m" ? start : end)[slot] ?? 0)}
                    onChange={(value) => setSize(key, slot, value)}
                  />
                )),
              )}
            </div>
            <PathField value={duct.path_m} index={index} onChange={(path) => onChange((_, rows) => {
              rows[index].path_m = path;
            })} />
            <button type="button" onClick={() => onChange((_, rows) => {
              rows.splice(index, 1);
            })}>Remove this air path</button>
          </div>
        );
      })}
      <button type="button" data-testid="add-duct" onClick={() => onChange((_, rows) => {
        rows.push({
          id: `path-${rows.length + 1}`,
          from: places[0],
          to: "outside",
          path_m: [[0.05, 0.08, 0.05], [0.05, 0.0, 0.05]],
          section: { shape: "round", start_m: [0.035], end_m: [0.035] },
        });
      })}>Add an air path</button>
    </div>
  );
}

/** A panel speaker (README 5.9, `dml_panel`): its exciter, its panel, and the air sealed behind it. */
function PanelSource({
  item,
  index,
  catalogs,
  onChange,
}: {
  item: Record<string, unknown>;
  index: number;
  catalogs: Catalogs | null;
  onChange: (change: (radiator: Record<string, unknown>) => void) => void;
}) {
  const panel = (item.panel ?? {}) as Record<string, unknown>;
  const at = Array.isArray(item.exciter_at_m) ? (item.exciter_at_m as number[]) : [0, 0];
  const setPanel = (field: string, value: unknown) =>
    onChange((radiator) => {
      radiator.panel = { ...((radiator.panel ?? {}) as Record<string, unknown>), [field]: value };
    });
  return (
    <div data-testid={`panel-source-${index}`}>
      <p data-testid={`panel-note-${index}`} title="A stiff, light panel made to vibrate by a small motor (an exciter). It sends sound out through many bending resonances at once.">
        Sound source {index + 1}: panel speaker (DML panel), {mm(Number(panel.width_m)).toFixed(0)} x{" "}
        {mm(Number(panel.height_m)).toFixed(0)} mm, {mm(Number(panel.thickness_m)).toFixed(1)} mm thick. Its result depends on the
        panel material's stiffness and damping numbers.
      </p>
      <div className="grid">
        <TextField label="Exciter catalog id" value={String(item.exciter_id ?? "")} onChange={(value) => onChange((radiator) => {
          radiator.exciter_id = value;
        })} />
        <NumberField label="Panel width (mm)" value={mm(Number(panel.width_m))} onChange={(value) => setPanel("width_m", value / 1000)} />
        <NumberField label="Panel height (mm)" value={mm(Number(panel.height_m))} onChange={(value) => setPanel("height_m", value / 1000)} />
        {Array.isArray(panel.layers) ? null : (
          <NumberField label="Panel thickness (mm)" value={mm(Number(panel.thickness_m))} onChange={(value) => setPanel("thickness_m", value / 1000)} />
        )}
        <label title="How the panel is held at its rim: free (resting on soft foam), supported (hinged), or clamped.">
          Panel edges
          <select value={String(panel.edges ?? "free")} onChange={(event) => setPanel("edges", event.target.value)}>
            <option value="free">free</option>
            <option value="supported">supported</option>
            <option value="clamped">clamped</option>
          </select>
        </label>
        <NumberField label="Exciter from the panel's left edge (mm)" value={mm(Number(at[0]))} onChange={(value) => onChange((radiator) => {
          radiator.exciter_at_m = [value / 1000, Number(at[1])];
        })} />
        <NumberField label="Exciter from the panel's bottom edge (mm)" value={mm(Number(at[1]))} onChange={(value) => onChange((radiator) => {
          radiator.exciter_at_m = [Number(at[0]), value / 1000];
        })} />
        <NumberField label="Air sealed behind the panel, depth (mm)" value={mm(Number(item.cavity_depth_m ?? 0))} onChange={(value) => onChange((radiator) => {
          radiator.cavity_depth_m = value / 1000;
        })} />
      </div>
      <PanelBuildField
        panel={panel}
        index={index}
        catalogs={catalogs}
        setPanel={(change) =>
          onChange((radiator) => {
            const next = { ...((radiator.panel ?? {}) as Record<string, unknown>) };
            change(next);
            radiator.panel = next;
          })
        }
      />
    </div>
  );
}

type OutsideRun = { dx: number; elapsedMs: number; result: SimulateResult };

/** Boundary-element outside (`bem`, README 5.6), against the everyday baffle model. */
function BemOutside({
  result,
  quick,
  elapsedMs,
}: {
  result: SimulateResult;
  quick: SimulateResult | null;
  elapsedMs: number;
}) {
  const label = `${result.fidelity.enclosure} enclosure, ${result.fidelity.radiation} radiation, ${result.fidelity.room} room. ${result.fidelity.ignored.join(" ")}`;
  return (
    <div data-testid="bem-results">
      <p data-testid="bem-timing">
        Boundary-element mesh of the outer shape in {(elapsedMs / 1000).toFixed(1)} s. Mouths are still rigid patches;
        mutual mouth loads are left out. Not the everyday model.
      </p>
      <Graph
        title="Level with the boundary-element outside"
        explanation="Green is the boundary-element model of the air around the cabinet. Brown is the everyday baffle model."
        freqs={result.frequencies_hz}
        values={result.summed_spl_db}
        overlay={quick?.summed_spl_db}
        unit="dB SPL"
        testId="bem-spl-graph"
        fidelity={label}
      />
      {result.in_room && (
        <Graph
          title="Loudness in the room with the boundary-element outside"
          explanation="Green is the sound at the listening position with the wall behind the speaker and the room's reverberant sound, from the boundary-element model. Brown is the direct sound alone."
          freqs={result.frequencies_hz}
          values={result.in_room.spl_db}
          overlay={result.summed_spl_db}
          unit="dB SPL"
          testId="bem-room-graph"
          fidelity={label}
        />
      )}
    </div>
  );
}

/** The air around the cabinet in 3D (README 5.6, `exterior`), against the everyday model. */
function OutsideThreeD({ runs, quick }: { runs: OutsideRun[]; quick: SimulateResult | null }) {
  const finest = runs.reduce((best, run) => (run.dx < best.dx ? run : best));
  const coarse = runs.find((run) => run !== finest);
  const result = finest.result;
  const cell = `${(finest.dx * 1000).toFixed(0)} mm`;
  // Two cell sizes: their largest and typical difference up to the coarse grid's limit.
  let spread: { most: number; median: number; below: number } | null = null;
  if (coarse) {
    const below = 343.2 / (10 * coarse.dx);
    const all: number[] = [];
    result.frequencies_hz.forEach((freq, index) => {
      if (freq <= below) {
        all.push(Math.abs(result.summed_spl_db[index] - coarse.result.summed_spl_db[index]));
      }
    });
    all.sort((a, b) => a - b);
    if (all.length > 0) {
      spread = { most: all[all.length - 1], median: all[Math.floor(all.length / 2)], below };
    }
  }
  const label = `${result.fidelity.enclosure} enclosure, ${result.fidelity.radiation} radiation (${cell}), ${result.fidelity.room} room. ${result.fidelity.ignored.join(" ")}`;
  return (
    <div data-testid="outside-results">
      {spread ? (
        <p data-testid="outside-uncertainty">
          The 10 mm and 5 mm grids differ by {spread.median.toFixed(2)} dB typically and {spread.most.toFixed(2)} dB at
          most, up to {(spread.below / 1000).toFixed(1)} kHz. That is the uncertainty from turning the cabinet into cells.
        </p>
      ) : null}
      <Graph
        title="Level with the outside in 3D"
        explanation={`Green is the direct sound with the air around the cabinet solved in 3D at ${cell} cells. Brown is the everyday model.`}
        freqs={result.frequencies_hz}
        values={result.summed_spl_db}
        overlay={quick?.summed_spl_db}
        unit="dB SPL"
        testId="outside-spl-graph"
        fidelity={label}
      />
      {result.in_room && (
        <Graph
          title="Loudness in the room with the outside in 3D"
          explanation="Green is the sound at the listening position with the wall behind the speaker and the room's reverberant sound, from the 3D model. Brown is the direct sound alone."
          freqs={result.frequencies_hz}
          values={result.in_room.spl_db}
          overlay={result.summed_spl_db}
          unit="dB SPL"
          testId="outside-room-graph"
          fidelity={label}
        />
      )}
    </div>
  );
}

function FullThreeD({ runs, quick }: { runs: SimulateResult[]; quick: SimulateResult | null }) {
  const finest = runs.reduce((best, run) => ((run.fdtd?.dx_m ?? 1) < (best.fdtd?.dx_m ?? 1) ? run : best));
  const report = finest.fdtd;
  if (!report) {
    return null;
  }
  const coarse = runs.find((run) => run !== finest);
  // Two cell sizes (README 5.5): the bass difference, and the typical difference up to the
  // coarse grid's limit. Lossless resonance peaks are left out: their heights are not modeled.
  let spread: { bass: number; median: number; below: number } | null = null;
  if (coarse?.fdtd) {
    const below = Math.min(coarse.fdtd.valid_up_to_hz, report.valid_up_to_hz);
    let bass = 0;
    const all: number[] = [];
    finest.frequencies_hz.forEach((freq, index) => {
      const diff = Math.abs(finest.summed_spl_db[index] - coarse.summed_spl_db[index]);
      if (freq <= 300) {
        bass = Math.max(bass, diff);
      }
      if (freq <= below) {
        all.push(diff);
      }
    });
    all.sort((a, b) => a - b);
    spread = { bass, median: all.length > 0 ? all[Math.floor(all.length / 2)] : 0, below };
  }
  const magnitude = (curve: { re: number[]; im: number[] }) =>
    curve.re.map((re, index) => Math.hypot(re, curve.im[index] ?? 0));
  const cell = `${(report.dx_m * 1000).toFixed(0)} mm`;
  return (
    <div data-testid="fdtd-results">
      {spread ? (
        <p data-testid="fdtd-uncertainty">
          The 10 mm and 5 mm grids differ by up to {spread.bass.toFixed(2)} dB below 300 Hz, and typically by{" "}
          {spread.median.toFixed(2)} dB up to {(spread.below / 1000).toFixed(1)} kHz. That is the uncertainty from turning
          the cabinet into cells. Peaks at the cabinet's own resonances are not modeled yet.
        </p>
      ) : null}
      <p>
        Grid air {(report.grid_volume_m3 * 1000).toFixed(2)} L against {(report.net_volume_m3 * 1000).toFixed(2)} L in
        the design; {report.steps} time steps{report.decayed ? "" : ", stopped at the time limit"}.
      </p>
      <Graph
        title="Level with full 3D"
        explanation={`Green is the full 3D model at ${cell} cells, brown the quick model.`}
        freqs={finest.frequencies_hz}
        values={finest.summed_spl_db}
        overlay={quick?.summed_spl_db}
        unit="dB SPL"
        testId="fdtd-spl-graph"
        fidelity={`Full 3D (${cell}), ${finest.fidelity.radiation} radiation. ${finest.fidelity.ignored.join(" ")}`}
      />
      <Graph
        title="Air load"
        explanation="How the air in the cabinet pushes back on the cone. Green is the full 3D model, brown the quick model."
        freqs={report.frequencies_hz}
        values={magnitude(report.z_ab)}
        overlay={magnitude(report.z_ab_quick)}
        unit="Pa·s/m³"
        testId="impedance-3d-graph"
        fidelity={`Quick model and full 3D at ${cell} cells, rigid walls, air without losses.`}
      />
    </div>
  );
}
