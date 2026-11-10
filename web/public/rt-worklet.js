class SeatlockRtProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const wasmBytes = options.processorOptions?.wasmBytes;
    if (!wasmBytes) {
      throw new Error("Missing real-time wasm bytes");
    }
    const module = new WebAssembly.Module(wasmBytes);
    const instance = new WebAssembly.Instance(module, {});
    this.exports = instance.exports;
    const rate = sampleRate || 48000;
    this.exports.seatlock_rt_init(rate);
    this.exports.seatlock_rt_set_tone(440, 0.15);
    this.capacity = this.exports.seatlock_rt_output_capacity();
    this.outPtr = this.exports.seatlock_rt_output_ptr();
    this.boundBuffer = null;
    this.outView = null;
    this.bindOutput();
  }

  bindOutput() {
    const memory = this.exports.memory;
    this.outView = new Float32Array(memory.buffer, this.outPtr, this.capacity);
    this.boundBuffer = memory.buffer;
  }

  process(_inputs, outputs) {
    const channel = outputs[0]?.[0];
    if (!channel) {
      return true;
    }
    if (this.boundBuffer !== this.exports.memory.buffer) {
      this.bindOutput();
    }
    let offset = 0;
    while (offset < channel.length) {
      const frames = Math.min(this.capacity, channel.length - offset);
      this.exports.seatlock_rt_process(frames);
      channel.set(this.outView.subarray(0, frames), offset);
      offset += frames;
    }
    return true;
  }
}

registerProcessor("seatlock-rt", SeatlockRtProcessor);
