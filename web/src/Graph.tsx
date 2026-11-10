type Props = {
  title: string;
  explanation: string;
  freqs: number[];
  values: number[];
  unit: string;
  testId: string;
  fidelity: string;
};

export function Graph({ title, explanation, freqs, values, unit, testId, fidelity }: Props) {
  const width = 640;
  const height = 220;
  const pad = 40;
  const finite = values.filter((value) => Number.isFinite(value));
  const ymin = finite.length ? Math.min(...finite) : 0;
  const ymax = finite.length ? Math.max(...finite) : 1;
  const span = Math.max(ymax - ymin, 1e-6);
  const logMin = Math.log(20);
  const logMax = Math.log(20_000);
  const points = freqs
    .map((freq, index) => {
      const value = values[index];
      if (freq < 20 || freq > 20_000 || !Number.isFinite(value)) {
        return null;
      }
      const x = pad + ((Math.log(freq) - logMin) / (logMax - logMin)) * (width - pad * 2);
      const y = height - pad - ((value - ymin) / span) * (height - pad * 2);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .filter((point): point is string => point !== null)
    .join(" ");

  return (
    <figure className="graph">
      <figcaption>
        {title} <span className="unit">({unit})</span>
      </figcaption>
      <p className="explain">{explanation}</p>
      <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={title} data-testid={testId}>
        <rect x="0" y="0" width={width} height={height} fill="#fffaf4" />
        <polyline fill="none" stroke="#2f5d50" strokeWidth="2" points={points} />
        <text x={pad} y={18} fontSize="12">
          {ymax.toFixed(1)}
        </text>
        <text x={pad} y={height - 8} fontSize="12">
          {ymin.toFixed(1)} · 20 Hz to 20 kHz
        </text>
      </svg>
      <p className="fidelity">{fidelity}</p>
    </figure>
  );
}

export function nearestValue(freqs: number[], values: number[], targetHz: number): number {
  let best = 0;
  let bestDistance = Number.POSITIVE_INFINITY;
  freqs.forEach((freq, index) => {
    const distance = Math.abs(Math.log(freq) - Math.log(targetHz));
    if (distance < bestDistance) {
      bestDistance = distance;
      best = index;
    }
  });
  return values[best] ?? Number.NaN;
}

export function displayGrid(): number[] {
  const start = 20;
  const end = 20_000;
  const perOctave = 48;
  const steps = Math.round(Math.log2(end / start) * perOctave);
  const freqs: number[] = [];
  for (let i = 0; i <= steps; i += 1) {
    const freq = start * 2 ** (i / perOctave);
    if (freq < end) {
      freqs.push(freq);
    }
  }
  freqs.push(end);
  return freqs;
}
