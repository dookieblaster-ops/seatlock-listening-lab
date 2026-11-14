import type { CatalogEntry } from "./useEngine";
import { useState } from "react";
import { fromField, partField, quarterTurn, readSpeakerFiles, type PartField } from "./importMesh";

/* eslint-disable @typescript-eslint/no-explicit-any */
type AnyDesign = any;

/** What a click on the 3D view places: a sound source's mount, a built port, or a hole mark. */
export type PickTarget =
  | { mount: string; asHole?: boolean }
  | { port: true }
  | { hole: "seal" | "opening" }
  | null;

type Point = {
  id: string;
  point_m: number[];
  normal: number[];
  base_normal?: number[];
  tilt_deg?: number;
  turn_deg?: number;
  hole?: "seal" | "opening";
};

const PRINT_MATERIALS = ["pla", "petg"];

function turned(base: number[], tiltDeg: number, turnDeg: number): number[] {
  const az = Math.atan2(base[1], base[0]) + (turnDeg * Math.PI) / 180;
  const el = Math.asin(Math.max(-1, Math.min(1, base[2]))) + (tiltDeg * Math.PI) / 180;
  return [Math.cos(el) * Math.cos(az), Math.cos(el) * Math.sin(az), Math.sin(el)];
}

function unit(v: number[]): number[] {
  const n = Math.hypot(v[0], v[1], v[2]) || 1;
  return v.map((x) => x / n);
}

function nextId(points: Point[], prefix: string): string {
  let k = 1;
  while (points.some((p) => p.id === `${prefix}-${k}`)) {
    k += 1;
  }
  return `${prefix}-${k}`;
}

/** The design with a source's mount, a new port, or a hole mark at a point the user clicked. */
export function placeAt(design: AnyDesign, target: PickTarget, point: number[], normal: number[]): AnyDesign {
  if (!target) {
    return design;
  }
  const next = structuredClone(design);
  const outer = next.speaker.enclosure.outer;
  const points: Point[] = outer.points ?? [];
  const n = unit(normal);
  if ("mount" in target) {
    const id = `at-${target.mount}`;
    const kept = points.filter((p) => p.id !== id);
    const placed: Point = { id, point_m: point, normal: n, base_normal: n, tilt_deg: 0, turn_deg: 0 };
    if (target.asHole) {
      placed.hole = "seal";
    }
    kept.push(placed);
    outer.points = kept;
    const mount = next.speaker.enclosure.mounts.find((m: any) => m.id === target.mount);
    if (mount) {
      mount.surface = id;
      mount.at_m = [0, 0];
      mount.rear_into = mount.rear_into || "main";
    }
    return next;
  }
  if ("hole" in target) {
    const id = nextId(points, target.hole === "opening" ? "mouth" : "hole");
    outer.points = [...points, { id, point_m: point, normal: n, base_normal: n, tilt_deg: 0, turn_deg: 0, hole: target.hole }];
    return next;
  }
  // A round port 40 mm across and 100 mm long, straight in from the surface.
  const ducts = next.speaker.enclosure.ducts;
  let k = 1;
  while (ducts.some((d: any) => d.id === `port-${k}`)) {
    k += 1;
  }
  const id = `port-${k}`;
  const length = 0.1;
  ducts.push({
    id,
    from: "main",
    to: "outside",
    path_m: [point.map((x, i) => x - n[i] * length), point],
    section: { shape: "round", start_m: [0.04], end_m: [0.04] },
  });
  outer.points = [...points, { id: `at-${id}`, point_m: point, normal: n }];
  return next;
}

export function ImportedCabinet({
  design,
  publish,
  picking,
  setPicking,
  catalogs,
}: {
  design: AnyDesign;
  publish: (next: AnyDesign) => void;
  picking: PickTarget;
  setPicking: (target: PickTarget) => void;
  catalogs?: Catalogs;
}) {
  const [message, setMessage] = useState("");
  const [placeAsHole, setPlaceAsHole] = useState(false);
  const outer = design.speaker.enclosure.outer;
  const imported = outer.kind === "mesh";
  const parts: PartField[] = imported ? outer.parts ?? [] : [];
  const points: Point[] = imported ? outer.points ?? [] : [];
  const mountSurfaces = new Set(design.speaker.enclosure.mounts.map((m: any) => m.surface));
  const freeHoles = points.filter((p) => p.hole && !mountSurfaces.has(p.id));

  async function load(files: FileList | null) {
    if (!files || files.length === 0) {
      return;
    }
    try {
      const read = await readSpeakerFiles(Array.from(files));
      const next = structuredClone(design);
      next.speaker.enclosure.outer = { kind: "mesh", parts: read.map((p) => partField(p)), points: [] };
      // Box chambers and placed plastic belong to the old cabinet; ducts are kept only if the
      // user re-places them.
      next.speaker.enclosure.chambers = [];
      next.speaker.enclosure.solids = [];
      next.speaker.enclosure.ducts = [];
      for (const mount of next.speaker.enclosure.mounts) {
        mount.surface = "";
        mount.rear_into = "main";
      }
      publish(next);
      setMessage(
        `Imported ${read.length} part${read.length === 1 ? "" : "s"}. Turn the model if the front is not toward you, then place each sound source. Mark holes the model already has (driver cut-outs, wire holes, moulded port mouths).`,
      );
    } catch (error) {
      setMessage(`Could not read that file: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  function turn(axis: 0 | 1 | 2) {
    const next = structuredClone(design);
    const turnedParts = quarterTurn(parts.map(fromField), axis);
    next.speaker.enclosure.outer.parts = turnedParts.map((p, i) => ({ ...parts[i], ...partField(p, parts[i].role, parts[i].material), name: parts[i].name, role: parts[i].role, material: parts[i].material }));
    next.speaker.enclosure.outer.points = [];
    next.speaker.enclosure.ducts = [];
    for (const mount of next.speaker.enclosure.mounts) {
      mount.surface = "";
    }
    publish(next);
    setMessage("Turned. Mounting points and ports were cleared; place them again.");
  }

  const printMaterials = catalogs && catalogs.materials.length > 0 ? catalogs.materials.map((m) => m.id) : PRINT_MATERIALS;
  const printName = (id: string) => catalogs?.materials.find((m) => m.id === id)?.name ?? id.toUpperCase();

  function setPart(index: number, change: Partial<PartField>) {
    const next = structuredClone(design);
    const part = next.speaker.enclosure.outer.parts[index];
    Object.assign(part, change);
    if (change.role === "damping") {
      part.material = "";
    } else if (change.role === "solid" && !printMaterials.includes(part.material)) {
      part.material = "pla";
      delete part.flow_resistivity_pa_s_m2;
    }
    publish(next);
  }

  function setAngle(id: string, key: "tilt_deg" | "turn_deg", value: number) {
    const next = structuredClone(design);
    const point = next.speaker.enclosure.outer.points.find((p: Point) => p.id === id);
    if (!point) {
      return;
    }
    point[key] = value;
    point.base_normal = point.base_normal ?? point.normal;
    point.normal = turned(point.base_normal, point.tilt_deg ?? 0, point.turn_deg ?? 0);
    publish(next);
  }

  function setHole(id: string, hole: "seal" | "opening" | "") {
    const next = structuredClone(design);
    const point = next.speaker.enclosure.outer.points.find((p: Point) => p.id === id);
    if (!point) {
      return;
    }
    if (hole) {
      point.hole = hole;
    } else {
      delete point.hole;
    }
    publish(next);
  }

  function removePoint(id: string) {
    const next = structuredClone(design);
    next.speaker.enclosure.outer.points = (next.speaker.enclosure.outer.points as Point[]).filter((p) => p.id !== id);
    publish(next);
  }

  const holeLabel = (hole?: "seal" | "opening") =>
    hole === "seal" ? "fills a hole in the model" : hole === "opening" ? "moulded port mouth" : "no hole mark";

  return (
    <div data-testid="imported-cabinet">
      <h3>Import a speaker</h3>
      <label title="A 3MF file (one part per object) or STL files (one part each), in millimeters. The parts must be closed solids; the air they enclose is the cabinet's air.">
        3MF or STL file
        <input data-testid="import-file" type="file" accept=".3mf,.stl" multiple onChange={(event) => void load(event.target.files)} />
      </label>
      {message && <p data-testid="import-message">{message}</p>}
      {imported && (
        <>
          <p>
            Turn the model a quarter turn:{" "}
            <button onClick={() => turn(0)}>About left-right</button>{" "}
            <button onClick={() => turn(1)}>About front-back</button>{" "}
            <button onClick={() => turn(2)}>About up-down</button>
          </p>
          <table data-testid="imported-parts" title="Each part of the imported speaker: solid (printed) or damping material.">
            <thead>
              <tr>
                <th>Part</th>
                <th>Kind</th>
                <th>Material</th>
              </tr>
            </thead>
            <tbody>
              {parts.map((part, index) => (
                <tr key={`${part.name}-${index}`}>
                  <td>{part.name}</td>
                  <td>
                    <select
                      data-testid={`part-role-${index}`}
                      value={part.role}
                      onChange={(event) => setPart(index, { role: event.target.value as "solid" | "damping" })}
                    >
                      <option value="solid">Solid (printed)</option>
                      <option value="damping">Damping material</option>
                    </select>
                  </td>
                  <td>
                    {part.role === "damping" ? (
                      <>
                        <label title="A damping material from the catalog, with its published air resistance, or your own figure.">
                          Damping material
                          <select
                            data-testid={`part-damping-${index}`}
                            value={part.flow_resistivity_pa_s_m2 !== undefined || !part.material ? "" : part.material}
                            onChange={(event) => {
                              const next = structuredClone(design);
                              const target = next.speaker.enclosure.outer.parts[index];
                              target.material = event.target.value;
                              if (event.target.value) {
                                delete target.flow_resistivity_pa_s_m2;
                              }
                              publish(next);
                            }}
                          >
                            <option value="">Typed in below</option>
                            {(catalogs?.damping ?? []).map((m) => (
                              <option key={m.id} value={m.id}>
                                {m.name ?? m.id} ({Number(m.flow_resistivity_pa_s_m2 ?? 0).toFixed(0)} Pa·s/m²)
                              </option>
                            ))}
                          </select>
                        </label>
                        <label title="The material's flow resistivity from its data sheet: how hard it is to push air through it. Fiber fill is often a few thousand, dense wool tens of thousands.">
                          Air resistance (Pa·s/m²)
                          <input
                            data-testid={`part-resistivity-${index}`}
                            type="number"
                            value={part.flow_resistivity_pa_s_m2 ?? ""}
                            onChange={(event) => setPart(index, { flow_resistivity_pa_s_m2: Number(event.target.value) })}
                          />
                        </label>
                      </>
                    ) : (
                      <select value={part.material} onChange={(event) => setPart(index, { material: event.target.value })}>
                        {printMaterials.map((m) => (
                          <option key={m} value={m}>
                            {printName(m)}
                          </option>
                        ))}
                      </select>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <h3>Place sound sources and ports</h3>
          <p>Choose what to place, then click the model in the 3D view where it goes. It faces straight out from that spot; tilt or turn it below.</p>
          <label
            data-testid="place-as-hole"
            title="Turn this on when the model already has the cut-out, wire hole, or moulded mouth. The lab then holds that hole closed while it finds the air inside, instead of cutting a new one."
          >
            <input
              type="checkbox"
              checked={placeAsHole}
              onChange={(event) => {
                const on = event.target.checked;
                setPlaceAsHole(on);
                if (picking && "mount" in picking) {
                  setPicking({ mount: picking.mount, asHole: on });
                }
              }}
            />
            The model already has this hole
          </label>
          <ul>
            {design.speaker.enclosure.mounts.map((mount: any, index: number) => {
              const point = points.find((p) => p.id === mount.surface);
              const active = picking && "mount" in picking && picking.mount === mount.id;
              return (
                <li key={mount.id}>
                  Sound source {index + 1} ({design.speaker.radiators[index]?.driver_id ?? design.speaker.radiators[index]?.kind}):{" "}
                  {point ? `at ${point.point_m.map((x) => (x * 1000).toFixed(0)).join(", ")} mm` : "not placed"}
                  {point?.hole ? ` (${holeLabel(point.hole)})` : ""}{" "}
                  <button
                    data-testid={`place-mount-${index}`}
                    onClick={() => setPicking(active ? null : { mount: mount.id, asHole: placeAsHole })}
                  >
                    {active ? "Click the model…" : "Place on model"}
                  </button>
                  {point && (
                    <>
                      {" "}
                      <label title="Tilts the source up (positive) or down from straight out.">
                        Tilt (°)
                        <input type="number" value={point.tilt_deg ?? 0} onChange={(event) => setAngle(point.id, "tilt_deg", Number(event.target.value))} />
                      </label>{" "}
                      <label title="Turns the source left or right from straight out.">
                        Turn (°)
                        <input type="number" value={point.turn_deg ?? 0} onChange={(event) => setAngle(point.id, "turn_deg", Number(event.target.value))} />
                      </label>{" "}
                      <label title="Mark whether this point is a hole already in the model: sealed by what is mounted here, or the mouth of a moulded duct.">
                        Hole in model
                        <select
                          data-testid={`mount-hole-${index}`}
                          value={point.hole ?? ""}
                          onChange={(event) => setHole(point.id, event.target.value as "seal" | "opening" | "")}
                        >
                          <option value="">None (lab cuts it)</option>
                          <option value="seal">Fills a hole (seal)</option>
                          <option value="opening">Moulded mouth</option>
                        </select>
                      </label>
                    </>
                  )}
                </li>
              );
            })}
            <li>
              <button data-testid="place-port" onClick={() => setPicking(picking && "port" in picking ? null : { port: true })}>
                {picking && "port" in picking ? "Click the model…" : "Add a port"}
              </button>{" "}
              {design.speaker.enclosure.ducts.length} port{design.speaker.enclosure.ducts.length === 1 ? "" : "s"}; edit their size and length under the inside.
            </li>
            <li>
              <button
                data-testid="place-seal-hole"
                onClick={() => setPicking(picking && "hole" in picking && picking.hole === "seal" ? null : { hole: "seal" })}
              >
                {picking && "hole" in picking && picking.hole === "seal" ? "Click the model…" : "Mark a sealed hole"}
              </button>{" "}
              <button
                data-testid="place-opening-hole"
                onClick={() => setPicking(picking && "hole" in picking && picking.hole === "opening" ? null : { hole: "opening" })}
              >
                {picking && "hole" in picking && picking.hole === "opening" ? "Click the model…" : "Mark a moulded port mouth"}
              </button>
              <span title="Sealed holes are wire holes or cut-outs filled by a driver. A moulded port mouth is open to the outside and measured from the model's air; do not also add a built port for it.">
                {" "}
                For holes the model already has that are not a sound source above.
              </span>
            </li>
          </ul>
          {freeHoles.length > 0 && (
            <ul data-testid="marked-holes">
              {freeHoles.map((point) => (
                <li key={point.id}>
                  {holeLabel(point.hole)} at {point.point_m.map((x) => (x * 1000).toFixed(0)).join(", ")} mm{" "}
                  <label>
                    Kind
                    <select
                      data-testid={`free-hole-${point.id}`}
                      value={point.hole ?? "seal"}
                      onChange={(event) => setHole(point.id, event.target.value as "seal" | "opening")}
                    >
                      <option value="seal">Sealed hole</option>
                      <option value="opening">Moulded mouth</option>
                    </select>
                  </label>{" "}
                  <button data-testid={`remove-hole-${point.id}`} onClick={() => removePoint(point.id)}>
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </div>
  );
}

export type Catalogs = {
  panels: CatalogEntry[];
  damping: CatalogEntry[];
  fills: CatalogEntry[];
  materials: CatalogEntry[];
};
