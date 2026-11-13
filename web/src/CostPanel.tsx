import type { CostReport } from "./useEngine";

function money(value: number | null) {
  return value === null ? "no price yet" : `$${value.toFixed(2)}`;
}

function amount(quantity: number, unit: string) {
  if (unit === "kg") return `${(quantity * 1000).toFixed(0)} g`;
  const whole = Number.isInteger(quantity) ? quantity.toFixed(0) : quantity.toFixed(2);
  return unit === "piece" ? `${whole} piece${quantity === 1 ? "" : "s"}` : `${whole}`;
}

/** Cost of the whole system, line by line (README section 9.2). Physics stays in the engine. */
export function CostPanel({ cost, printNote }: { cost: CostReport; printNote?: string }) {
  return (
    <div className="cost" title="What the design costs to build, from catalog prices with their dates.">
      <p data-testid="cost-note">
        <strong>Cost:</strong> {cost.summary}
      </p>
      <table className="cost-lines">
        <tbody>
          {cost.lines.map((line, index) => (
            <tr key={`${index}-${line.item}`}>
              <td>{line.item}</td>
              <td>{amount(line.quantity, line.unit)}</td>
              <td>
                {money(line.total_usd)}
                {line.unconfirmed ? " (unconfirmed)" : ""}
              </td>
              <td className="muted">{line.read_on ? `price as of ${line.read_on}` : ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {cost.print_mass_kg > 0 && (
        <p data-testid="print-time">
          Printing: {(cost.print_mass_kg * 1000).toFixed(0)} g of plastic, at least{" "}
          {(cost.print_time_h_at_least ?? 0).toFixed(1)} hours on the printer.
          {printNote ? ` ${printNote}` : ""}
        </p>
      )}
      {cost.cut_list.length > 0 && (
        <ul data-testid="cut-list">
          {cost.cut_list.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}
      {cost.problems.map((line) => (
        <p key={line} className="warning">
          {line}
        </p>
      ))}
    </div>
  );
}
