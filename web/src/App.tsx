import { useRef, useState } from "react";

import type { Issue } from "./generated/Issue";
import { useEngine } from "./useEngine";
import { useTestTone } from "./useTestTone";
import "./App.css";

export default function App() {
  const { version, ready, error: engineError, designCheck } = useEngine();
  const tone = useTestTone();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const [designName, setDesignName] = useState("No design loaded");
  const [canonical, setCanonical] = useState<string | null>(null);
  const [issues, setIssues] = useState<Issue[]>([]);
  const [status, setStatus] = useState<string>("");

  async function checkDesign(text: string, label: string) {
    const result = await designCheck(text);
    setCanonical(result.canonical_json);
    setIssues(result.issues);
    setDesignName(label);
    setStatus(
      result.issues.length === 0
        ? "Design checked with no warnings."
        : `${result.issues.length} notice(s) from the engine.`,
    );
  }

  async function onOpenFile(file: File) {
    const text = await file.text();
    await checkDesign(text, file.name);
  }

  function onSave() {
    if (!canonical) {
      return;
    }
    const blob = new Blob([canonical], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = designName.endsWith(".seatlock.json")
      ? designName
      : "design.seatlock.json";
    anchor.click();
    URL.revokeObjectURL(url);
  }

  async function loadExample() {
    const response = await fetch("/examples/sealed-test.seatlock.json");
    const text = await response.text();
    await checkDesign(text, "sealed-test.seatlock.json");
  }

  return (
    <main className="app">
      <header>
        <h1>Seatlock Listening Lab</h1>
        <p className="lede">
          Try speaker ideas before you build them. Level 0: open a design, hear a test tone, and
          save your file.
        </p>
      </header>

      <section className="panel">
        <h2>Engine</h2>
        <p data-testid="engine-version">
          Version <strong>{ready ? version : "loading…"}</strong>
        </p>
        {engineError && (
          <p className="error" data-testid="engine-error">
            {engineError}
          </p>
        )}
      </section>

      <section className="panel">
        <h2>Design file</h2>
        <div className="row">
          <button type="button" onClick={() => fileInputRef.current?.click()}>
            Open design…
          </button>
          <input
            ref={fileInputRef}
            type="file"
            accept=".json,.seatlock.json,application/json"
            hidden
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) {
                void onOpenFile(file);
              }
            }}
          />
          <button type="button" onClick={() => void loadExample()} data-testid="load-example">
            Load example sealed box
          </button>
          <button type="button" onClick={onSave} disabled={!canonical}>
            Save design
          </button>
        </div>
        <p>
          Current file: <span data-testid="design-name">{designName}</span>
        </p>
        <p data-testid="design-status">{status}</p>
        {issues.length > 0 && (
          <ul className="issues" data-testid="issue-list">
            {issues.map((issue) => (
              <li key={`${issue.code}-${issue.field}`}>
                <span className="issue-field">{issue.field || "(whole file)"}</span>:{" "}
                {issue.message}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="panel">
        <h2>Listen</h2>
        <p>A short test tone from the same sound code the lab will use for playback later.</p>
        <div className="row">
          <button type="button" onClick={() => void tone.start()} disabled={tone.playing}>
            Play test tone
          </button>
          <button type="button" onClick={() => void tone.stop()} disabled={!tone.playing}>
            Stop
          </button>
        </div>
        {tone.error && <p className="error">{tone.error}</p>}
      </section>
    </main>
  );
}
