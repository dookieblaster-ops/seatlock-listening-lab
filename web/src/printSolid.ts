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

export async function buildCabinetMesh(input: {
  width_m: number;
  height_m: number;
  depth_m: number;
  wall_m: number;
  mounts: Array<{ position_m: number[]; cutout_diameter_m: number; outward: number[] }>;
}): Promise<SolidMesh> {
  const wasm = await manifold();
  const { Manifold } = wasm;
  const outer = Manifold.cube([input.width_m, input.depth_m, input.height_m], false);
  const innerSize: [number, number, number] = [
    input.width_m - 2 * input.wall_m,
    input.depth_m - 2 * input.wall_m,
    input.height_m - 2 * input.wall_m,
  ];
  const inner = Manifold.cube(innerSize, false).translate([input.wall_m, input.wall_m, input.wall_m]);
  let solid = outer.subtract(inner);
  outer.delete();
  inner.delete();

  for (const mount of input.mounts) {
    if (mount.cutout_diameter_m <= 0) {
      continue;
    }
    const radius = mount.cutout_diameter_m / 2;
    const blank = Manifold.cylinder(input.wall_m + 0.0004, radius, radius, 48, false);
    const [nx, ny, nz] = mount.outward;
    let cutter = blank;
    if (ny < -0.5) {
      const turned = blank.rotate(-90, 0, 0);
      blank.delete();
      cutter = turned.translate([mount.position_m[0], -0.0002, mount.position_m[2]]);
      turned.delete();
    } else if (ny > 0.5) {
      const turned = blank.rotate(90, 0, 0);
      blank.delete();
      cutter = turned.translate([mount.position_m[0], input.depth_m - input.wall_m - 0.0002, mount.position_m[2]]);
      turned.delete();
    } else if (nx < -0.5) {
      const turned = blank.rotate(0, 90, 0);
      blank.delete();
      cutter = turned.translate([-0.0002, mount.position_m[1], mount.position_m[2]]);
      turned.delete();
    } else if (nx > 0.5) {
      const turned = blank.rotate(0, -90, 0);
      blank.delete();
      cutter = turned.translate([input.width_m - input.wall_m - 0.0002, mount.position_m[1], mount.position_m[2]]);
      turned.delete();
    } else if (nz > 0.5) {
      cutter = blank.translate([mount.position_m[0], mount.position_m[1], input.height_m - input.wall_m - 0.0002]);
      blank.delete();
    } else {
      const turned = blank.rotate(180, 0, 0);
      blank.delete();
      cutter = turned.translate([mount.position_m[0], mount.position_m[1], input.wall_m + 0.0002]);
      turned.delete();
    }
    const next = solid.subtract(cutter);
    solid.delete();
    cutter.delete();
    solid = next;
  }

  const volume = solid.volume();
  const mesh = solid.getMesh();
  const stride = mesh.numProp;
  const count = mesh.vertProperties.length / stride;
  const positions = new Float32Array(count * 3);
  for (let i = 0; i < count; i += 1) {
    positions[i * 3] = mesh.vertProperties[i * stride];
    positions[i * 3 + 1] = mesh.vertProperties[i * stride + 1];
    positions[i * 3 + 2] = mesh.vertProperties[i * stride + 2];
  }
  const indices = new Uint32Array(mesh.triVerts);
  solid.delete();
  return { positions, indices, solidVolumeM3: volume };
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

export function meshTo3mf(positions: Float32Array, indices: Uint32Array): Uint8Array {
  const encoder = new TextEncoder();
  const vertices: string[] = [];
  for (let i = 0; i < positions.length; i += 3) {
    vertices.push(
      `<vertex x="${positions[i] * 1000}" y="${positions[i + 1] * 1000}" z="${positions[i + 2] * 1000}"/>`,
    );
  }
  const triangles: string[] = [];
  for (let i = 0; i < indices.length; i += 3) {
    triangles.push(`<triangle v1="${indices[i]}" v2="${indices[i + 1]}" v3="${indices[i + 2]}"/>`);
  }
  const model = `<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources><object id="1" type="model"><mesh>
    <vertices>${vertices.join("")}</vertices>
    <triangles>${triangles.join("")}</triangles>
  </mesh></object></resources>
  <build><item objectid="1"/></build>
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
