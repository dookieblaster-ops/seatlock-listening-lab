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

  const correct = useCallback(
    async (text: string) => {
      const json = await call({ type: "correct", text });
      return JSON.parse(json) as {
        message: string;
        filters: Array<Record<string, unknown>>;
        per_radiator?: Array<{ message: string; filters: Array<Record<string, unknown>> }>;
        error?: string;
      };
    },
    [call],
  );

  const listen = useCallback(
    async (text: string, correction: boolean, impulseLen: number, sampleRate: number) => {
      const json = await call({
        type: "listen",
        text,
        correction,
        impulseLen,
        sampleRate,
      });
      return JSON.parse(json) as ListenPackage;
    },
    [call],
  );

  const driverRecord = useCallback(
    async (driverId: string) => {
      const json = await call({ type: "catalog.driver", driverId });
      return JSON.parse(json) as {
        status?: string;
        maker?: string;
        model?: string;
        tier?: string;
        price?: { amount?: number; currency?: string; vendor?: string; read_on?: string; via?: string } | null;
        mounting?: { via?: string };
        error?: string;
      };
    },
    [call],
  );

  const catalogList = useCallback(
    async (folder: "panels" | "damping" | "fills" | "materials") => {
      const json = await call({ type: "call", method: "catalog.list", request: JSON.stringify({ text: folder }) });
      return JSON.parse(json) as CatalogEntry[];
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

  return { version, ready, error, designCheck, simulate, geometry, correct, listen, driverRecord, catalogList };
}

export type ListenPackage = {
  sample_rate_hz: number;
  channels: Array<{ coeffs: number[][]; gain: number; delay_samples: number }>;
  impulses: number[][];
  filter_text: string;
  spl_1k_db: number;
  correction_on: boolean;
  error?: string;
};

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
  cost: CostReport;
  print_tree: import("./printSolid").PrintTree;
  print_split?: import("./printSolid").PrintSplit & { pins_m3: number };
  print_note?: string;
  cut_list?: string[];
  warnings?: string[];
  can_export_print?: boolean;
  mounts: Array<{
    position_m: number[];
    outward: number[];
    cutout_diameter_m: number;
  }>;
  error?: string;
};

export type CostLine = {
  item: string;
  quantity: number;
  unit: string;
  unit_usd: number | null;
  total_usd: number | null;
  read_on: string | null;
  unconfirmed: boolean;
};

export type CostReport = {
  speakers: number;
  lines: CostLine[];
  known_total_usd: number;
  complete: boolean;
  unconfirmed: boolean;
  summary: string;
  problems: string[];
  cut_list: string[];
  print_mass_kg: number;
  print_time_h_at_least: number | null;
};

export type SimulateResult = {
  revision: number;
  frequencies_hz: number[];
  summed_spl_db: number[];
  radiators: Array<{
    spl_db: number[];
    impedance_ohm: number[];
    cone_travel_m: number[];
    port_speed_m?: number[];
    max_spl_db: number[];
    fidelity: { enclosure: string; radiation: string; room: string; ignored: string[] };
  }>;
  issues: Array<{ severity: string; code: string; message: string; field: string }>;
  fidelity: { enclosure: string; radiation: string; room: string; ignored: string[] };
  cost_note: string;
  cost: CostReport;
  interior_volume_m3: number | null;
  f6_hz: number;
  flatness_db: number;
  axis_flatness_db?: number;
  window_flatness_db?: number;
  max_spl_metric_db: number;
  fdtd?: FdtdReport | null;
  /** Sound power level, dB re 1 pW, when a room or the request asked for it. */
  sound_power_db?: number[] | null;
  directivity_index_db?: number[] | null;
  /** The in-room response when the design has a room; the score then comes from it. */
  in_room?: {
    spl_db: number[];
    early_spl_db: number[];
    reflection_spl_db: number[];
    diffuse_spl_db: number[];
  } | null;
  error?: string;
};

export type FdtdReport = {
  dx_m: number;
  cells: number;
  steps: number;
  decayed: boolean;
  elapsed_ms: number;
  valid_up_to_hz: number;
  grid_volume_m3: number;
  net_volume_m3: number;
  self_check_error: number | null;
  frequencies_hz: number[];
  z_ab: { re: number[]; im: number[] };
  z_ab_quick: { re: number[]; im: number[] };
  port_ids: string[];
};

export type CatalogEntry = {
  id: string;
  name: string | null;
  foaming?: boolean;
  core_only?: boolean;
  flow_resistivity_pa_s_m2?: number | null;
  status?: string;
};
