import type { ManifoldToplevel } from "manifold-3d";

export type SolidMesh = {
  positions: Float32Array;
  indices: Uint32Array;
  solidVolumeM3: number;
};

let library: Promise<ManifoldToplevel> | null = null;

async function manifold(): Promise<ManifoldToplevel> {
  if (!library) {
    library = import("manifold-3d").then(async (mod) => {
      const wasm = await mod.default();
      wasm.setup();
      return wasm;
    });
  }
  return library;
}

/** A constructive solid geometry tree from the engine (GeometryReport.print_tree). */
export type PrintTree =
  | { op: "box"; min_m: number[]; max_m: number[] }
  | {
      op: "cylinder";
      base_m: number[];
      axis: number[];
      length_m: number;
      radius_m: number;
      /** The radius at the far end, for a taper. */
      radius_end_m?: number;
      segments?: number;
    }
  | { op: "hull"; points_m: number[][] }
  | {
      op: "prism";
      outline_m: number[][];
      origin_m: number[];
      u: number[];
      v: number[];
      w: number[];
      length_m: number;
    }
  | {
      /** A profile of `[radius, depth]` corners revolved about the y axis through `origin_m`. */
      op: "revolve";
      profile_m: number[][];
      origin_m: number[];
      segments?: number;
    }
  | { op: "union" | "difference"; of: PrintTree[] };

type Wasm = ManifoldToplevel;
type Solid = InstanceType<Wasm["Manifold"]>;

/** Builds one node of the tree. Returns null for an empty union. Every object made is deleted. */
function build(wasm: Wasm, node: PrintTree): Solid | null {
  const { Manifold } = wasm;
  if (node.op === "box") {
    const size: [number, number, number] = [
      node.max_m[0] - node.min_m[0],
      node.max_m[1] - node.min_m[1],
      node.max_m[2] - node.min_m[2],
    ];
    const cube = Manifold.cube(size, false);
    const moved = cube.translate([node.min_m[0], node.min_m[1], node.min_m[2]]);
    cube.delete();
    return moved;
  }
  if (node.op === "cylinder") {
    const blank = Manifold.cylinder(
      node.length_m,
      node.radius_m,
      node.radius_end_m ?? node.radius_m,
      node.segments ?? 96,
      false,
    );
    // The blank runs along +z. Turn it to `axis`: about x by -asin(ay), then about y by atan2(ax, az).
    const [ax, ay, az] = node.axis;
    const rx = (-Math.asin(Math.max(-1, Math.min(1, ay))) * 180) / Math.PI;
    const ry = (Math.atan2(ax, az) * 180) / Math.PI;
    const turned = blank.rotate([rx, ry, 0]);
    blank.delete();
    const moved = turned.translate([node.base_m[0], node.base_m[1], node.base_m[2]]);
    turned.delete();
    return moved;
  }
  if (node.op === "hull") {
    return Manifold.hull(node.points_m.map((p) => [p[0], p[1], p[2]] as [number, number, number]));
  }
  if (node.op === "prism") {
    // The outline is extruded along +z, then placed by origin + a*u + b*v + z*w (column-major).
    const outline = node.outline_m.map((p) => [p[0], p[1]] as [number, number]);
    const blank = Manifold.extrude([outline], node.length_m);
    const { u, v, w, origin_m: o } = node;
    const placed = blank.transform([u[0], u[1], u[2], 0, v[0], v[1], v[2], 0, w[0], w[1], w[2], 0, o[0], o[1], o[2], 1]);
    blank.delete();
    return placed;
  }
  if (node.op === "revolve") {
    // Manifold revolves about z; turn z onto y, then move the axis to its place.
    const blank = Manifold.revolve([node.profile_m.map((p) => [p[0], p[1]] as [number, number])], node.segments ?? 96);
    const turned = blank.rotate([-90, 0, 0]);
    blank.delete();
    const moved = turned.translate([node.origin_m[0], node.origin_m[1], node.origin_m[2]]);
    turned.delete();
    return moved;
  }
  const parts = node.of.map((child) => build(wasm, child)).filter((part): part is Solid => part !== null);
  if (parts.length === 0) {
    return null;
  }
  // Batch booleans: one union of everything, or the first minus one union of the rest.
  // Subtracting dozens of pin and bolt holes one at a time is many times slower.
  if (node.op === "union") {
    if (parts.length === 1) {
      return parts[0];
    }
    const joined = Manifold.union(parts);
    parts.forEach((part) => part.delete());
    return joined;
  }
  const [first, ...rest] = parts;
  if (rest.length === 0) {
    return first;
  }
  const cutter = rest.length === 1 ? rest[0] : Manifold.union(rest);
  const result = first.subtract(cutter);
  first.delete();
  cutter.delete();
  if (rest.length > 1) {
    rest.forEach((part) => part.delete());
  }
  return result;
}

/** The engine's plan for cutting the print into parts (GeometryReport.print_split). */
export type PrintSplit = {
  parts: Array<{ name: string; min_m: number[]; max_m: number[] }>;
  pins: Array<{ center_m: number[]; axis: number }>;
  pin_radius_m: number;
  pin_length_m: number;
};

export type PrintPart = MeshObject & { volumeM3: number; sizeMm: number[] };

function toMesh(solid: Solid, name: string): PrintPart {
  const mesh = solid.getMesh();
  const stride = mesh.numProp;
  const count = mesh.vertProperties.length / stride;
  const positions = new Float32Array(count * 3);
  const low = [Infinity, Infinity, Infinity];
  const high = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < count; i += 1) {
    for (let k = 0; k < 3; k += 1) {
      const value = mesh.vertProperties[i * stride + k];
      positions[i * 3 + k] = value;
      low[k] = Math.min(low[k], value);
      high[k] = Math.max(high[k], value);
    }
  }
  return {
    name,
    positions,
    indices: new Uint32Array(mesh.triVerts),
    volumeM3: solid.volume(),
    sizeMm: [0, 1, 2].map((k) => (count ? (high[k] - low[k]) * 1000 : 0)),
  };
}

/**
 * The whole cabinet for the 3D view, and the print cut into the parts the engine planned
 * followed by the alignment pins, each its own object; the solid is built once. Pins lie in
 * a row beside the cabinet. With no plan, the whole cabinet is one part.
 */
export async function buildPrint(input: {
  print_tree: PrintTree;
  print_split?: PrintSplit;
}): Promise<{ mesh: SolidMesh; parts: PrintPart[] }> {
  const wasm = await manifold();
  const { Manifold } = wasm;
  const whole = build(wasm, input.print_tree);
  if (!whole) {
    return { mesh: { positions: new Float32Array(), indices: new Uint32Array(), solidVolumeM3: 0 }, parts: [] };
  }
  const all = toMesh(whole, "cabinet");
  const mesh = { positions: all.positions, indices: all.indices, solidVolumeM3: all.volumeM3 };
  const split = input.print_split;
  if (!split || split.parts.length === 0) {
    whole.delete();
    return { mesh, parts: [all] };
  }
  const parts: PrintPart[] = [];
  for (const part of split.parts) {
    const size = [0, 1, 2].map((k) => part.max_m[k] - part.min_m[k]) as [number, number, number];
    const cell = Manifold.cube(size, false);
    const placed = cell.translate([part.min_m[0], part.min_m[1], part.min_m[2]]);
    cell.delete();
    const piece = whole.intersect(placed);
    placed.delete();
    if (!piece.isEmpty()) {
      parts.push(toMesh(piece, part.name));
    }
    piece.delete();
  }
  whole.delete();
  split.pins.forEach((_, index) => {
    const pin = Manifold.cylinder(split.pin_length_m, split.pin_radius_m, split.pin_radius_m, 48, false);
    const placed = pin.translate([-0.02 - 0.01 * (index % 10), 0.01 * Math.floor(index / 10), 0]);
    pin.delete();
    parts.push(toMesh(placed, `pin ${index + 1}`));
    placed.delete();
  });
  return { mesh, parts };
}

function zipStore(files: Array<{ name: string; data: Uint8Array }>): Uint8Array {
  const parts: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  const encoder = new TextEncoder();
  for (const file of files) {
    const name = encoder.encode(file.name);
    const local = new Uint8Array(30 + name.length);
    const view = new DataView(local.buffer);
    view.setUint32(0, 0x04034b50, true);
    view.setUint16(8, 0, true);
    view.setUint32(18, file.data.length, true);
    view.setUint32(22, file.data.length, true);
    view.setUint16(26, name.length, true);
    local.set(name, 30);
    parts.push(local, file.data);
    const dir = new Uint8Array(46 + name.length);
    const dirView = new DataView(dir.buffer);
    dirView.setUint32(0, 0x02014b50, true);
    dirView.setUint32(20, file.data.length, true);
    dirView.setUint32(24, file.data.length, true);
    dirView.setUint16(28, name.length, true);
    dirView.setUint32(42, offset, true);
    dir.set(name, 46);
    central.push(dir);
    offset += local.length + file.data.length;
  }
  const centralSize = central.reduce((sum, part) => sum + part.length, 0);
  const end = new Uint8Array(22);
  const endView = new DataView(end.buffer);
  endView.setUint32(0, 0x06054b50, true);
  endView.setUint16(8, files.length, true);
  endView.setUint16(10, files.length, true);
  endView.setUint32(12, centralSize, true);
  endView.setUint32(16, offset, true);
  const total = offset + centralSize + end.length;
  const out = new Uint8Array(total);
  let cursor = 0;
  for (const part of [...parts, ...central, end]) {
    out.set(part, cursor);
    cursor += part.length;
  }
  return out;
}

/** One mesh for a 3MF file, in meters. */
export type MeshObject = { name: string; positions: Float32Array; indices: Uint32Array };

/** A 3MF package with one object per mesh (README section 6.4), in millimeters. */
export function meshTo3mf(objects: MeshObject[]): Uint8Array {
  const encoder = new TextEncoder();
  const resources: string[] = [];
  const items: string[] = [];
  objects.forEach((object, index) => {
    const id = index + 1;
    const vertices: string[] = [];
    for (let i = 0; i < object.positions.length; i += 3) {
      vertices.push(
        `<vertex x="${object.positions[i] * 1000}" y="${object.positions[i + 1] * 1000}" z="${object.positions[i + 2] * 1000}"/>`,
      );
    }
    const triangles: string[] = [];
    for (let i = 0; i < object.indices.length; i += 3) {
      triangles.push(
        `<triangle v1="${object.indices[i]}" v2="${object.indices[i + 1]}" v3="${object.indices[i + 2]}"/>`,
      );
    }
    const name = object.name.replace(/[<>&"]/g, "");
    resources.push(
      `<object id="${id}" type="model" name="${name}"><mesh><vertices>${vertices.join("")}</vertices><triangles>${triangles.join("")}</triangles></mesh></object>`,
    );
    items.push(`<item objectid="${id}"/>`);
  });
  const model = `<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>${resources.join("")}</resources>
  <build>${items.join("")}</build>
</model>`;
  const types = `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
</Types>`;
  const rels = `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>`;
  return zipStore([
    { name: "[Content_Types].xml", data: encoder.encode(types) },
    { name: "_rels/.rels", data: encoder.encode(rels) },
    { name: "3D/3dmodel.model", data: encoder.encode(model) },
  ]);
}

export function meshToStl(positions: Float32Array, indices: Uint32Array): ArrayBuffer {
  const triangles = indices.length / 3;
  const buffer = new ArrayBuffer(84 + triangles * 50);
  const view = new DataView(buffer);
  view.setUint32(80, triangles, true);
  let offset = 84;
  for (let i = 0; i < triangles; i += 1) {
    const a = indices[i * 3] * 3;
    const b = indices[i * 3 + 1] * 3;
    const c = indices[i * 3 + 2] * 3;
    const ax = positions[a] * 1000;
    const ay = positions[a + 1] * 1000;
    const az = positions[a + 2] * 1000;
    const bx = positions[b] * 1000;
    const by = positions[b + 1] * 1000;
    const bz = positions[b + 2] * 1000;
    const cx = positions[c] * 1000;
    const cy = positions[c + 1] * 1000;
    const cz = positions[c + 2] * 1000;
    view.setFloat32(offset, 0, true);
    view.setFloat32(offset + 4, 0, true);
    view.setFloat32(offset + 8, 0, true);
    const verts = [ax, ay, az, bx, by, bz, cx, cy, cz];
    verts.forEach((value, index) => view.setFloat32(offset + 12 + index * 4, value, true));
    view.setUint16(offset + 48, 0, true);
    offset += 50;
  }
  return buffer;
}

/** The corner points and outward triangles of the convex hull of `points_m`. */
export async function hullMesh(points_m: number[][]): Promise<{ points_m: number[][]; triangles: number[][] }> {
  const wasm = await manifold();
  const solid = wasm.Manifold.hull(points_m.map((p) => [p[0], p[1], p[2]] as [number, number, number]));
  const mesh = solid.getMesh();
  const stride = mesh.numProp;
  const points: number[][] = [];
  for (let i = 0; i < mesh.vertProperties.length / stride; i += 1) {
    points.push([mesh.vertProperties[i * stride], mesh.vertProperties[i * stride + 1], mesh.vertProperties[i * stride + 2]]);
  }
  const triangles: number[][] = [];
  for (let i = 0; i < mesh.triVerts.length; i += 3) {
    triangles.push([mesh.triVerts[i], mesh.triVerts[i + 1], mesh.triVerts[i + 2]]);
  }
  solid.delete();
  return { points_m: points, triangles };
}

/** Every hull in a tree, built ahead of time, keyed by its points. */
export async function hullsOf(tree: PrintTree): Promise<Map<string, { points_m: number[][]; triangles: number[][] }>> {
  const found = new Map<string, { points_m: number[][]; triangles: number[][] }>();
  const walk = async (node: PrintTree): Promise<void> => {
    if (node.op === "hull") {
      found.set(JSON.stringify(node.points_m), await hullMesh(node.points_m));
    } else if (node.op === "union" || node.op === "difference") {
      for (const child of node.of) {
        await walk(child);
      }
    }
  };
  await walk(tree);
  return found;
}
