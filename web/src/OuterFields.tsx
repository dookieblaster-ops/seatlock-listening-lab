/** Editable outer-shape fields for each built kind (README 6.2). Boxes keep width/height/depth;
 * tubes, revolved shapes, sloped baffles, and prisms get their own fields instead of fake box ones. */

function mm(value: number) {
  return value * 1000;
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

function TextField({
  label,
  value,
  onChange,
  testId,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  testId?: string;
}) {
  return (
    <label>
      {label}
      <input
        data-testid={testId}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

type Outer = Record<string, unknown>;

export function OuterFields({
  outer,
  listeningDistanceM,
  onNumber,
  onText,
  onJson,
}: {
  outer: Outer;
  listeningDistanceM: number;
  onNumber: (field: string, value: number) => void;
  onText: (field: string, value: string) => void;
  onJson: (field: string, raw: string) => void;
}) {
  const kind = String(outer.kind ?? "box");
  return (
    <div className="grid" data-testid="outer-fields" data-kind={kind}>
      <p data-testid="outer-kind">
        Outer shape: <strong>{kind}</strong>
      </p>
      {kind === "tube" && (
        <>
          <TextField
            label="Stock catalog id"
            testId="tube-stock-id"
            value={String(outer.stock_id ?? "")}
            onChange={(value) => onText("stock_id", value)}
          />
          <NumberField
            label="Tube length (mm)"
            testId="tube-length-mm"
            value={mm(Number(outer.length_m))}
            onChange={(value) => onNumber("length_m", value / 1000)}
          />
          <NumberField
            label="End-cap thickness (mm)"
            testId="tube-cap-mm"
            value={mm(Number(outer.cap_thickness_m ?? 0.008))}
            onChange={(value) => onNumber("cap_thickness_m", value / 1000)}
          />
          {outer.wall_m != null && (
            <NumberField
              label="Wall thickness (mm)"
              testId="tube-wall-mm"
              value={mm(Number(outer.wall_m))}
              onChange={(value) => onNumber("wall_m", value / 1000)}
            />
          )}
        </>
      )}
      {kind === "revolved" && (
        <>
          <NumberField
            label="Widest diameter (mm)"
            testId="revolved-diameter-mm"
            value={mm(Number(outer.diameter_m))}
            onChange={(value) => onNumber("diameter_m", value / 1000)}
          />
          <NumberField
            label="Length (mm)"
            testId="revolved-length-mm"
            value={mm(Number(outer.length_m))}
            onChange={(value) => onNumber("length_m", value / 1000)}
          />
          <NumberField
            label="Front flat diameter (mm)"
            testId="revolved-front-mm"
            value={mm(Number(outer.front_diameter_m))}
            onChange={(value) => onNumber("front_diameter_m", value / 1000)}
          />
          <NumberField
            label="Back flat diameter (mm)"
            testId="revolved-back-mm"
            value={mm(Number(outer.back_diameter_m ?? outer.front_diameter_m))}
            onChange={(value) => onNumber("back_diameter_m", value / 1000)}
          />
          <NumberField
            label="Widest point (0–1 from front)"
            testId="revolved-widest-at"
            value={Number(outer.widest_at ?? 0.5)}
            onChange={(value) => onNumber("widest_at", value)}
          />
          <NumberField
            label="Wall thickness (mm)"
            testId="revolved-wall-mm"
            value={mm(Number(outer.wall_m))}
            onChange={(value) => onNumber("wall_m", value / 1000)}
          />
        </>
      )}
      {kind === "sloped" && (
        <>
          <NumberField
            label="Width (mm)"
            testId="sloped-width-mm"
            value={mm(Number(outer.width_m))}
            onChange={(value) => onNumber("width_m", value / 1000)}
          />
          <NumberField
            label="Height (mm)"
            testId="sloped-height-mm"
            value={mm(Number(outer.height_m))}
            onChange={(value) => onNumber("height_m", value / 1000)}
          />
          <NumberField
            label="Depth at bottom (mm)"
            testId="sloped-depth-bottom-mm"
            value={mm(Number(outer.depth_bottom_m))}
            onChange={(value) => onNumber("depth_bottom_m", value / 1000)}
          />
          <NumberField
            label="Depth at top (mm)"
            testId="sloped-depth-top-mm"
            value={mm(Number(outer.depth_top_m))}
            onChange={(value) => onNumber("depth_top_m", value / 1000)}
          />
          <NumberField
            label="Wall thickness (mm)"
            testId="sloped-wall-mm"
            value={mm(Number(outer.wall_m))}
            onChange={(value) => onNumber("wall_m", value / 1000)}
          />
        </>
      )}
      {kind === "prism" && (
        <>
          <NumberField
            label="Extrusion length (mm)"
            testId="prism-length-mm"
            value={mm(Number(outer.length_m))}
            onChange={(value) => onNumber("length_m", value / 1000)}
          />
          <NumberField
            label="Wall thickness (mm)"
            testId="prism-wall-mm"
            value={mm(Number(outer.wall_m))}
            onChange={(value) => onNumber("wall_m", value / 1000)}
          />
          <label className="wide">
            Front outline (meters, [x, z] corners)
            <textarea
              data-testid="prism-outline"
              rows={6}
              value={JSON.stringify(outer.outline_m ?? [], null, 2)}
              onChange={(event) => onJson("outline_m", event.target.value)}
            />
          </label>
        </>
      )}
      {(kind === "box" ||
        kind === "rounded" ||
        kind === "chamfered" ||
        kind === "sheet_box") && (
        <>
          {kind === "sheet_box" && (
            <TextField
              label="Sheet stock catalog id"
              testId="sheet-stock-id"
              value={String(outer.stock_id ?? "")}
              onChange={(value) => onText("stock_id", value)}
            />
          )}
          <NumberField
            label="Width (mm)"
            testId="box-width-mm"
            value={mm(Number(outer.width_m))}
            onChange={(value) => onNumber("width_m", value / 1000)}
          />
          <NumberField
            label="Height (mm)"
            value={mm(Number(outer.height_m))}
            onChange={(value) => onNumber("height_m", value / 1000)}
          />
          <NumberField
            label="Depth (mm)"
            value={mm(Number(outer.depth_m))}
            onChange={(value) => onNumber("depth_m", value / 1000)}
          />
          {kind !== "sheet_box" && (
            <NumberField
              label="Wall thickness (mm)"
              value={mm(Number(outer.wall_m))}
              onChange={(value) => onNumber("wall_m", value / 1000)}
            />
          )}
          {kind === "rounded" && (
            <NumberField
              label="Edge radius (mm)"
              testId="rounded-edge-mm"
              value={mm(Number(outer.edge_radius_m ?? 0))}
              onChange={(value) => onNumber("edge_radius_m", value / 1000)}
            />
          )}
          {kind === "chamfered" && (
            <NumberField
              label="Chamfer (mm)"
              testId="chamfer-mm"
              value={mm(Number(outer.chamfer_m ?? 0))}
              onChange={(value) => onNumber("chamfer_m", value / 1000)}
            />
          )}
          {kind === "box" && (
            <NumberField
              label="Edge radius (mm)"
              value={mm(Number(outer.edge_radius_m ?? 0))}
              onChange={(value) => onNumber("edge_radius_m", value / 1000)}
            />
          )}
        </>
      )}
      {kind !== "mesh" &&
        kind !== "tube" &&
        kind !== "revolved" &&
        kind !== "sloped" &&
        kind !== "prism" &&
        kind !== "box" &&
        kind !== "rounded" &&
        kind !== "chamfered" &&
        kind !== "sheet_box" && (
          <p data-testid="outer-fields-fallback">
            This outer shape ({kind}) has no dedicated page fields yet; edit the design file.
          </p>
        )}
      <NumberField
        label="Listening distance (m)"
        value={listeningDistanceM}
        onChange={(value) => onNumber("__listening_distance_m", value)}
      />
    </div>
  );
}
