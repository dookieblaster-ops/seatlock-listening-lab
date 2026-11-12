import type { PrintSplit, PrintTree } from "./printSolid";

/** Millimeters, with trailing zeros dropped. */
function mm(value_m: number): string {
  const text = (value_m * 1000).toFixed(4).replace(/\.?0+$/, "");
  return text === "-0" || text === "" ? "0" : text;
}

function vec(values_m: number[]): string {
  return `[${values_m.map(mm).join(", ")}]`;
}

function plain(value: number): string {
  const text = value.toFixed(9).replace(/\.?0+$/, "");
  return text === "-0" || text === "" ? "0" : text;
}

/** A convex hull's corner points and triangles, turned outward, from a list of points. */
export type HullMesh = (points_m: number[][]) => { points_m: number[][]; triangles: number[][] };

function node(tree: PrintTree, indent: string, hull: HullMesh): string[] {
  if (tree.op === "box") {
    const size = [0, 1, 2].map((k) => tree.max_m[k] - tree.min_m[k]);
    return [`${indent}translate(${vec(tree.min_m)}) cube(${vec(size)});`];
  }
  if (tree.op === "cylinder") {
    // The same turn the page's own builder makes: about x, then about y.
    const [ax, ay, az] = tree.axis;
    const rx = (-Math.asin(Math.max(-1, Math.min(1, ay))) * 180) / Math.PI;
    const ry = (Math.atan2(ax, az) * 180) / Math.PI;
    const turn = rx === 0 && ry === 0 ? "" : `rotate([${plain(rx)}, ${plain(ry)}, 0]) `;
    return [
      `${indent}translate(${vec(tree.base_m)}) ${turn}cylinder(h = ${mm(tree.length_m)}, r1 = ${mm(tree.radius_m)}, r2 = ${mm(
        tree.radius_end_m ?? tree.radius_m,
      )}, $fn = ${tree.segments ?? 96});`,
    ];
  }
  if (tree.op === "hull") {
    // OpenSCAD wants each face clockwise as seen from outside.
    const mesh = hull(tree.points_m);
    const points = mesh.points_m.map(vec).join(", ");
    const faces = mesh.triangles.map((t) => `[${t[0]}, ${t[2]}, ${t[1]}]`).join(", ");
    return [`${indent}polyhedron(points = [${points}], faces = [${faces}], convexity = 2);`];
  }
  if (tree.op === "prism") {
    const { u, v, w, origin_m: o } = tree;
    const rows = [0, 1, 2].map((k) => `[${plain(u[k])}, ${plain(v[k])}, ${plain(w[k])}, ${mm(o[k])}]`);
    const outline = tree.outline_m.map((p) => `[${mm(p[0])}, ${mm(p[1])}]`).join(", ");
    return [
      `${indent}multmatrix([${rows.join(", ")}, [0, 0, 0, 1]]) linear_extrude(height = ${mm(tree.length_m)}) polygon([${outline}]);`,
    ];
  }
  if (tree.op === "revolve") {
    const profile = tree.profile_m.map((p) => `[${mm(p[0])}, ${mm(p[1])}]`).join(", ");
    return [
      `${indent}translate(${vec(tree.origin_m)}) rotate([-90, 0, 0]) rotate_extrude($fn = ${tree.segments ?? 96}) polygon([${profile}]);`,
    ];
  }
  const children = tree.of.flatMap((child) => node(child, `${indent}  `, hull));
  if (children.length === 0) {
    return [];
  }
  if (tree.op === "union") {
    return [`${indent}union() {`, ...children, `${indent}}`];
  }
  // The first child, minus all the others.
  const first = node(tree.of[0], `${indent}  `, hull);
  if (first.length === 0) {
    return [];
  }
  return [`${indent}difference() {`, ...children, `${indent}}`];
}

/**
 * The cabinet as an OpenSCAD program (also opened by FreeCAD's OpenSCAD workbench), in
 * millimeters: the engine's own recipe of solids, written out. `part` at the top picks the
 * whole cabinet or one printable part.
 */
export function printToScad(
  input: { print_tree: PrintTree; print_split?: PrintSplit },
  name: string,
  hull: HullMesh,
): string {
  const split = input.print_split;
  const parts = split?.parts ?? [];
  const lines: string[] = [
    `// ${name}: cabinet from Seatlock Listening Lab. Sizes are in millimeters.`,
    "// This file is the lab's own recipe for the printed solid: simple solids joined and cut.",
    "// Changes made here do not go back into the lab. To resize, change the design in the lab",
    "// and export again.",
    "",
  ];
  if (parts.length > 0) {
    lines.push(
      `// Which piece to show: 0 is the whole cabinet; 1 to ${parts.length} is one printable part.`,
      "part = 0;",
      "",
    );
  }
  const body = node(input.print_tree, "  ", hull);
  lines.push("module cabinet() {", ...(body.length > 0 ? body : ["  // Nothing is printed for this cabinet."]), "}", "");
  if (parts.length > 0 && split) {
    lines.push("// The boxes that cut the cabinet into parts that fit the printer: [low corner, size].", "cuts = [");
    parts.forEach((part) => {
      const size = [0, 1, 2].map((k) => part.max_m[k] - part.min_m[k]);
      lines.push(`  [${vec(part.min_m)}, ${vec(size)}], // ${part.name}`);
    });
    lines.push(
      "];",
      "",
      "if (part == 0) {",
      "  cabinet();",
      "} else {",
      "  intersection() {",
      "    cabinet();",
      "    translate(cuts[part - 1][0]) cube(cuts[part - 1][1]);",
      "  }",
      "}",
      "",
      `// Alignment pins: print ${split.pins.length}, each as below. Their holes are already in the cabinet.`,
      `module pin() { cylinder(h = ${mm(split.pin_length_m)}, r = ${mm(split.pin_radius_m)}, $fn = 48); }`,
    );
  } else {
    lines.push("cabinet();");
  }
  return `${lines.join("\n")}\n`;
}
