export type WasmEngineExports = {
  memory: WebAssembly.Memory;
  seatlock_alloc: (len: number) => number;
  seatlock_free: (ptr: number, len: number) => void;
  seatlock_call: (
    methodPtr: number,
    methodLen: number,
    reqPtr: number,
    reqLen: number,
    outLenPtr: number,
  ) => number;
};

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function writeBytes(memory: WebAssembly.Memory, ptr: number, data: Uint8Array) {
  new Uint8Array(memory.buffer).set(data, ptr);
}

function readBytes(memory: WebAssembly.Memory, ptr: number, len: number) {
  return new Uint8Array(memory.buffer.slice(ptr, ptr + len));
}

export class WasmEngine {
  private readonly exports: WasmEngineExports;

  constructor(exports: WebAssembly.Exports) {
    this.exports = exports as WasmEngineExports;
  }

  call(method: string, request = "{}"): string {
    const memory = this.exports.memory;
    const methodBytes = encoder.encode(method);
    const reqBytes = encoder.encode(request);

    const methodPtr = this.exports.seatlock_alloc(methodBytes.length);
    const reqPtr = this.exports.seatlock_alloc(reqBytes.length);
    const outLenPtr = this.exports.seatlock_alloc(8);
    if (!methodPtr || !reqPtr || !outLenPtr) {
      throw new Error("Engine out of memory");
    }

    writeBytes(memory, methodPtr, methodBytes);
    writeBytes(memory, reqPtr, reqBytes);
    new DataView(memory.buffer).setUint32(outLenPtr, 0, true);

    const outPtr = this.exports.seatlock_call(
      methodPtr,
      methodBytes.length,
      reqPtr,
      reqBytes.length,
      outLenPtr,
    );
    const outLen = new DataView(memory.buffer).getUint32(outLenPtr, true);

    this.exports.seatlock_free(methodPtr, methodBytes.length);
    this.exports.seatlock_free(reqPtr, reqBytes.length);
    this.exports.seatlock_free(outLenPtr, 8);

    if (!outPtr || outLen === 0) {
      throw new Error("Empty reply from engine");
    }

    const outBytes = readBytes(memory, outPtr, outLen);
    const text = decoder.decode(outBytes);
    this.exports.seatlock_free(outPtr, outLen);
    return text;
  }
}

/** `onProgress(steps, maxSteps)` hears from long engine runs; returning false stops the run. */
export async function loadWasmEngine(
  url: string,
  onProgress?: (steps: number, maxSteps: number) => boolean,
): Promise<WasmEngine> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to load engine wasm: ${response.status}`);
  }
  const bytes = await response.arrayBuffer();
  const imports = {
    env: {
      seatlock_progress: (steps: number, maxSteps: number) =>
        onProgress?.(steps, maxSteps) === false ? 0 : 1,
    },
  };
  const { instance } = await WebAssembly.instantiate(bytes, imports);
  return new WasmEngine(instance.exports);
}
