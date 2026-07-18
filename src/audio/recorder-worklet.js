// AudioWorkletProcessor that linearly downsamples the input to 16 kHz mono
// and posts little-endian Int16 PCM chunks to the main thread every ~250 ms.
//
// Linear interpolation has no anti-alias filter, so content above 8 kHz will
// fold back into the audible band. That is fine for speech (voice energy is
// mostly below 4 kHz and whisper.cpp is robust). If we ever transcribe music,
// add a lowpass before the decimator.

const TARGET_SAMPLE_RATE = 16000;
const TARGET_BYTES_PER_SAMPLE = 2;
const FLUSH_INTERVAL_SECONDS = 0.25;

class RecorderProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.inputSampleRate = sampleRate;
    this.ratio = this.inputSampleRate / TARGET_SAMPLE_RATE;
    this.fracIndex = 0;
    this.flushSamples = Math.round(TARGET_SAMPLE_RATE * FLUSH_INTERVAL_SECONDS);
    this.pending = [];
    this.pendingSamples = 0;
  }

  process(inputs) {
    const input = inputs[0];
    if (!input || input.length === 0) return true;
    const channel = input[0];
    if (!channel || channel.length === 0) return true;

    const expected = Math.max(0, Math.ceil((channel.length - this.fracIndex) / this.ratio));
    const out = new Int16Array(expected);
    let written = 0;
    let i = this.fracIndex;
    while (i < channel.length) {
      const idx = i | 0;
      const frac = i - idx;
      const s1 = channel[idx];
      const s2 = idx + 1 < channel.length ? channel[idx + 1] : s1;
      const sample = s1 + (s2 - s1) * frac;
      const clamped = sample < -1 ? -1 : sample > 1 ? 1 : sample;
      out[written++] = clamped < 0
        ? Math.round(clamped * 0x8000)
        : Math.round(clamped * 0x7fff);
      i += this.ratio;
    }
    this.fracIndex = i - channel.length;

    if (written > 0) {
      const trimmed = written === out.length ? out : out.slice(0, written);
      this.pending.push(trimmed);
      this.pendingSamples += trimmed.length;
    }

    if (this.pendingSamples >= this.flushSamples) {
      this.flush();
    }
    return true;
  }

  flush() {
    if (this.pendingSamples === 0) return;
    const bytes = new Uint8Array(this.pendingSamples * TARGET_BYTES_PER_SAMPLE);
    const view = new DataView(bytes.buffer);
    let offset = 0;
    for (const chunk of this.pending) {
      for (let k = 0; k < chunk.length; k++) {
        view.setInt16(offset, chunk[k], true);
        offset += 2;
      }
    }
    this.pending = [];
    this.pendingSamples = 0;
    this.port.postMessage({ type: "chunk", bytes }, [bytes.buffer]);
  }
}

registerProcessor("recorder-processor", RecorderProcessor);
