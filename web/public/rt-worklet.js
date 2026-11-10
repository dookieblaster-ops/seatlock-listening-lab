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
  }

  process(_inputs, outputs) {
    const channel = outputs[0]?.[0];
    if (!channel) {
      return true;
    }
    this.exports.seatlock_rt_process(channel, channel.length);
    return true;
  }
}

registerProcessor("seatlock-rt", SeatlockRtProcessor);
