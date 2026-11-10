import { useCallback, useRef, useState } from "react";

export function useTestTone() {
  const contextRef = useRef<AudioContext | null>(null);
  const nodeRef = useRef<AudioWorkletNode | null>(null);
  const [playing, setPlaying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const start = useCallback(async () => {
    setError(null);
    const response = await fetch("/seatlock_rt.wasm");
    const wasmBytes = await response.arrayBuffer();

    const ctx = contextRef.current ?? new AudioContext();
    if (ctx.state === "suspended") {
      await ctx.resume();
    }
    contextRef.current = ctx;

    await ctx.audioWorklet.addModule("/rt-worklet.js");
    const node = new AudioWorkletNode(ctx, "seatlock-rt", {
      processorOptions: { wasmBytes },
    });
    node.connect(ctx.destination);
    nodeRef.current = node;
    setPlaying(true);
  }, []);

  const stop = useCallback(async () => {
    nodeRef.current?.disconnect();
    nodeRef.current = null;
    if (contextRef.current) {
      await contextRef.current.suspend();
    }
    setPlaying(false);
  }, []);

  return { playing, error, start, stop, setError };
}
