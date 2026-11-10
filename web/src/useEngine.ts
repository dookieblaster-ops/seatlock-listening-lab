import { useCallback, useEffect, useRef, useState } from "react";

import type { DesignCheckResult } from "./engineWorker";

type EngineWorker = Worker;

export function useEngine() {
  const workerRef = useRef<EngineWorker | null>(null);
  const nextId = useRef(1);
  const pending = useRef(
    new Map<number, { resolve: (json: string) => void; reject: (err: Error) => void }>(),
  );

  const [version, setVersion] = useState<string>("…");
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const worker = new Worker(new URL("./engineWorker.ts", import.meta.url), {
      type: "module",
    });
    workerRef.current = worker;

    worker.onerror = () => {
      setError("The simulation engine worker failed to start.");
    };

    worker.onmessage = (event) => {
      const data = event.data;
      if (data.type === "ready") {
        setVersion(data.version);
        setReady(true);
        return;
      }
      if (data.type === "error") {
        if (data.id !== undefined) {
          pending.current.get(data.id)?.reject(new Error(data.message));
          pending.current.delete(data.id);
        } else {
          setError(data.message);
        }
        return;
      }
      if (data.type === "result") {
        pending.current.get(data.id)?.resolve(data.json);
        pending.current.delete(data.id);
      }
    };

    worker.postMessage({ type: "init" });

    return () => {
      worker.terminate();
    };
  }, []);

  const designCheck = useCallback(async (text: string): Promise<DesignCheckResult> => {
    if (!workerRef.current) {
      throw new Error("Engine worker not ready");
    }
    const id = nextId.current++;
    const json = await new Promise<string>((resolve, reject) => {
      pending.current.set(id, { resolve, reject });
      workerRef.current?.postMessage({ type: "design.check", id, text });
    });
    return JSON.parse(json) as DesignCheckResult;
  }, []);

  return { version, ready, error, designCheck };
}
