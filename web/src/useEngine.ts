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

  const call = useCallback(async (message: object): Promise<string> => {
    if (!workerRef.current) {
      throw new Error("Engine worker not ready");
    }
    const id = nextId.current++;
    const json = await new Promise<string>((resolve, reject) => {
      pending.current.set(id, { resolve, reject });
      workerRef.current?.postMessage({ ...message, id });
    });
    return json;
  }, []);

  const designCheck = useCallback(
    async (text: string): Promise<DesignCheckResult> => {
      const json = await call({ type: "design.check", text });
      return JSON.parse(json) as DesignCheckResult;
    },
    [call],
  );

  const geometry = useCallback(
    async (text: string) => {
      const json = await call({ type: "geometry", text });
      return JSON.parse(json) as GeometryReport;
    },
    [call],
  );

  const simulate = useCallback(
    async (text: string, revision: number, frequencies: number[]) => {
      const json = await call({ type: "simulate", text, revision, frequencies });
      return JSON.parse(json) as SimulateResult;
    },
    [call],
  );

  return { version, ready, error, designCheck, simulate, geometry };
}

export type GeometryReport = {
  width_m: number;
  height_m: number;
  depth_m: number;
  wall_m: number;
  interior_volume_m3: number;
  solid_volume_m3: number;
  voxel_air_volume_m3: number;
  print_mass_kg: number;
  cost_note: string;
  mounts: Array<{
    position_m: number[];
    outward: number[];
    cutout_diameter_m: number;
  }>;
  error?: string;
};

export type SimulateResult = {
  revision: number;
  frequencies_hz: number[];
  summed_spl_db: number[];
  radiators: Array<{
    spl_db: number[];
    impedance_ohm: number[];
    cone_travel_m: number[];
    fidelity: { enclosure: string; radiation: string; room: string; ignored: string[] };
  }>;
  issues: Array<{ severity: string; code: string; message: string; field: string }>;
  fidelity: { enclosure: string; radiation: string; room: string; ignored: string[] };
  cost_note: string;
  error?: string;
};
