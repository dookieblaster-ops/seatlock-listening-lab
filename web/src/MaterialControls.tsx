import type { Catalogs } from "./ImportedCabinet";
import type { CatalogEntry } from "./useEngine";

/* eslint-disable @typescript-eslint/no-explicit-any */

const label = (entry: CatalogEntry) => entry.name ?? entry.id;

/** Print material for the cabinet: the catalog's filaments, foaming ones marked. */
export function PrintMaterialField({
  value,
  catalogs,
  onChange,
}: {
  value: string;
  catalogs: Catalogs | null;
  onChange: (value: string) => void;
}) {
  const options = catalogs?.materials ?? [];
  return (
    <label title="The filament the cabinet is printed in. Foaming filaments print lighter (and less stiff) than solid ones; the lab prices them by their printed density.">
      Print material
      <select data-testid="build-material" value={value} onChange={(event) => onChange(event.target.value)}>
        {options.some((m) => m.id === value) ? null : <option value={value}>{value}</option>}
        {options.map((m) => (
          <option key={m.id} value={m.id}>
            {label(m)}
            {m.foaming ? " (foaming)" : ""}
          </option>
        ))}
      </select>
    </label>
  );
}

type Wall = { kind?: string; skin_m?: number; fill?: string; core?: string };

/** `build.wall`: a solid printed wall, two skins with a poured fill, or two skins with a
 * second filament between them (README section 6.2). */
export function WallBuildField({
  wall,
  catalogs,
  onChange,
}: {
  wall: Wall | undefined;
  catalogs: Catalogs | null;
  onChange: (wall: Wall | undefined) => void;
}) {
  const kind = wall?.kind ?? "solid";
  const skin = wall?.skin_m ?? 0.0016;
  const fills = catalogs?.fills ?? [];
  const cores = (catalogs?.materials ?? []).filter((m) => m.id !== "pla");
  return (
    <div className="grid" data-testid="wall-build">
      <label title="How the cabinet walls are made. Filled: two thin printed skins with sand or gypsum cement poured between them. Two filaments: the space between the skins printed in a second, softer or foaming filament (constrained-layer damping). The lab prices them and hears them: each flat wall vibrates as a plate of these layers, and its sound is added to the speaker's.">
        Wall build
        <select
          data-testid="wall-kind"
          value={kind}
          onChange={(event) => {
            const next = event.target.value;
            if (next === "solid") {
              onChange(undefined);
            } else if (next === "filled") {
              onChange({ kind: "filled", skin_m: skin, fill: wall?.fill ?? fills[0]?.id ?? "" });
            } else {
              onChange({ kind: "two_material", skin_m: skin, core: wall?.core ?? cores[0]?.id ?? "" });
            }
          }}
        >
          <option value="solid">Solid printed wall</option>
          <option value="filled">Two skins, filled (sand, gypsum)</option>
          <option value="two_material">Two skins, second filament between</option>
        </select>
      </label>
      {kind !== "solid" ? (
        <label title="Thickness of each printed skin; the rest of the wall is the core.">
          Skin thickness (mm)
          <input
            data-testid="wall-skin"
            type="number"
            value={Math.round(skin * 1e5) / 100}
            onChange={(event) => onChange({ ...wall, skin_m: Number(event.target.value) / 1000 })}
          />
        </label>
      ) : null}
      {kind === "filled" ? (
        <label>
          Fill
          <select data-testid="wall-fill" value={wall?.fill ?? ""} onChange={(event) => onChange({ ...wall, fill: event.target.value })}>
            {fills.map((m) => (
              <option key={m.id} value={m.id}>
                {label(m)}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      {kind === "two_material" ? (
        <label>
          Core filament
          <select data-testid="wall-core" value={wall?.core ?? ""} onChange={(event) => onChange({ ...wall, core: event.target.value })}>
            {cores.map((m) => (
              <option key={m.id} value={m.id}>
                {label(m)}
                {m.foaming ? " (foaming)" : ""}
              </option>
            ))}
          </select>
        </label>
      ) : null}
    </div>
  );
}

type PanelLayer = { material: string; thickness_m: number; turn_deg?: number };

/** A panel of one material, or layers from its back face to its front (README section 5.9). */
export function PanelBuildField({
  panel,
  index,
  catalogs,
  setPanel,
}: {
  panel: Record<string, any>;
  index: number;
  catalogs: Catalogs | null;
  setPanel: (change: (panel: Record<string, any>) => void) => void;
}) {
  const materials = catalogs?.panels ?? [];
  const sheets = materials.filter((m) => !m.core_only);
  const layers: PanelLayer[] | null = Array.isArray(panel.layers) ? panel.layers : null;
  const total = (list: PanelLayer[]) => list.reduce((sum, layer) => sum + Number(layer.thickness_m), 0);
  const setLayers = (list: PanelLayer[]) =>
    setPanel((p) => {
      p.layers = list;
      p.thickness_m = total(list);
      delete p.material;
    });
  const materialSelect = (value: string, onChange: (value: string) => void, testId: string, all: CatalogEntry[]) => (
    <select data-testid={testId} value={value} onChange={(event) => onChange(event.target.value)}>
      {all.some((m) => m.id === value) ? null : <option value={value}>{value || "choose"}</option>}
      {all.map((m) => (
        <option key={m.id} value={m.id}>
          {label(m)}
          {m.status === "synthetic" ? " (test material)" : ""}
        </option>
      ))}
    </select>
  );
  return (
    <div data-testid={`panel-build-${index}`}>
      <label title="One sheet of one material, or a stack of layers: a sandwich (two stiff skins on a light core) or plywood (veneers with the grain turned in turn).">
        Panel build
        <select
          data-testid={`panel-kind-${index}`}
          value={layers ? "layers" : "one"}
          onChange={(event) => {
            if (event.target.value === "one") {
              setPanel((p) => {
                p.material = layers?.[0]?.material ?? sheets[0]?.id ?? "";
                p.thickness_m = layers ? total(layers) : p.thickness_m;
                delete p.layers;
              });
            } else {
              const t = Number(panel.thickness_m ?? 0.003);
              setLayers([{ material: String(panel.material ?? "pla-printed"), thickness_m: t }]);
            }
          }}
        >
          <option value="one">One material</option>
          <option value="layers">Layers (sandwich, plywood)</option>
        </select>
      </label>
      {layers ? (
        <>
          <table data-testid={`panel-layers-${index}`}>
            <thead>
              <tr>
                <th>Layer (back to front)</th>
                <th>Thickness (mm)</th>
                <th title="Turn the layer a quarter turn, so its strong direction runs up the panel instead of across.">Turned 90°</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {layers.map((layer, k) => (
                <tr key={k}>
                  <td>
                    {materialSelect(
                      layer.material,
                      (value) => setLayers(layers.map((l, j) => (j === k ? { ...l, material: value } : l))),
                      `panel-layer-material-${index}-${k}`,
                      materials,
                    )}
                  </td>
                  <td>
                    <input
                      type="number"
                      value={Math.round(layer.thickness_m * 1e5) / 100}
                      onChange={(event) =>
                        setLayers(layers.map((l, j) => (j === k ? { ...l, thickness_m: Number(event.target.value) / 1000 } : l)))
                      }
                    />
                  </td>
                  <td>
                    <input
                      type="checkbox"
                      checked={Number(layer.turn_deg ?? 0) !== 0}
                      onChange={(event) =>
                        setLayers(layers.map((l, j) => (j === k ? { ...l, turn_deg: event.target.checked ? 90 : 0 } : l)))
                      }
                    />
                  </td>
                  <td>
                    <button type="button" disabled={layers.length < 2} onClick={() => setLayers(layers.filter((_, j) => j !== k))}>
                      Remove
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p>
            <button type="button" onClick={() => setLayers([...layers, { material: layers[layers.length - 1].material, thickness_m: 0.001 }])}>
              Add a layer
            </button>{" "}
            <button
              type="button"
              data-testid={`panel-sandwich-${index}`}
              title="0.4 mm printed PLA skins on a 5 mm polypropylene honeycomb core."
              onClick={() =>
                setLayers([
                  { material: "pla-printed", thickness_m: 0.0004 },
                  { material: "thermhex-pp-honeycomb", thickness_m: 0.005 },
                  { material: "pla-printed", thickness_m: 0.0004 },
                ])
              }
            >
              Sandwich
            </button>{" "}
            <button
              type="button"
              title="Three 1 mm birch veneers, the middle one turned."
              onClick={() =>
                setLayers([
                  { material: "birch-veneer", thickness_m: 0.001 },
                  { material: "birch-veneer", thickness_m: 0.001, turn_deg: 90 },
                  { material: "birch-veneer", thickness_m: 0.001 },
                ])
              }
            >
              Plywood
            </button>{" "}
            Total {(total(layers) * 1000).toFixed(2)} mm.
          </p>
        </>
      ) : (
        <label>
          Panel material
          {materialSelect(String(panel.material ?? ""), (value) => setPanel((p) => {
            p.material = value;
          }), `panel-material-${index}`, sheets)}
        </label>
      )}
    </div>
  );
}
