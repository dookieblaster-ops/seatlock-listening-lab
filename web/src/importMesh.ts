/**
 * Reading a speaker drawn elsewhere (README 6.2, outer shape `mesh`): 3MF assemblies and STL
 * files become parts of closed triangle meshes in meters, the assembly moved so that it
 * starts at the origin.
 */

export type ImportedPart = {
  name: string;
  /** x, y, z in m per corner. */
  vertices: Float32Array;
  /** Three corner indices per triangle, wound outward. */
  triangles: Uint32Array;
};

/** A part as the design file holds it (README 6.2). */
export type PartField = {
  name: string;
  role: "solid" | "damping";
  material: string;
  vertices_m: string;
  triangles: string;
  flow_resistivity_pa_s_m2?: number;
};

export function toBase64(bytes: Uint8Array): string {
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(text);
}

export function fromBase64(text: string): Uint8Array {
  const raw = atob(text);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i += 1) {
    out[i] = raw.charCodeAt(i);
  }
  return out;
}

export function partField(part: ImportedPart, role: "solid" | "damping" = "solid", material = "pla"): PartField {
  const v = new Uint8Array(new Float32Array(part.vertices).buffer);
  const t = new Uint8Array(new Uint32Array(part.triangles).buffer);
  return { name: part.name, role, material: role === "damping" ? "" : material, vertices_m: toBase64(v), triangles: toBase64(t) };
}

export function fromField(field: PartField): ImportedPart {
  const v = fromBase64(field.vertices_m);
  const t = fromBase64(field.triangles);
  return {
    name: field.name,
    vertices: new Float32Array(v.buffer, v.byteOffset, v.byteLength / 4),
    triangles: new Uint32Array(t.buffer, t.byteOffset, t.byteLength / 4),
  };
}

/** Merges corners at the same place (to a micrometer), so a triangle soup becomes a mesh. */
export function weld(soup: number[] | Float32Array, name: string): ImportedPart {
  const index = new Map<string, number>();
  const vertices: number[] = [];
  const triangles: number[] = [];
  for (let i = 0; i < soup.length; i += 3) {
    const key = `${Math.round(soup[i] * 1e6)},${Math.round(soup[i + 1] * 1e6)},${Math.round(soup[i + 2] * 1e6)}`;
    let at = index.get(key);
    if (at === undefined) {
      at = vertices.length / 3;
      index.set(key, at);
      vertices.push(soup[i], soup[i + 1], soup[i + 2]);
    }
    triangles.push(at);
  }
  // Drop triangles that collapsed to a line.
  const kept: number[] = [];
  for (let i = 0; i < triangles.length; i += 3) {
    const [a, b, c] = [triangles[i], triangles[i + 1], triangles[i + 2]];
    if (a !== b && b !== c && a !== c) {
      kept.push(a, b, c);
    }
  }
  return { name, vertices: new Float32Array(vertices), triangles: new Uint32Array(kept) };
}

/** An STL file, binary or text, in millimeters as STL files almost always are. */
export function parseStl(buffer: ArrayBuffer, name: string, scale = 0.001): ImportedPart {
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);
  const count = bytes.length >= 84 ? view.getUint32(80, true) : 0;
  const soup: number[] = [];
  if (bytes.length === 84 + 50 * count && count > 0) {
    for (let t = 0; t < count; t += 1) {
      const base = 84 + 50 * t + 12;
      for (let k = 0; k < 9; k += 1) {
        soup.push(view.getFloat32(base + 4 * k, true) * scale);
      }
    }
  } else {
    const text = new TextDecoder().decode(bytes);
    const pattern = /vertex\s+(\S+)\s+(\S+)\s+(\S+)/g;
    for (let m = pattern.exec(text); m; m = pattern.exec(text)) {
      soup.push(Number(m[1]) * scale, Number(m[2]) * scale, Number(m[3]) * scale);
    }
  }
  if (soup.length === 0) {
    throw new Error(`${name} holds no triangles.`);
  }
  return weld(soup, name);
}

/** The files in a zip archive (stored or deflated), by name. */
export async function unzip(buffer: ArrayBuffer): Promise<Map<string, Uint8Array>> {
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);
  let end = -1;
  for (let i = bytes.length - 22; i >= Math.max(0, bytes.length - 65557); i -= 1) {
    if (view.getUint32(i, true) === 0x06054b50) {
      end = i;
      break;
    }
  }
  if (end < 0) {
    throw new Error("the file is not a zip archive, so not a 3MF file.");
  }
  const entries = view.getUint16(end + 10, true);
  let at = view.getUint32(end + 16, true);
  const out = new Map<string, Uint8Array>();
  for (let e = 0; e < entries; e += 1) {
    if (view.getUint32(at, true) !== 0x02014b50) {
      break;
    }
    const method = view.getUint16(at + 10, true);
    const size = view.getUint32(at + 20, true);
    const nameLength = view.getUint16(at + 28, true);
    const extraLength = view.getUint16(at + 30, true);
    const commentLength = view.getUint16(at + 32, true);
    const local = view.getUint32(at + 42, true);
    const name = new TextDecoder().decode(bytes.subarray(at + 46, at + 46 + nameLength));
    at += 46 + nameLength + extraLength + commentLength;
    const start = local + 30 + view.getUint16(local + 26, true) + view.getUint16(local + 28, true);
    const data = bytes.subarray(start, start + size);
    if (method === 0) {
      out.set(name, data);
    } else if (method === 8) {
      const stream = new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      out.set(name, new Uint8Array(await new Response(stream).arrayBuffer()));
    }
  }
  return out;
}

const UNITS: Record<string, number> = {
  micron: 1e-6,
  millimeter: 1e-3,
  centimeter: 1e-2,
  inch: 0.0254,
  foot: 0.3048,
  meter: 1,
};

type Matrix = number[];
const IDENTITY: Matrix = [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];

function parseMatrix(text: string | null): Matrix {
  if (!text) {
    return IDENTITY;
  }
  const m = text.trim().split(/\s+/).map(Number);
  return m.length === 12 && m.every(Number.isFinite) ? m : IDENTITY;
}

/** `b` after `a`: 3MF's row-vector matrices, `x' = x M`. */
function compose(a: Matrix, b: Matrix): Matrix {
  const at = (m: Matrix, r: number, c: number) => (r === 3 ? m[9 + c] : m[r * 3 + c]);
  const out: number[] = [];
  for (let r = 0; r < 4; r += 1) {
    for (let c = 0; c < 3; c += 1) {
      let v = r === 3 ? at(b, 3, c) : 0;
      for (let k = 0; k < 3; k += 1) {
        v += at(a, r, k) * at(b, k, c);
      }
      out.push(v);
    }
  }
  return out;
}

/** A 3MF file: every object the build places, each its own part, in m. */
export async function parse3mf(buffer: ArrayBuffer, fileName: string): Promise<ImportedPart[]> {
  const files = await unzip(buffer);
  const models = [...files.entries()].filter(([name]) => name.toLowerCase().endsWith(".model"));
  if (models.length === 0) {
    throw new Error(`${fileName} holds no 3D model.`);
  }
  type Obj = { name: string; vertices?: number[]; triangles?: number[]; components: Array<{ id: string; m: Matrix }> };
  const objects = new Map<string, Obj>();
  const items: Array<{ id: string; m: Matrix }> = [];
  let scale = 1e-3;
  for (const [, data] of models) {
    const doc = new DOMParser().parseFromString(new TextDecoder().decode(data), "application/xml");
    const model = doc.getElementsByTagNameNS("*", "model")[0];
    const unit = model?.getAttribute("unit");
    if (unit && UNITS[unit] !== undefined) {
      scale = UNITS[unit];
    }
    for (const el of Array.from(doc.getElementsByTagNameNS("*", "object"))) {
      const id = el.getAttribute("id") ?? "";
      const obj: Obj = { name: el.getAttribute("name") ?? "", components: [] };
      const mesh = el.getElementsByTagNameNS("*", "mesh")[0];
      if (mesh) {
        obj.vertices = [];
        for (const v of Array.from(mesh.getElementsByTagNameNS("*", "vertex"))) {
          obj.vertices.push(Number(v.getAttribute("x")), Number(v.getAttribute("y")), Number(v.getAttribute("z")));
        }
        obj.triangles = [];
        for (const t of Array.from(mesh.getElementsByTagNameNS("*", "triangle"))) {
          obj.triangles.push(Number(t.getAttribute("v1")), Number(t.getAttribute("v2")), Number(t.getAttribute("v3")));
        }
      }
      for (const c of Array.from(el.getElementsByTagNameNS("*", "component"))) {
        obj.components.push({ id: c.getAttribute("objectid") ?? "", m: parseMatrix(c.getAttribute("transform")) });
      }
      objects.set(id, obj);
    }
    for (const it of Array.from(doc.getElementsByTagNameNS("*", "item"))) {
      items.push({ id: it.getAttribute("objectid") ?? "", m: parseMatrix(it.getAttribute("transform")) });
    }
  }
  if (items.length === 0) {
    for (const [id, obj] of objects) {
      if (obj.vertices) {
        items.push({ id, m: IDENTITY });
      }
    }
  }
  const parts: ImportedPart[] = [];
  const place = (id: string, m: Matrix, label: string, depth: number) => {
    const obj = objects.get(id);
    if (!obj || depth > 8) {
      return;
    }
    const name = obj.name || label;
    if (obj.vertices && obj.triangles && obj.triangles.length > 0) {
      const v = new Float32Array(obj.vertices.length);
      for (let i = 0; i < obj.vertices.length; i += 3) {
        const [x, y, z] = [obj.vertices[i], obj.vertices[i + 1], obj.vertices[i + 2]];
        v[i] = (x * m[0] + y * m[3] + z * m[6] + m[9]) * scale;
        v[i + 1] = (x * m[1] + y * m[4] + z * m[7] + m[10]) * scale;
        v[i + 2] = (x * m[2] + y * m[5] + z * m[8] + m[11]) * scale;
      }
      parts.push({ name, vertices: v, triangles: new Uint32Array(obj.triangles) });
    }
    obj.components.forEach((c, k) => place(c.id, compose(c.m, m), `${name || label} ${k + 1}`, depth + 1));
  };
  items.forEach((item, k) => place(item.id, item.m, `${fileName} part ${k + 1}`, 0));
  if (parts.length === 0) {
    throw new Error(`${fileName} holds no meshes.`);
  }
  // Names must be told apart.
  const seen = new Map<string, number>();
  for (const part of parts) {
    const count = (seen.get(part.name) ?? 0) + 1;
    seen.set(part.name, count);
    if (count > 1) {
      part.name = `${part.name} ${count}`;
    }
  }
  return parts;
}

/** Moves the assembly so its lowest corner is at the origin. */
export function normalize(parts: ImportedPart[]): ImportedPart[] {
  const low = [Infinity, Infinity, Infinity];
  for (const part of parts) {
    for (let i = 0; i < part.vertices.length; i += 1) {
      low[i % 3] = Math.min(low[i % 3], part.vertices[i]);
    }
  }
  return parts.map((part) => {
    const v = new Float32Array(part.vertices);
    for (let i = 0; i < v.length; i += 1) {
      v[i] -= low[i % 3];
    }
    return { ...part, vertices: v };
  });
}

/** Turns the assembly a quarter turn about an axis, then moves it back to the origin. */
export function quarterTurn(parts: ImportedPart[], axis: 0 | 1 | 2): ImportedPart[] {
  return normalize(
    parts.map((part) => {
      const v = new Float32Array(part.vertices.length);
      for (let i = 0; i < v.length; i += 3) {
        const [x, y, z] = [part.vertices[i], part.vertices[i + 1], part.vertices[i + 2]];
        const turned = axis === 0 ? [x, -z, y] : axis === 1 ? [z, y, -x] : [-y, x, z];
        v.set(turned, i);
      }
      return { ...part, vertices: v };
    }),
  );
}

/** Every file the user picked, read into parts and moved to the origin. */
export async function readSpeakerFiles(files: File[]): Promise<ImportedPart[]> {
  const parts: ImportedPart[] = [];
  for (const file of files) {
    const buffer = await file.arrayBuffer();
    const base = file.name.replace(/\.[^.]+$/, "");
    if (file.name.toLowerCase().endsWith(".3mf")) {
      parts.push(...(await parse3mf(buffer, base)));
    } else {
      parts.push(parseStl(buffer, base));
    }
  }
  return normalize(parts);
}
