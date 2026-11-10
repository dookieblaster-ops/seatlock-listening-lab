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
