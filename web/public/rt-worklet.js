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
    this.inPtr = this.exports.seatlock_rt_input_ptr();
    this.boundBuffer = null;
    this.views = [];
    this.inView = null;
    this.useInput = false;
    this.bindOutput();
    const preset = options.processorOptions?.coeffs;
    if (preset && typeof preset.length === "number" && preset.length > 1) {
      this.loadCoefficients(preset);
    }
    this.port.onmessage = (event) => {
      const data = event.data;
      // A typed array posted from the page fails `instanceof` inside the worklet.
      if (!data || typeof data.length !== "number" || data.length < 1 || typeof data[0] !== "number") {
        return;
      }
      const command = data[0];
      if (command === 1) {
        this.exports.seatlock_rt_set_source(data[1] || 0, data[2] || 0, data[3] || 0);
        this.useInput = data[1] === 3;
      } else if (command === 2) {
        this.exports.seatlock_rt_set_voltage(data[1] || 0);
      } else if (command === 3) {
        this.loadCoefficients(data);
      }
    };
  }

  bindOutput() {
    const memory = this.exports.memory;
    this.views = [];
    for (let channel = 0; channel < 16; channel += 1) {
      this.views.push(
        new Float32Array(memory.buffer, this.outPtr + channel * this.capacity * 4, this.capacity),
      );
    }
    this.inView = new Float32Array(memory.buffer, this.inPtr, this.capacity);
    this.boundBuffer = memory.buffer;
  }

  loadCoefficients(data) {
    let index = 1;
    const channels = data[index] || 1;
    index += 1;
    this.exports.seatlock_rt_set_channels(channels);
    for (let channel = 0; channel < channels; channel += 1) {
      this.exports.seatlock_rt_clear_channel(channel);
      const count = data[index] || 0;
      const gain = data[index + 1];
      const delay = data[index + 2] || 0;
      index += 3;
      this.exports.seatlock_rt_set_channel_gain(channel, gain);
      this.exports.seatlock_rt_set_delay(channel, delay);
      for (let stage = 0; stage < count; stage += 1) {
        this.exports.seatlock_rt_add_biquad(
          channel,
          data[index],
          data[index + 1],
          data[index + 2],
          data[index + 3],
          data[index + 4],
        );
        index += 5;
      }
    }
    this.exports.seatlock_rt_commit();
  }

  process(inputs, outputs) {
    const channels = outputs[0];
    const first = channels?.[0];
    if (!first) {
      return true;
    }
    if (this.boundBuffer !== this.exports.memory.buffer) {
      this.bindOutput();
    }
    let offset = 0;
    while (offset < first.length) {
      const frames = Math.min(this.capacity, first.length - offset);
      const incoming = inputs[0]?.[0];
      if (this.useInput && incoming && this.inView) {
        this.inView.fill(0);
        const available = Math.max(0, incoming.length - offset);
        const count = Math.min(frames, available);
        if (count === this.capacity && incoming.length === this.capacity) {
          this.inView.set(incoming);
        } else if (count > 0) {
          this.inView.set(incoming.subarray(offset, offset + count));
        }
      }
      this.exports.seatlock_rt_process(frames);
      for (let channel = 0; channel < channels.length; channel += 1) {
        const output = channels[channel];
        const view = this.views[channel];
        if (!output || !view) {
          continue;
        }
        if (output.length === this.capacity && frames === this.capacity) {
          output.set(view);
        } else {
          output.set(view.subarray(0, frames), offset);
        }
      }
      offset += frames;
    }
    return true;
  }
}

registerProcessor("seatlock-rt", SeatlockRtProcessor);
