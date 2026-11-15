import { describe, expect, it } from "vitest";

import { defaultPassive } from "./PassiveEditor";

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
});
