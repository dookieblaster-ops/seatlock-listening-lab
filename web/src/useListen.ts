import { useCallback, useRef, useState } from "react";

export type ListenChannel = {
  coeffs: number[][];
  gain: number;
  delay_samples: number;
};

export function coefficientMessage(channels: ListenChannel[]): Float64Array {
  const values: number[] = [3, channels.length];
  for (const channel of channels) {
    values.push(channel.coeffs.length, channel.gain, channel.delay_samples);
    for (const row of channel.coeffs) {
      values.push(row[0] ?? 1, row[1] ?? 0, row[2] ?? 0, row[3] ?? 0, row[4] ?? 0);
    }
  }
  return new Float64Array(values);
}

const SOURCE_KIND = { tone: 0, pink: 1, sweep: 2, file: 3 } as const;

export type ListenSource = keyof typeof SOURCE_KIND;

export function useListen() {
  const contextRef = useRef<AudioContext | null>(null);
  const nodeRef = useRef<AudioWorkletNode | null>(null);
  const extrasRef = useRef<AudioNode[]>([]);
  const workletReady = useRef(false);
  const [playing, setPlaying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const send = useCallback((values: Float64Array) => {
    nodeRef.current?.port.postMessage(values);
  }, []);

  const stop = useCallback(async () => {
    for (const node of extrasRef.current) {
      node.disconnect();
    }
    extrasRef.current = [];
    nodeRef.current?.disconnect();
    nodeRef.current = null;
    if (contextRef.current) {
      await contextRef.current.suspend();
    }
    setPlaying(false);
  }, []);

  const start = useCallback(
    async (options: {
      source: ListenSource;
      volts: number;
      file: ArrayBuffer | null;
      prepare: (sampleRate: number) => Promise<{ channels: ListenChannel[]; impulses: number[][] }>;
    }) => {
      setError(null);
      const context = contextRef.current ?? new AudioContext();
      contextRef.current = context;
      if (context.state === "suspended") {
        await context.resume();
      }

      await stop();
      if (context.state === "suspended") {
        await context.resume();
      }
      const prepared = await options.prepare(context.sampleRate);

      const wasmBytes = await fetch("/seatlock_rt.wasm").then((response) => response.arrayBuffer());
      if (!workletReady.current) {
        await context.audioWorklet.addModule("/rt-worklet.js");
        workletReady.current = true;
      }
      const count = Math.max(1, Math.min(4, prepared.channels.length));
      const node = new AudioWorkletNode(context, "seatlock-rt", {
        numberOfInputs: 1,
        numberOfOutputs: 1,
        outputChannelCount: [count],
        processorOptions: { wasmBytes },
      });
      nodeRef.current = node;

      const splitter = context.createChannelSplitter(count);
      const sum = context.createGain();
      sum.gain.value = 0.25;
      node.connect(splitter);
      for (let index = 0; index < count; index += 1) {
        const convolver = context.createConvolver();
        convolver.normalize = false;
        const samples = prepared.impulses[index] ?? [1];
        const buffer = context.createBuffer(1, samples.length, context.sampleRate);
        buffer.getChannelData(0).set(samples);
        convolver.buffer = buffer;
        splitter.connect(convolver, index);
        convolver.connect(sum);
        extrasRef.current.push(convolver);
      }
      sum.connect(context.destination);
      extrasRef.current.push(splitter, sum);

      send(coefficientMessage(prepared.channels.slice(0, count)));
      send(new Float64Array([2, options.volts]));
      const kind = SOURCE_KIND[options.source];
      send(new Float64Array([1, kind, options.source === "tone" ? 440 : 3, 0.2]));

      if (options.source === "file" && options.file) {
        const audio = await context.decodeAudioData(options.file.slice(0));
        const player = context.createBufferSource();
        player.buffer = audio;
        player.loop = true;
        player.connect(node);
        player.start();
        extrasRef.current.push(player);
      }

      setPlaying(true);
    },
    [send, stop],
  );

  return { playing, error, setError, start, stop, send };
}
