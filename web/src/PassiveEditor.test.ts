import { describe, expect, it } from "vitest";

import { addSeriesDriver, defaultPassive } from "./PassiveEditor";

describe("defaultPassive", () => {
  it("wires one driver straight across the amplifier", () => {
    expect(defaultPassive(1)).toEqual({
      source: ["amp", "gnd"],
      elements: [{ kind: "driver", a: "amp", b: "gnd", radiator: 0 }],
    });
  });

  it("starts a two-way with a first-order LR/CR split", () => {
    const network = defaultPassive(2);
    expect(network.source).toEqual(["amp", "gnd"]);
    expect(network.elements).toHaveLength(4);
    expect(network.elements.map((row) => row.kind)).toEqual(["L", "driver", "C", "driver"]);
    expect(network.elements[1].radiator).toBe(0);
    expect(network.elements[3].radiator).toBe(1);
  });

  it("puts the next driver in series under the last driver load", () => {
    const network = addSeriesDriver(defaultPassive(1), 2);
    expect(network.elements.map((row) => row.kind)).toEqual(["driver", "driver"]);
    expect(network.elements[0]).toMatchObject({ a: "amp", b: "series-1", radiator: 0 });
    expect(network.elements[1]).toMatchObject({ a: "series-1", b: "gnd", radiator: 1 });
  });
});
