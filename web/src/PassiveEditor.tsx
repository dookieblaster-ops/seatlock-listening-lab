/** Passive R/L/C crossover board on the page (README section 7). */

export type PassiveNetwork = {
  source: [string, string];
  elements: Array<Record<string, unknown>>;
};

export type PassiveEditorProps = {
  value: PassiveNetwork | null | undefined;
  radiatorCount: number;
  onChange: (next: PassiveNetwork | null) => void;
};

/** A starter board: one driver wired across the amp, or a first-order two-way split. */
export function defaultPassive(radiatorCount: number): PassiveNetwork {
  if (radiatorCount >= 2) {
    return {
      source: ["amp", "gnd"],
      elements: [
        { kind: "L", a: "amp", b: "w", henry: 0.0015 },
        { kind: "driver", a: "w", b: "gnd", radiator: 0 },
        { kind: "C", a: "amp", b: "t", farad: 3.3e-6 },
        { kind: "driver", a: "t", b: "gnd", radiator: 1 },
      ],
    };
  }
  return {
    source: ["amp", "gnd"],
    elements: [{ kind: "driver", a: "amp", b: "gnd", radiator: 0 }],
  };
}

function kindOf(row: Record<string, unknown>): string {
  return String(row.kind ?? "R");
}

function valueLabel(kind: string): string {
  if (kind === "L") {
    return "Inductance (mH)";
  }
  if (kind === "C") {
    return "Capacitance (µF)";
  }
  return "Resistance (ohm)";
}

function displayValue(row: Record<string, unknown>): number {
  const kind = kindOf(row);
  if (kind === "L") {
    return Number(row.henry ?? 0) * 1000;
  }
  if (kind === "C") {
    return Number(row.farad ?? 0) * 1e6;
  }
  return Number(row.ohm ?? 0);
}

function storeValue(kind: string, display: number): Record<string, number> {
  if (kind === "L") {
    return { henry: display / 1000 };
  }
  if (kind === "C") {
    return { farad: display / 1e6 };
  }
  return { ohm: display };
}

/** Put another driver in series with the last driver load (two woofers, one current). */
export function addSeriesDriver(network: PassiveNetwork, radiatorCount: number): PassiveNetwork {
  const elements = network.elements.map((row) => ({ ...row }));
  const used = new Set(
    elements.filter((row) => row.kind === "driver").map((row) => Number(row.radiator)),
  );
  let radiator = 0;
  while (used.has(radiator) && radiator < Math.max(1, radiatorCount)) {
    radiator += 1;
  }
  if (radiator >= Math.max(1, radiatorCount)) {
    radiator = Math.max(0, radiatorCount - 1);
  }
  let last = -1;
  elements.forEach((row, index) => {
    if (row.kind === "driver") {
      last = index;
    }
  });
  if (last < 0) {
    elements.push({ kind: "driver", a: network.source[0], b: network.source[1], radiator });
    return { ...network, elements };
  }
  const join = `series-${radiator}`;
  elements[last] = { ...elements[last], b: join };
  elements.push({ kind: "driver", a: join, b: network.source[1], radiator });
  return { ...network, elements };
}

function blankPart(kind: string, radiatorCount: number): Record<string, unknown> {
  if (kind === "driver") {
    return { kind: "driver", a: "amp", b: "gnd", radiator: Math.max(0, radiatorCount - 1) };
  }
  return { kind, a: "amp", b: "gnd", ...storeValue(kind, kind === "L" ? 1.5 : kind === "C" ? 3.3 : 6) };
}

export function PassiveEditor({ value, radiatorCount, onChange }: PassiveEditorProps) {
  const on = value != null;
  const network = value ?? defaultPassive(radiatorCount);

  const setNetwork = (next: PassiveNetwork) => onChange(next);
  const patchElement = (index: number, patch: Record<string, unknown>) => {
    const elements = network.elements.map((row, i) => (i === index ? { ...row, ...patch } : row));
    setNetwork({ ...network, elements });
  };

  return (
    <div data-testid="passive-editor">
      <h3 title="Parts between one amplifier and the drivers, the way a kit crossover board works. When this is on, the graphs and Play use it instead of the digital filters on each driver.">
        Passive crossover board
      </h3>
      <label className="check">
        <input
          type="checkbox"
          data-testid="passive-enabled"
          checked={on}
          onChange={(event) => {
            onChange(event.target.checked ? (value ?? defaultPassive(radiatorCount)) : null);
          }}
        />
        Use a passive crossover board (one amp, R/L/C parts to the drivers)
      </label>
      {!on && (
        <p data-testid="passive-off-note">
          Off: each driver has its own digital filters and its own amplifier channel.
        </p>
      )}
      {on && (
        <>
          <p data-testid="passive-on-note">
            On: the board sets each driver&apos;s voltage. Digital filters listed on each driver
            are left out of the graphs and Play; delay, level, and polarity still apply.
          </p>
          <div className="grid">
            <label>
              Amplifier plus node
              <input
                data-testid="passive-source-plus"
                value={network.source[0]}
                onChange={(event) =>
                  setNetwork({ ...network, source: [event.target.value, network.source[1]] })
                }
              />
            </label>
            <label>
              Amplifier ground node
              <input
                data-testid="passive-source-gnd"
                value={network.source[1]}
                onChange={(event) =>
                  setNetwork({ ...network, source: [network.source[0], event.target.value] })
                }
              />
            </label>
          </div>
          {network.elements.map((row, index) => {
            const kind = kindOf(row);
            return (
              <div className="grid" key={`passive-${index}`} data-testid={`passive-part-${index}`}>
                <label>
                  Part
                  <select
                    data-testid={`passive-kind-${index}`}
                    value={kind}
                    onChange={(event) => {
                      const nextKind = event.target.value;
                      patchElement(index, blankPart(nextKind, radiatorCount));
                    }}
                  >
                    <option value="R">Resistor</option>
                    <option value="L">Inductor</option>
                    <option value="C">Capacitor</option>
                    <option value="driver">Driver load</option>
                  </select>
                </label>
                <label>
                  From node
                  <input
                    data-testid={`passive-a-${index}`}
                    value={String(row.a ?? "")}
                    onChange={(event) => patchElement(index, { a: event.target.value })}
                  />
                </label>
                <label>
                  To node
                  <input
                    data-testid={`passive-b-${index}`}
                    value={String(row.b ?? "")}
                    onChange={(event) => patchElement(index, { b: event.target.value })}
                  />
                </label>
                {kind === "driver" ? (
                  <label>
                    Driver number
                    <select
                      data-testid={`passive-radiator-${index}`}
                      value={String(row.radiator ?? 0)}
                      onChange={(event) =>
                        patchElement(index, { radiator: Number(event.target.value) })
                      }
                    >
                      {Array.from({ length: Math.max(1, radiatorCount) }, (_, i) => (
                        <option key={i} value={i}>
                          Driver {i + 1}
                        </option>
                      ))}
                    </select>
                  </label>
                ) : (
                  <label>
                    {valueLabel(kind)}
                    <input
                      data-testid={`passive-value-${index}`}
                      type="number"
                      value={Number.isFinite(displayValue(row)) ? displayValue(row) : ""}
                      onChange={(event) =>
                        patchElement(index, storeValue(kind, Number(event.target.value)))
                      }
                    />
                  </label>
                )}
                <button
                  type="button"
                  data-testid={`passive-remove-${index}`}
                  onClick={() => {
                    const elements = network.elements.filter((_, i) => i !== index);
                    setNetwork({ ...network, elements });
                  }}
                >
                  Remove this part
                </button>
              </div>
            );
          })}
          <div className="grid">
            <button
              type="button"
              data-testid="passive-add-series"
              title="Two woofers that share one current, the way a tower kit wires a pair in series. The new driver sits below the last driver load."
              onClick={() => onChange(addSeriesDriver(network, radiatorCount))}
            >
              Add a driver in series
            </button>
            {(["R", "L", "C", "driver"] as const).map((kind) => (
              <button
                key={kind}
                type="button"
                data-testid={`passive-add-${kind.toLowerCase()}`}
                onClick={() =>
                  setNetwork({
                    ...network,
                    elements: [...network.elements, blankPart(kind, radiatorCount)],
                  })
                }
              >
                Add {kind === "driver" ? "a driver load" : kind === "R" ? "a resistor" : kind === "L" ? "an inductor" : "a capacitor"}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
